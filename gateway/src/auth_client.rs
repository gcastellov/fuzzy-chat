use crosscutting::{
    Component, ComponentDescriptor, ConnectionSettings, Credentials, abstractions::GrpcClient,
};
use mockall::automock;
use std::{error::Error, sync::Arc};
use tokio::sync::RwLock;
use tonic::{
    Request, async_trait,
    transport::{self, Channel, ClientTlsConfig},
};

use super::auth_proto::{
    LoginRequest, LogoutRequest, PingRequest, auth_service_client::AuthServiceClient,
};

#[derive(Debug, Clone, Default)]
pub struct ClientSession {
    pub access_key: Option<String>,
    pub uid: Option<String>,
}

impl ClientSession {
    pub fn set_session(&mut self, uid: String, access_key: String) {
        self.uid = Some(uid);
        self.access_key = Some(access_key);
    }

    pub fn is_authenticated(&self) -> bool {
        self.uid.is_some() && self.access_key.is_some()
    }
}

struct AuthClient {
    session: Arc<RwLock<ClientSession>>,
    client: Option<AuthServiceClient<Channel>>,
    credentials: Credentials,
    connection_settings: ConnectionSettings,
    component_type: Component,
}

#[async_trait]
impl GrpcClient for AuthClient {
    async fn initialize(&mut self) -> Result<(), Box<dyn Error>> {
        let controller_settings = Component::Controller.get_connection_settings()?;

        let tls_config = ClientTlsConfig::new()
            .ca_certificate(tonic::transport::Certificate::from_pem(
                controller_settings.certificate.clone(),
            ))
            .domain_name(controller_settings.domain_name.clone());

        let channel = Channel::builder(controller_settings.get_public_endpoint())
            .tls_config(tls_config)?
            .connect()
            .await
            .map_err(|e: transport::Error| format!("Failed to connect to gRPC server: {}", e))?;

        self.client = Some(AuthServiceClient::new(channel));
        Ok(())
    }
}

#[async_trait]
#[automock]
pub trait Authenticator: GrpcClient {
    async fn login(&mut self) -> Result<(), Box<dyn Error>>;
    async fn logout(&mut self) -> Result<(), Box<dyn Error>>;
    async fn ping(&mut self) -> Result<(String, i64), Box<dyn Error>>;
    async fn get_session(&self) -> ClientSession;
    async fn is_authenticated(&self) -> bool;
}

#[async_trait]
impl Authenticator for AuthClient {
    async fn login(&mut self) -> Result<(), Box<dyn Error>> {
        let request = Request::new(LoginRequest {
            component_type: self.component_type.clone().into(),
            uid: self.credentials.uid.clone(),
            pwd: self.credentials.pwd.clone(),
            on_ip: self.connection_settings.ip.clone(),
            on_port: self.connection_settings.port as u32,
            public_key: self.connection_settings.certificate.clone(),
            domain_name: self.connection_settings.domain_name.clone(),
        });

        let response = self
            .client
            .as_mut()
            .unwrap()
            .login(request)
            .await
            .map_err(|status| format!("Login failed: {}", status))?
            .into_inner();

        let mut session = self.session.write().await;
        session.set_session(self.credentials.uid.clone(), response.access_key.clone());
        Ok(())
    }

    async fn logout(&mut self) -> Result<(), Box<dyn Error>> {
        let request = tonic::Request::new(LogoutRequest {
            access_key: self
                .session
                .read()
                .await
                .access_key
                .clone()
                .unwrap_or_default(),
        });

        self.client
            .as_mut()
            .unwrap()
            .logout(request)
            .await
            .map_err(|status| format!("Logout failed: {}", status))?;

        self.session = Arc::new(RwLock::new(ClientSession::default()));
        Ok(())
    }

    async fn ping(&mut self) -> Result<(String, i64), Box<dyn Error>> {
        let session = self.session.read().await;
        let request = Request::new(PingRequest {
            access_key: session.access_key.clone().unwrap_or_default(),
        });

        let response = self.client.as_mut().unwrap().ping(request).await?;
        let response = response.into_inner();
        Ok((response.status, response.timestamp))
    }

    async fn get_session(&self) -> ClientSession {
        self.session.read().await.clone()
    }

    async fn is_authenticated(&self) -> bool {
        let session = self.get_session().await;
        session.is_authenticated()
    }
}

impl AuthClient {
    pub fn new(session: Arc<RwLock<ClientSession>>, descriptor: &ComponentDescriptor) -> Self {
        Self {
            client: None,
            session,
            component_type: descriptor.into(),
            credentials: descriptor.get_credentials().clone(),
            connection_settings: descriptor.get_connection_settings().clone(),
        }
    }
}

pub struct AuthClientFactory;

#[automock]
pub trait AuthenticatorFactory: Send + Sync {
    fn get_authenticator(
        &self,
        session: Arc<RwLock<ClientSession>>,
        descriptor: &ComponentDescriptor,
    ) -> Box<dyn Authenticator>;
}

impl AuthenticatorFactory for AuthClientFactory {
    fn get_authenticator(
        &self,
        session: Arc<RwLock<ClientSession>>,
        descriptor: &ComponentDescriptor,
    ) -> Box<dyn Authenticator> {
        Box::new(AuthClient::new(session, descriptor))
    }
}

#[async_trait]
impl GrpcClient for MockAuthenticator {
    async fn initialize(&mut self) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
}
