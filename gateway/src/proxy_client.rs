use crosscutting::abstractions::GrpcClient;
use mockall::automock;
use std::error::Error;
use tonic::{
    Request, async_trait,
    transport::{Channel, ClientTlsConfig, Uri},
};

pub mod proxy {
    tonic::include_proto!("proxy");
}

pub use proxy::{
    CommandRequest, CommandResponse, CommandType, proxy_service_client::ProxyServiceClient,
};

struct ProxyClient {
    endpoint: Uri,
    public_key: Vec<u8>,
    domain_name: String,
    client: Option<ProxyServiceClient<Channel>>,
}

#[automock]
pub trait ProxyFactory: Send + Sync {
    fn get_proxy(&self, endpoint: Uri, public_key: Vec<u8>, domain_name: String) -> Box<dyn Proxy>;
}

#[derive(Default)]
pub struct ProxyClientFactory;

#[async_trait]
#[automock]
pub trait Proxy: GrpcClient {
    async fn send_command(
        &mut self,
        conversation_id: String,
        nonce: String,
        command: CommandType,
        content: Vec<u8>,
    ) -> Result<CommandResponse, Box<dyn Error>>;
}

#[async_trait]
impl GrpcClient for ProxyClient {
    async fn initialize(&mut self) -> Result<(), Box<dyn Error>> {
        let tls_config = ClientTlsConfig::new()
            .ca_certificate(tonic::transport::Certificate::from_pem(
                self.public_key.clone(),
            ))
            .domain_name(self.domain_name.clone());

        let channel = Channel::builder(self.endpoint.to_owned())
            .tls_config(tls_config)?
            .connect()
            .await?;

        self.client = Some(ProxyServiceClient::new(channel));
        Ok(())
    }
}

#[async_trait]
impl Proxy for ProxyClient {
    async fn send_command(
        &mut self,
        conversation_id: String,
        nonce: String,
        command: CommandType,
        content: Vec<u8>,
    ) -> Result<CommandResponse, Box<dyn Error>> {
        if self.client.is_none() {
            return Err("Client not initialized".into());
        }

        let request = CommandRequest {
            conversation_id,
            nonce,
            command: command.into(),
            content: Some(content),
        };

        let response = self
            .client
            .as_mut()
            .unwrap()
            .execute_command(Request::new(request))
            .await?;

        Ok(response.into_inner())
    }
}

impl ProxyClient {
    pub fn new(endpoint: Uri, public_key: Vec<u8>, domain_name: String) -> Self {
        Self {
            endpoint,
            public_key,
            domain_name,
            client: None,
        }
    }
}

#[async_trait]
impl GrpcClient for MockProxy {
    async fn initialize(&mut self) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
}

impl ProxyFactory for ProxyClientFactory {
    fn get_proxy(&self, endpoint: Uri, public_key: Vec<u8>, domain_name: String) -> Box<dyn Proxy> {
        Box::new(ProxyClient::new(endpoint, public_key, domain_name))
    }
}
