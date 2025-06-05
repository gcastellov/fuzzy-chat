use super::*;
use crosscutting::{abstractions::GrpcClient, settings};
use std::{error::Error, fs, path::PathBuf};
use tonic::{
    Request,
    transport::{Channel, ClientTlsConfig, Uri},
};

pub mod client {
    tonic::include_proto!("client");
}

use client::{TextRequest, TextResponse, landing_service_client::LandingServiceClient};

#[async_trait]
#[automock]
pub trait Lander: GrpcClient {
    async fn send_message(
        &mut self,
        conversation_id: String,
        access_key: String,
        nonce: String,
        content: Vec<u8>,
    ) -> Result<TextResponse, Box<dyn Error>>;
}

struct LandingClient {
    endpoint: Uri,
    client: Option<LandingServiceClient<Channel>>,
}

impl LandingClient {
    pub fn new(endpoint: Uri) -> Self {
        Self {
            endpoint,
            client: None,
        }
    }
}

#[async_trait]
impl Lander for LandingClient {
    async fn send_message(
        &mut self,
        conversation_id: String,
        access_key: String,
        nonce: String,
        content: Vec<u8>,
    ) -> Result<TextResponse, Box<dyn Error>> {
        let request = TextRequest {
            conversation_id,
            access_key,
            nonce,
            content,
        };

        let response = self
            .client
            .as_mut()
            .unwrap()
            .receive(Request::new(request))
            .await?;
        Ok(response.into_inner())
    }
}

#[async_trait]
impl GrpcClient for LandingClient {
    async fn initialize(&mut self) -> Result<(), Box<dyn Error>> {
        let cert_path = PathBuf::from(settings::environment::get_certificates_dir());
        let cert_path = cert_path.join("ca.crt");
        let ca_cert = fs::read(cert_path)?;

        let domain_name = settings::service::get_controller_domain_name()?;
        let tls_config = ClientTlsConfig::new()
            .ca_certificate(tonic::transport::Certificate::from_pem(ca_cert))
            .domain_name(domain_name);

        let channel = Channel::builder(self.endpoint.clone())
            .tls_config(tls_config)?
            .connect()
            .await?;

        self.client = Some(LandingServiceClient::new(channel));
        Ok(())
    }
}

#[async_trait]
impl GrpcClient for MockLander {
    async fn initialize(&mut self) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
}

#[derive(Default)]
pub struct LandingClientFactory;

#[automock]
pub trait LanderFactory: Send + Sync {
    fn get_lander(&self, endpoint: Uri) -> Box<dyn Lander>;
}

impl LanderFactory for LandingClientFactory {
    fn get_lander(&self, endpoint: Uri) -> Box<dyn Lander> {
        Box::new(LandingClient::new(endpoint))
    }
}
