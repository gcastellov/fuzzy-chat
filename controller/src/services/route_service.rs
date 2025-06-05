use crate::{
    models::{
        SessionInfo,
        route_proto::{
            InitRequest, InitResponse, RedeemRequest, RedeemResponse, RouteRequest, RouteResponse,
            SourceInfo, route_service_server::RouteService,
        },
    },
    routing::RouteManager,
    session::SessionManager,
};

use super::*;

const CONTROLLER_UID: &str = "controller_uid";

pub struct RouteServiceImpl {
    route_manager: Arc<RouteManager>,
    session_manager: Arc<SessionManager>,
}

impl RouteServiceImpl {
    pub fn new(session_manager: Arc<SessionManager>, route_manager: Arc<RouteManager>) -> Self {
        Self {
            route_manager,
            session_manager,
        }
    }

    async fn handle_route(
        &self,
        conversation_id: &str,
        session_info: &SessionInfo,
        end_route: bool,
    ) -> Result<Response<RouteResponse>, Status> {
        let nonce = self
            .route_manager
            .store_route(
                conversation_id.to_string(),
                session_info.on_ip_address.clone(),
                session_info.on_port_number,
                end_route,
            )
            .await
            .ok_or(Status::internal("Failed to store route"))?;

        let response = RouteResponse {
            ip_address: session_info.on_ip_address.clone(),
            port_number: session_info.on_port_number as u32,
            nonce,
            end_route,
        };

        Ok(Response::new(response))
    }
}

#[tonic::async_trait]
impl RouteService for RouteServiceImpl {
    async fn initialize(
        &self,
        request: Request<InitRequest>,
    ) -> Result<Response<InitResponse>, Status> {
        let init_request = request.into_inner();
        let access_key = init_request.access_key.to_owned();

        guards::check_session(&self.session_manager, access_key.as_str()).await?;
        let session_info = self.session_manager.get_session(&access_key).await.unwrap();

        let to = if init_request.to.is_empty() {
            CONTROLLER_UID.to_string()
        } else {
            init_request.to
        };

        let conversation_id = self
            .route_manager
            .initialize(&session_info.uid, &to)
            .await
            .ok_or_else(|| Status::internal("Failed to initialize conversation"))?;

        let response = InitResponse { conversation_id };

        Ok(Response::new(response))
    }

    async fn route(
        &self,
        request: Request<RouteRequest>,
    ) -> Result<Response<RouteResponse>, Status> {
        let route_request = request.into_inner();
        let conversation_id = route_request.conversation_id.to_owned();
        let access_key = route_request.access_key.to_owned();

        guards::check_session(&self.session_manager, access_key.as_str()).await?;
        guards::check_conversation(&self.route_manager, &conversation_id).await?;

        let conversation = self
            .route_manager
            .get_conversation(&conversation_id)
            .await
            .unwrap();

        if self.route_manager.check_for_final_route(&conversation) {
            let client = self.session_manager.get_client(&conversation.to).await;
            if let Some(client_session) = client {
                return self
                    .handle_route(&conversation_id, &client_session, true)
                    .await;
            }

            return Err(Status::not_found(
                "Reached final route, no more routes available as no client found",
            ));
        }

        let proxies = self.session_manager.get_proxies(&access_key).await;

        if proxies.is_none() {
            return Err(Status::not_found("No proxies found"));
        }

        if let Some(proxy_session) = self
            .route_manager
            .get_next_route(&conversation, &proxies.unwrap())
            .await
        {
            return self
                .handle_route(&conversation_id, &proxy_session, false)
                .await;
        }

        return Err(Status::not_found(
            "Next route wasn't found, no more routes available",
        ));
    }

