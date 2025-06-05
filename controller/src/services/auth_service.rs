use super::*;
use crate::models::auth_proto::{
    ComponentType, LoginRequest, LoginResponse, LogoutRequest, LogoutResponse, PingRequest,
    PingResponse, auth_service_server::AuthService,
};
use crate::{membership::MemberManager, session::SessionManager};
use std::net::SocketAddr;

fn get_remote_address<T>(request: &Request<T>) -> Option<SocketAddr> {
    if cfg!(test) {
        Some(networking::to_socket_address("127.0.0.1", 8080).unwrap())
    } else {
        request.remote_addr()
    }
}

pub struct AuthServiceImpl {
    session_manager: Arc<SessionManager>,
    member_manager: Arc<MemberManager>,
}

pub trait RemoteAddress {
    fn get_remote_address(&self) -> Option<SocketAddr>;
}

impl RemoteAddress for Request<LoginRequest> {
    fn get_remote_address(&self) -> Option<SocketAddr> {
        get_remote_address(self)
    }
}

impl RemoteAddress for Request<PingRequest> {
    fn get_remote_address(&self) -> Option<SocketAddr> {
        get_remote_address(self)
    }
}

impl AuthServiceImpl {
    pub fn new(session_manager: Arc<SessionManager>, member_manager: Arc<MemberManager>) -> Self {
        Self {
            session_manager,
            member_manager,
        }
    }

    async fn validate_credentials(&self, uid: &str, pwd: &str) -> bool {
        self.member_manager
            .get_member(uid)
            .await
            .map(|member| member.pwd == pwd)
            .unwrap_or(false)
    }
}

#[tonic::async_trait]
impl AuthService for AuthServiceImpl {
    async fn ping(&self, request: Request<PingRequest>) -> Result<Response<PingResponse>, Status> {
        let client_ip = request
            .get_remote_address()
            .ok_or_else(|| Status::internal("Could not get client IP address"))?;

        let ping_request = request.into_inner();
        let access_key = ping_request.access_key.clone();
        guards::check_session(&self.session_manager, access_key.as_str()).await?;

        let session = self
            .session_manager
            .get_session(access_key.as_str())
            .await
            .unwrap();
        if session.client_ip != client_ip {
            warn!(
                "Session IP mismatch: expected {}, got {}",
                session.client_ip, client_ip
            );
            return Err(Status::unauthenticated("Invalid connection"));
        }

        let reply = PingResponse {
            status: "PONG".to_string(),
            timestamp: chrono::Utc::now().timestamp_micros(),
        };

        Ok(Response::new(reply))
    }

    async fn login(
        &self,
        request: Request<LoginRequest>,
    ) -> Result<Response<LoginResponse>, Status> {
        let client_ip = request
            .get_remote_address()
            .ok_or_else(|| Status::internal("Could not get client IP address"))?;

        let login_request = request.into_inner();
        if login_request.uid.is_empty() || login_request.uid.is_empty() {
            warn!("UID or PWD are empty");
            return Err(Status::invalid_argument("UID and PWD cannot be empty"));
        }

        let component_type = ComponentType::try_from(login_request.component_type)
            .map_err(|_| Status::invalid_argument("Invalid component type"))?;

        if component_type == ComponentType::Controller {
            return Err(Status::invalid_argument("Invalid component type"));
        }

        if self
            .validate_credentials(&login_request.uid, &login_request.pwd)
            .await
        {
            let access_key = self
                .session_manager
                .set_session(
                    component_type as u8,
                    &login_request.uid,
                    client_ip,
                    &login_request.on_ip,
                    login_request.on_port as u16,
                )
                .await;

            debug!(
                "Storing session for {}: on: {}:{}",
                access_key, login_request.on_ip, login_request.on_port
            );

            let reply = LoginResponse {
                access_key,
                message: "Login successful".to_string(),
            };

            let component: &str = component_type.as_str_name();
            info!(
                "Accepted connection from {} : {}",
                component, reply.access_key
            );
            return Ok(Response::new(reply));
        }

        Err(Status::unauthenticated("Invalid credentials".to_string()))
    }

    async fn logout(
        &self,
        request: Request<LogoutRequest>,
    ) -> Result<Response<LogoutResponse>, Status> {
        let login_request = request.into_inner();
        let access_key = login_request.access_key.clone();
        guards::check_session(&self.session_manager, access_key.as_str()).await?;

        self.session_manager
            .remove_session(access_key.as_str())
            .await;

        info!("Session dropped: {}", access_key);
        Ok(Response::new(LogoutResponse {}))
    }
}

