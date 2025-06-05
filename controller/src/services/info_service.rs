use super::*;
use crate::models::info_proto::{StatusRequest, StatusResponse, info_service_server::InfoService};
use crate::session::SessionManager;

pub struct InfoServiceImpl {
    session_manager: Arc<SessionManager>,
    version: String,
}

impl InfoServiceImpl {
    pub fn new(session_manager: Arc<SessionManager>, version: String) -> Self {
        Self {
            session_manager,
            version,
        }
    }
}

#[tonic::async_trait]
impl InfoService for InfoServiceImpl {
    async fn status(
        &self,
        request: Request<StatusRequest>,
    ) -> Result<Response<StatusResponse>, Status> {
        let status_request = request.into_inner();
        let access_key = status_request.access_key.clone();
        guards::check_session(&self.session_manager, access_key.as_str()).await?;

        let proxies_count = self.session_manager.count_proxies().await;
        let clients_count = self.session_manager.count_clients().await;
        let controllers_count = self.session_manager.count_controllers().await;

        let reply = StatusResponse {
            version: self.version.clone(),
            connected_clients: u32::try_from(clients_count).unwrap_or_default(),
            connected_proxies: u32::try_from(proxies_count).unwrap_or_default(),
            connected_controllers: u32::try_from(controllers_count).unwrap_or_default(),
        };

        Ok(Response::new(reply))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        models::auth_proto::ComponentType, session::SessionManager, storage::RepositoryType,
    };
    use tokio_util::sync::CancellationToken;

    const EXPECTED_UID: &str = "L.KD<FCjkSA6AEg@";
    const EXPECTED_IP: &str = "127.0.0.1";
    const EXPECTED_PORT: u16 = 8080;
    const EXPECTED_VERSION: &str = "1.0.0";

    #[tokio::test]
    async fn given_client_session_when_getting_status_then_returns_expected_status() {
        let cancellation_token = CancellationToken::new();
        let repository_type = RepositoryType::InMemory;
        let session_manager =
            SessionManager::new(repository_type, cancellation_token.child_token());

        let access_key = session_manager
            .set_session(
                ComponentType::Client as u8,
                EXPECTED_UID,
                networking::to_socket_address(EXPECTED_IP, EXPECTED_PORT).unwrap(),
                EXPECTED_IP,
                EXPECTED_PORT,
            )
            .await;

        let service = InfoServiceImpl::new(Arc::new(session_manager), EXPECTED_VERSION.to_string());

        let request = StatusRequest {
            access_key: access_key,
        };

        let response = service.status(Request::new(request)).await;
        assert!(response.is_ok());

        let response = response.unwrap().into_inner();
        assert_eq!(response.version, EXPECTED_VERSION);
        assert_eq!(response.connected_clients, 1);
        assert_eq!(response.connected_proxies, 0);
        assert_eq!(response.connected_controllers, 0);
    }
}