    async fn redeem(
        &self,
        request: Request<RedeemRequest>,
    ) -> Result<Response<RedeemResponse>, Status> {
        let redeem_request = request.into_inner();
        let access_key = redeem_request.access_key.to_owned();
        let conversation_id = redeem_request.conversation_id.to_owned();

        guards::check_session(&self.session_manager, access_key.as_str()).await?;
        guards::check_conversation(&self.route_manager, &conversation_id).await?;

        if let Some(route) = self
            .route_manager
            .redeem_route(&conversation_id, &redeem_request.nonce)
            .await
        {
            let mut response = RedeemResponse { source_info: None };

            if route.end_route {
                let conversation = self
                    .route_manager
                    .get_conversation(&conversation_id)
                    .await
                    .unwrap();
                response.source_info = Some(SourceInfo {
                    from: conversation.from,
                });
                self.route_manager.finalize(&conversation_id).await;
            }

            return Ok(Response::new(response));
        }

        Err(Status::internal("Failed to redeem route"))
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::models::auth_proto::ComponentType;
    use crate::routing::RouteManager;
    use crate::session::SessionManager;
    use crate::storage::RepositoryType;
    use tokio_util::sync::CancellationToken;

    const EXPECTED_CONVERSATION_ID: &str = "test_conversation_id";
    const EXPECTED_UID: &str = "L.KD<FCjkSA6AEg@";
    const EXPECTED_IP: &str = "127.0.0.1";
    const EXPECTED_PORT: u16 = 8080;
    const EXPECTED_TARGET: &str = "test_target";
    const EXPECTED_ACCESS_KEY: &str = "test_access_key";
    const EXPECTED_NONCE: &str = "test_nonce";

    #[tokio::test]
    async fn given_non_existing_session_when_initializing_conversation_then_returns_error() {
        let cancellation_token = CancellationToken::new();
        let repository_type = RepositoryType::InMemory;
        let route_manager = Arc::new(RouteManager::new(
            repository_type,
            cancellation_token.child_token(),
        ));
        let session_manager = Arc::new(SessionManager::new(
            repository_type,
            cancellation_token.child_token(),
        ));
        let route_service = RouteServiceImpl::new(session_manager, route_manager);

        let init_request = InitRequest {
            access_key: EXPECTED_ACCESS_KEY.to_string(),
            to: EXPECTED_TARGET.to_string(),
        };

        let request = Request::new(init_request);
        let result = route_service.initialize(request).await;

        assert!(result.is_err());
        let status = result.unwrap_err();
        assert_eq!(status.code(), tonic::Code::Unauthenticated);
    }

    #[tokio::test]
    async fn given_existing_session_when_initializing_conversation_then_returns_ok() {
        let cancellation_token = CancellationToken::new();
        let repository_type = RepositoryType::InMemory;
        let route_manager = Arc::new(RouteManager::new(
            repository_type,
            cancellation_token.child_token(),
        ));
        let session_manager = Arc::new(SessionManager::new(
            repository_type,
            cancellation_token.child_token(),
        ));
        let route_service = RouteServiceImpl::new(session_manager.clone(), route_manager.clone());

        let access_key = session_manager
            .set_session(
                ComponentType::Client as u8,
                EXPECTED_UID,
                networking::to_socket_address(EXPECTED_IP, EXPECTED_PORT).unwrap(),
                EXPECTED_IP,
                EXPECTED_PORT,
            )
            .await;

        let init_request = InitRequest {
            access_key: access_key,
            to: EXPECTED_TARGET.to_string(),
        };

        let request = Request::new(init_request);
        let result = route_service.initialize(request).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn given_non_existing_session_when_routing_then_returns_error() {
        let cancellation_token = CancellationToken::new();
        let repository_type = RepositoryType::InMemory;
        let route_manager = Arc::new(RouteManager::new(
            repository_type,
            cancellation_token.child_token(),
        ));
        let session_manager = Arc::new(SessionManager::new(
            repository_type,
            cancellation_token.child_token(),
        ));
        let route_service = RouteServiceImpl::new(session_manager, route_manager);

        let route_request = RouteRequest {
            access_key: EXPECTED_ACCESS_KEY.to_string(),
            conversation_id: EXPECTED_CONVERSATION_ID.to_string(),
        };

        let request = Request::new(route_request);
        let result = route_service.route(request).await;

        assert!(result.is_err());
        let status = result.unwrap_err();
        assert_eq!(status.code(), tonic::Code::Unauthenticated);
    }

    #[tokio::test]
    async fn given_existing_session_but_conversation_when_routing_then_returns_error() {
        let cancellation_token = CancellationToken::new();
        let repository_type = RepositoryType::InMemory;
        let route_manager = Arc::new(RouteManager::new(
            repository_type,
            cancellation_token.child_token(),
        ));
        let session_manager = Arc::new(SessionManager::new(
            repository_type,
            cancellation_token.child_token(),
        ));
        let route_service = RouteServiceImpl::new(session_manager.clone(), route_manager.clone());

        let access_key = session_manager
            .set_session(
                ComponentType::Client as u8,
                EXPECTED_UID,
                networking::to_socket_address(EXPECTED_IP, EXPECTED_PORT).unwrap(),
                EXPECTED_IP,
                EXPECTED_PORT,
            )
            .await;

        let route_request = RouteRequest {
            access_key: access_key,
            conversation_id: EXPECTED_CONVERSATION_ID.to_string(),
        };

        let request = Request::new(route_request);
        let result = route_service.route(request).await;

        assert!(result.is_err());
        let status = result.unwrap_err();
        assert_eq!(status.code(), tonic::Code::NotFound);
    }

    #[tokio::test]
    async fn given_existing_session_and_conversation_but_proxies_when_routing_then_returns_error() {
        let cancellation_token = CancellationToken::new();
        let repository_type = RepositoryType::InMemory;
        let route_manager = Arc::new(RouteManager::new(
            repository_type,
            cancellation_token.child_token(),
        ));
        let session_manager = Arc::new(SessionManager::new(
            repository_type,
            cancellation_token.child_token(),
        ));
        let route_service = RouteServiceImpl::new(session_manager.clone(), route_manager.clone());

        let access_key = session_manager
            .set_session(
                ComponentType::Client as u8,
                EXPECTED_UID,
                networking::to_socket_address(EXPECTED_IP, EXPECTED_PORT).unwrap(),
                EXPECTED_IP,
                EXPECTED_PORT,
            )
            .await;

        let conversation_id = route_manager
            .initialize(EXPECTED_UID, EXPECTED_TARGET)
            .await
            .unwrap();

        let route_request = RouteRequest {
            access_key: access_key,
            conversation_id: conversation_id.to_string(),
        };

        let request = Request::new(route_request);
        let result = route_service.route(request).await;

        assert!(result.is_err());
        let status = result.unwrap_err();
        assert_eq!(status.code(), tonic::Code::NotFound);
    }

    #[tokio::test]
    async fn given_existing_session_and_conversation_and_proxies_when_routing_then_returns_ok() {
        let cancellation_token = CancellationToken::new();
        let repository_type = RepositoryType::InMemory;
        let route_manager = Arc::new(RouteManager::new(
            repository_type,
            cancellation_token.child_token(),
        ));
        let session_manager = Arc::new(SessionManager::new(
            repository_type,
            cancellation_token.child_token(),
        ));
        let route_service = RouteServiceImpl::new(session_manager.clone(), route_manager.clone());

        let access_key = session_manager
            .set_session(
                ComponentType::Client as u8,
                EXPECTED_UID,
                networking::to_socket_address(EXPECTED_IP, EXPECTED_PORT).unwrap(),
                EXPECTED_IP,
                EXPECTED_PORT,
            )
            .await;

        session_manager
            .set_session(
                ComponentType::Proxy as u8,
                EXPECTED_UID,
                networking::to_socket_address(EXPECTED_IP, EXPECTED_PORT).unwrap(),
                EXPECTED_IP,
                EXPECTED_PORT,
            )
            .await;

        let conversation_id = route_manager
            .initialize(EXPECTED_UID, EXPECTED_TARGET)
            .await
            .unwrap();

        let route_request = RouteRequest {
            access_key: access_key,
            conversation_id: conversation_id.to_string(),
        };

        let request = Request::new(route_request);
        let result = route_service.route(request).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn given_non_existing_session_when_redeeming_then_returns_error() {
        let cancellation_token = CancellationToken::new();
        let repository_type = RepositoryType::InMemory;
        let route_manager = Arc::new(RouteManager::new(
            repository_type,
            cancellation_token.child_token(),
        ));
        let session_manager = Arc::new(SessionManager::new(
            repository_type,
            cancellation_token.child_token(),
        ));
        let route_service = RouteServiceImpl::new(session_manager, route_manager);

        let redeem_request = RedeemRequest {
            access_key: EXPECTED_ACCESS_KEY.to_string(),
            conversation_id: EXPECTED_CONVERSATION_ID.to_string(),
            nonce: EXPECTED_NONCE.to_string(),
        };

        let request = Request::new(redeem_request);
        let result = route_service.redeem(request).await;

        assert!(result.is_err());
        let status = result.unwrap_err();
        assert_eq!(status.code(), tonic::Code::Unauthenticated);
    }

    #[tokio::test]
    async fn given_existing_session_but_conversation_when_redeeming_then_returns_error() {
        let cancellation_token = CancellationToken::new();
        let repository_type = RepositoryType::InMemory;
        let route_manager = Arc::new(RouteManager::new(
            repository_type,
            cancellation_token.child_token(),
        ));
        let session_manager = Arc::new(SessionManager::new(
            repository_type,
            cancellation_token.child_token(),
        ));
        let route_service = RouteServiceImpl::new(session_manager.clone(), route_manager.clone());

        let access_key = session_manager
            .set_session(
                ComponentType::Client as u8,
                EXPECTED_UID,
                networking::to_socket_address(EXPECTED_IP, EXPECTED_PORT).unwrap(),
                EXPECTED_IP,
                EXPECTED_PORT,
            )
            .await;

        let redeem_request = RedeemRequest {
            access_key: access_key,
            conversation_id: EXPECTED_CONVERSATION_ID.to_string(),
            nonce: EXPECTED_NONCE.to_string(),
        };

        let request = Request::new(redeem_request);
        let result = route_service.redeem(request).await;

        assert!(result.is_err());
        let status = result.unwrap_err();
        assert_eq!(status.code(), tonic::Code::NotFound);
    }

    #[tokio::test]
    async fn given_existing_session_and_conversation_but_route_when_redeeming_then_returns_error() {
        let cancellation_token = CancellationToken::new();
        let repository_type = RepositoryType::InMemory;
        let route_manager = Arc::new(RouteManager::new(
            repository_type,
            cancellation_token.child_token(),
        ));
        let session_manager = Arc::new(SessionManager::new(
            repository_type,
            cancellation_token.child_token(),
        ));
        let route_service = RouteServiceImpl::new(session_manager.clone(), route_manager.clone());

        let access_key = session_manager
            .set_session(
                ComponentType::Client as u8,
                EXPECTED_UID,
                networking::to_socket_address(EXPECTED_IP, EXPECTED_PORT).unwrap(),
                EXPECTED_IP,
                EXPECTED_PORT,
            )
            .await;

        let conversation_id = route_manager
            .initialize(EXPECTED_UID, EXPECTED_TARGET)
            .await
            .unwrap();

        let redeem_request = RedeemRequest {
            access_key: access_key,
            conversation_id: conversation_id.to_string(),
            nonce: EXPECTED_NONCE.to_string(),
        };

        let request = Request::new(redeem_request);
        let result = route_service.redeem(request).await;

        assert!(result.is_err());
        let status = result.unwrap_err();
        assert_eq!(status.code(), tonic::Code::Internal);
    }

    #[tokio::test]
    async fn given_existing_session_and_conversation_and_route_when_redeeming_then_returns_ok() {
        let cancellation_token = CancellationToken::new();
        let repository_type = RepositoryType::InMemory;
        let route_manager = Arc::new(RouteManager::new(
            repository_type,
            cancellation_token.child_token(),
        ));
        let session_manager = Arc::new(SessionManager::new(
            repository_type,
            cancellation_token.child_token(),
        ));
        let route_service = RouteServiceImpl::new(session_manager.clone(), route_manager.clone());

        let access_key = session_manager
            .set_session(
                ComponentType::Client as u8,
                EXPECTED_UID,
                networking::to_socket_address(EXPECTED_IP, EXPECTED_PORT).unwrap(),
                EXPECTED_IP,
                EXPECTED_PORT,
            )
            .await;

        let conversation_id = route_manager
            .initialize(EXPECTED_UID, EXPECTED_TARGET)
            .await
            .unwrap();
        let nonce = route_manager
            .store_route(
                conversation_id.clone(),
                EXPECTED_IP.to_string(),
                EXPECTED_PORT,
                false,
            )
            .await
            .unwrap();

        let redeem_request = RedeemRequest {
            access_key,
            conversation_id,
            nonce: nonce,
        };

        let request = Request::new(redeem_request);
        let result = route_service.redeem(request).await;

        assert!(result.is_ok());
    }
}