#[cfg(test)]
mod tests {
    use std::vec;

    use super::*;
    use crate::membership::MemberManager;
    use crate::models::Member;
    use crate::session::SessionManager;
    use crate::storage::RepositoryType;
    use tokio_util::sync::CancellationToken;

    const EXPECTED_UID: &str = "L.KD<FCjkSA6AEg@";
    const EXPECTED_PWD: &str = "w8(PR&-HCJ*ersZV";
    const EXPECTED_IP: &str = "127.0.0.1";
    const EXPECTED_PORT: u16 = 8080;

    fn create_service() -> AuthServiceImpl {
        let cancellation_token = CancellationToken::new();
        let repository_type = RepositoryType::InMemory;
        let session_manager =
            SessionManager::new(repository_type, cancellation_token.child_token());
        let member_manager = MemberManager::new(repository_type);
        AuthServiceImpl::new(Arc::new(session_manager), Arc::new(member_manager))
    }

    fn create_login_request() -> Request<LoginRequest> {
        Request::new(LoginRequest {
            component_type: ComponentType::Client as i32,
            uid: EXPECTED_UID.to_string(),
            pwd: EXPECTED_PWD.to_string(),
            on_ip: EXPECTED_IP.to_string(),
            on_port: EXPECTED_PORT as u32,
        })
    }

    impl MemberManager {
        async fn load_memebers(&self) {
            let members = vec![Member::new(
                EXPECTED_UID.to_string(),
                EXPECTED_PWD.to_string(),
            )];

            self.set_members(&members).await;
        }
    }

    #[tokio::test]
    async fn given_existing_member_when_login_is_called_then_login_is_successful() {
        let service = create_service();
        service.member_manager.load_memebers().await;

        let request = create_login_request();
        let response = service.login(request).await;

        assert!(response.is_ok());
        let login_response = response.unwrap().into_inner();
        assert_eq!(login_response.message, "Login successful");
        assert!(!login_response.access_key.is_empty());
    }

    #[tokio::test]
    async fn given_non_existing_member_when_login_is_called_then_login_is_unsuccessful() {
        let service = create_service();
        let expected_status = Status::unauthenticated("Invalid credentials");
        let request = create_login_request();
        let response = service.login(request).await;

        assert!(response.is_err());
        let status = response.unwrap_err();
        assert_eq!(status.code(), expected_status.code());
        assert_eq!(status.message(), expected_status.message());
    }

    #[tokio::test]
    async fn given_existing_session_when_logout_is_called_then_logout_is_successful() {
        let service = create_service();
        service.member_manager.load_memebers().await;
        let request = create_login_request();
        let login_response = service.login(request).await.unwrap().into_inner();
        let request = Request::new(LogoutRequest {
            access_key: login_response.access_key.clone(),
        });

        let response = service.logout(request).await;

        assert!(response.is_ok());
    }

    #[tokio::test]
    async fn given_non_existing_session_when_logout_is_called_then_logout_is_unsuccessful() {
        let service = create_service();
        let expected_status = Status::unauthenticated("Invalid access key");
        let request = Request::new(LogoutRequest {
            access_key: "some_invalid_key".to_string(),
        });

        let response = service.logout(request).await;

        assert!(response.is_err());
        let status = response.unwrap_err();
        assert_eq!(status.code(), expected_status.code());
        assert_eq!(status.message(), expected_status.message());
    }

    #[tokio::test]
    async fn given_existing_session_when_ping_is_called_then_ping_is_successful() {
        let service = create_service();
        service.member_manager.load_memebers().await;
        let request = create_login_request();
        let login_response = service.login(request).await.unwrap().into_inner();
        let request = Request::new(PingRequest {
            access_key: login_response.access_key.clone(),
        });

        let response = service.ping(request).await;

        assert!(response.is_ok());
        let inner_response = response.unwrap().into_inner();
        assert_eq!(inner_response.status, "PONG");
    }

    #[tokio::test]
    async fn given_non_existing_session_when_ping_is_called_then_ping_is_unsuccessful() {
        let service = create_service();
        let expected_status = Status::unauthenticated("Invalid access key");
        let request = Request::new(PingRequest {
            access_key: "some_invalid_key".to_string(),
        });

        let response = service.ping(request).await;

        assert!(response.is_err());
        let status = response.unwrap_err();
        assert_eq!(status.code(), expected_status.code());
        assert_eq!(status.message(), expected_status.message());
    }
}
