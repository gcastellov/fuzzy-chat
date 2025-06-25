use crosscutting::networking;
use routing::proxy_client::proxy::{CommandResponse, CommandType};
use routing::proxy_client::{ProxyClientFactory, ProxyFactory};
use routing::route_client::{RouteClientFactory, RouterFactory};
use std::error::Error;
use tonic::Status;
use tonic::transport::Uri;

pub enum Command {
    Send(String, Vec<u8>),
    Status,
}

impl Command {
    const SEND: &'static str = "/send";
    const STATUS: &'static str = "/status";

    pub fn from_str(command: &str) -> Result<Self, String> {
        let mut wording = command.split_whitespace();
        let cmd = wording.nth(0).unwrap_or("");
        match cmd.to_lowercase().as_str() {
            Command::SEND => {
                if let Some(to) = wording.next() {
                    let index = command.find(to).unwrap();
                    let message: Vec<u8> = command[index + to.len()..].trim().as_bytes().into();
                    if !message.is_empty() {
                        return Ok(Command::Send(to.to_string(), message));
                    }
                }

                Err("Invalid command format. Usage: /send <to> <message>".into())
            }
            Command::STATUS => Ok(Command::Status),
            _ => Err(format!("Unknown command: {}", command)),
        }
    }
}

struct Route {
    conversation_id: String,
    nonce: String,
    public_key: Vec<u8>,
    domain_name: String,
    uri: Uri,
}

pub struct Commander {
    access_key: String,
    router_factory: Box<dyn RouterFactory>,
    proxy_factory: Box<dyn ProxyFactory>,
}

impl Commander {
    pub fn new(access_key: String) -> Self {
        Commander {
            access_key,
            router_factory: Box::new(RouteClientFactory),
            proxy_factory: Box::new(ProxyClientFactory),
        }
    }

    pub async fn send_message(
        &mut self,
        to: &str,
        content: &[u8],
    ) -> Result<CommandResponse, Box<dyn Error>> {
        let route = self.initialize(to).await?;
        let mut proxy_client =
            self.proxy_factory
                .get_proxy(route.uri, route.public_key, route.domain_name);
        proxy_client.initialize().await.map_err(|e| {
            Status::internal(format!("Impossible to initialize proxy client: {}", e))
        })?;

        let response = proxy_client
            .send_command(
                route.conversation_id,
                route.nonce,
                CommandType::Send,
                content.to_vec(),
            )
            .await?;
        Ok(response)
    }

    pub async fn get_status(&mut self) -> Result<CommandResponse, Box<dyn Error>> {
        let route = self.initialize(&String::default()).await?;
        let mut proxy_client =
            self.proxy_factory
                .get_proxy(route.uri, route.public_key, route.domain_name);
        proxy_client.initialize().await.map_err(|e| {
            Status::internal(format!("Impossible to initialize proxy client: {}", e))
        })?;
        let response = proxy_client
            .send_command(
                route.conversation_id,
                route.nonce,
                CommandType::Status,
                vec![],
            )
            .await?;
        Ok(response)
    }

    async fn initialize(&mut self, to: &str) -> Result<Route, Box<dyn Error>> {
        let mut router = self.router_factory.get_router();
        router.initialize().await?;
        let init_response = router
            .init_conversation(self.access_key.clone(), to.to_string())
            .await?;
        let conversation_id = init_response.conversation_id;
        let route_response = router
            .get_route(conversation_id.clone(), self.access_key.clone())
            .await?;
        let uri =
            networking::to_https_endpoint(&route_response.ip_address, route_response.port_number)?;

        Ok(Route {
            conversation_id: conversation_id.clone(),
            nonce: route_response.nonce,
            public_key: route_response.public_key,
            domain_name: route_response.domain_name,
            uri: uri.clone(),
        })
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn send_command_from_str_is_well_formatted() {
        const CMD_STR: &str = "/send user123 Hello, World!";
        const EXPECTED_UID: &str = "user123";
        const EXPECTED_CONTENT: &[u8] = b"Hello, World!";

        let command = Command::from_str(CMD_STR);

        assert!(command.is_ok());
        if let Command::Send(to, content) = command.unwrap() {
            assert_eq!(to, EXPECTED_UID);
            assert_eq!(content, EXPECTED_CONTENT);
        }
    }

    #[test]
    fn send_command_from_str_with_invalid_format_returns_error() {
        const CMD_STR: &str = "/send";
        const EXPECTED_ERROR: &str = "Invalid command format. Usage: /send <to> <message>";

        let command = Command::from_str(CMD_STR);

        assert!(command.is_err());
        assert_eq!(command.err().unwrap(), EXPECTED_ERROR);
    }

    #[test]
    fn send_command_from_str_with_no_content_returns_error() {
        const CMD_STR: &str = "/send uid132132";
        const EXPECTED_ERROR: &str = "Invalid command format. Usage: /send <to> <message>";

        let command = Command::from_str(CMD_STR);

        assert!(command.is_err());
        assert_eq!(command.err().unwrap(), EXPECTED_ERROR);
    }

    #[test]
    fn status_command_from_str_is_well_formatted() {
        const CMD_STR: &str = "/status";

        let command = Command::from_str(CMD_STR);

        assert!(command.is_ok());
    }

    #[test]
    fn invalid_command_from_str_returns_error() {
        const CMD_STR: &str = "/invalid_command";
        const EXPECTED_ERROR: &str = "Unknown command: /invalid_command";

        let command = Command::from_str(CMD_STR);

        assert!(command.is_err());
        assert_eq!(command.err().unwrap(), EXPECTED_ERROR);
    }
}
