use crosscutting::{abstractions::GrpcClient, networking, settings};
use log::debug;
use mockall::automock;
use std::{error::Error, fs, path::PathBuf, sync::Arc};
use tokio::sync::RwLock;
use tonic::{
    Request, async_trait,
    transport::{self, Channel, ClientTlsConfig},
};

use super::auth_proto::{
    ComponentType, LoginRequest, LogoutRequest, PingRequest, auth_service_client::AuthServiceClient,
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
    uid: String,
    pwd: String,
    on_ip: String,
    on_port: u16,
    component_type: ComponentType,
}

#[async_trait]
impl GrpcClient for AuthClient {
    async fn initialize(&mut self) -> Result<(), Box<dyn Error>> {
        let cert_path = PathBuf::from(settings::environment::get_certificates_dir());
        let cert_path = cert_path.join("ca.crt");
        let ca_cert = fs::read(cert_path)?;

        let domain_name = settings::service::get_controller_domain_name()?;
        let (ip, port) = settings::service::get_controller_connection_settings()?;
        let channel_endpoint = networking::to_https_endpoint(&ip, port as u32)?;
        let tls_config = ClientTlsConfig::new()
            .ca_certificate(tonic::transport::Certificate::from_pem(ca_cert))
            .domain_name(domain_name);

        debug!("Connecting to gRPC server at: {}", channel_endpoint);

        let channel = Channel::builder(channel_endpoint)
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
            component_type: self.component_type as i32,
            uid: self.uid.clone(),
            pwd: self.pwd.clone(),
            on_ip: self.on_ip.clone(),
            on_port: self.on_port as u32,
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
        session.set_session(self.uid.clone(), response.access_key.clone());
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
    pub fn new(
        session: Arc<RwLock<ClientSession>>,
        descriptor: &settings::component::Descriptor,
    ) -> Self {
        Self {
            client: None,
            component_type: ComponentType::try_from(descriptor.component_type as i32).unwrap(),
            session,
            uid: descriptor.uid.clone(),
            pwd: descriptor.pwd.clone(),
            on_ip: descriptor.on_ip.clone(),
            on_port: descriptor.on_port,
        }
    }
}

pub struct AuthClientFactory;

#[automock]
pub trait AuthenticatorFactory: Send + Sync {
    fn get_authenticator(
        &self,
        session: Arc<RwLock<ClientSession>>,
        descriptor: &settings::component::Descriptor,
    ) -> Box<dyn Authenticator>;
}

impl AuthenticatorFactory for AuthClientFactory {
    fn get_authenticator(
        &self,
        session: Arc<RwLock<ClientSession>>,
        descriptor: &settings::component::Descriptor,
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
