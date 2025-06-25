use super::*;
use crosscutting::abstractions::GrpcClient;
use std::error::Error;
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
    public_key: Vec<u8>,
    domain_name: String,
    client: Option<LandingServiceClient<Channel>>,
}

impl LandingClient {
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
        let tls_config = ClientTlsConfig::new()
            .ca_certificate(tonic::transport::Certificate::from_pem(
                self.public_key.clone(),
            ))
            .domain_name(self.domain_name.clone());

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
    fn get_lander(
        &self,
        endpoint: Uri,
        public_key: Vec<u8>,
        domain_name: String,
    ) -> Box<dyn Lander>;
}

impl LanderFactory for LandingClientFactory {
    fn get_lander(
        &self,
        endpoint: Uri,
        public_key: Vec<u8>,
        domain_name: String,
    ) -> Box<dyn Lander> {
        Box::new(LandingClient::new(endpoint, public_key, domain_name))
    }
}
