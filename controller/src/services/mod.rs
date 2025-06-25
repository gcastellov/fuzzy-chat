pub mod auth_service;
pub mod info_service;
pub mod route_service;

use crate::models::{
    auth_proto::auth_service_server::AuthServiceServer,
    info_proto::info_service_server::InfoServiceServer,
    route_proto::route_service_server::RouteServiceServer,
};
use crate::{membership::MemberManager, routing::RouteManager, session::SessionManager};
use auth_service::AuthServiceImpl;
use crosscutting::{ComponentDescriptor, networking, settings, tracing};
use info_service::InfoServiceImpl;
use log::{debug, info, warn};
use route_service::RouteServiceImpl;
use std::sync::Arc;
use tonic::transport::{Server, ServerTlsConfig};

use tonic::{Request, Response, Status};

pub fn start_server_handler(
    descriptor: ComponentDescriptor,
    session_manager: Arc<SessionManager>,
    route_manger: Arc<RouteManager>,
    member_manager: Arc<MemberManager>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        if let ComponentDescriptor::Controller {
            version,
            connection_settings,
            ..
        } = descriptor
        {
            let socket_address = connection_settings.get_local_socket_address();
            debug!("Starting server on {}", socket_address);

            let auth_service = AuthServiceImpl::new(Arc::clone(&session_manager), member_manager);
            let route_service = RouteServiceImpl::new(Arc::clone(&session_manager), route_manger);
            let info_service = InfoServiceImpl::new(Arc::clone(&session_manager), version);

            debug!("Loading certificates ...");

            let identity =
                settings::service::load_tls_identity("server.crt", "server.key").unwrap();
            let tls_config = ServerTlsConfig::new().identity(identity);

            info!("Starting Controller server with TLS on {}", socket_address);

            _ = Server::builder()
                .tls_config(tls_config)
                .unwrap()
                .layer(tracing::UriTracingLayer)
                .add_service(AuthServiceServer::new(auth_service))
                .add_service(RouteServiceServer::new(route_service))
                .add_service(InfoServiceServer::new(info_service))
                .serve(socket_address)
                .await;
        }
    })
}

mod guards {

    use super::*;

    const INVALID_ACCESS_KEY: &str = "Invalid access key";
    const INVALID_CONVERSATION: &str = "Invalid conversation";

    pub async fn check_session(
        session_manager: &Arc<SessionManager>,
        access_key: &str,
    ) -> Result<(), Status> {
        session_manager
            .get_session(access_key)
            .await
            .ok_or_else(|| Status::unauthenticated(INVALID_ACCESS_KEY))?;
        Ok(())
    }

    pub async fn check_conversation(
        route_manager: &Arc<RouteManager>,
        conversation_id: &str,
    ) -> Result<(), Status> {
        route_manager
            .get_conversation(conversation_id)
            .await
            .ok_or_else(|| Status::not_found(INVALID_CONVERSATION))?;
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::routing::RouteManager;
        use crate::storage::RepositoryType;
        use crosscutting::{Component, ConnectionSettings};
        use tokio_util::sync::CancellationToken;

        const EXPECTED_FROM_UID: &str = "from_uid";
        const EXPECTED_TO_UID: &str = "to_uid";
        const EXPECTED_CONVERSATION_ID: &str = "conversation_id";
        const EXPECTED_IP: &str = "127.0.0.1";
        const EXPECTED_PORT: u16 = 8080;
        const EXPECTED_PUBLIC_KEY: &[u8] = b"test_public_key";
        const EXPECTED_DOMAIN_NAME: &str = "test_domain_name";

        fn get_connection_settings() -> ConnectionSettings {
            ConnectionSettings {
                ip: EXPECTED_IP.to_string(),
                port: EXPECTED_PORT,
                domain_name: EXPECTED_DOMAIN_NAME.to_string(),
                certificate: EXPECTED_PUBLIC_KEY.to_vec(),
            }
        }

        #[tokio::test]
        async fn given_invalid_access_key_when_checking_session_then_returns_error() {
            let repository_type = RepositoryType::InMemory;
            let session_manager = Arc::new(SessionManager::new(
                repository_type,
                CancellationToken::new(),
            ));

            let result = check_session(&session_manager, "invalid_access_key").await;

            assert!(result.is_err());
            let status = result.unwrap_err();
            assert_eq!(status.code(), tonic::Code::Unauthenticated);
            assert_eq!(status.message(), super::INVALID_ACCESS_KEY);
        }

        #[tokio::test]
        async fn given_valid_access_key_when_checking_session_then_returns_ok() {
            let repository_type = RepositoryType::InMemory;
            let session_manager = Arc::new(SessionManager::new(
                repository_type,
                CancellationToken::new(),
            ));
            let access_key = session_manager
                .set_session(
                    EXPECTED_FROM_UID,
                    Component::Client,
                    &networking::to_socket_address(EXPECTED_IP, EXPECTED_PORT).unwrap(),
                    &get_connection_settings(),
                )
                .await;

            let result = check_session(&session_manager, &access_key).await;

            assert!(result.is_ok());
        }

        #[tokio::test]
        async fn given_non_existing_conversation_when_checking_conversation_then_returns_error() {
            let repository_type = RepositoryType::InMemory;
            let route_manager =
                Arc::new(RouteManager::new(repository_type, CancellationToken::new()));

            let result = check_conversation(&route_manager, EXPECTED_CONVERSATION_ID).await;

            assert!(result.is_err());
            let status = result.unwrap_err();
            assert_eq!(status.code(), tonic::Code::NotFound);
            assert_eq!(status.message(), super::INVALID_CONVERSATION);
        }

        #[tokio::test]
        async fn given_existing_conversation_when_checking_conversation_then_returns_ok() {
            let repository_type = RepositoryType::InMemory;
            let route_manager =
                Arc::new(RouteManager::new(repository_type, CancellationToken::new()));
            let conversation_id = route_manager
                .initialize(EXPECTED_FROM_UID, EXPECTED_TO_UID)
                .await
                .unwrap();

            let result = check_conversation(&route_manager, &conversation_id).await;

            assert!(result.is_ok());
        }
    }
}
