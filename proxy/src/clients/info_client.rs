use super::*;
use crate::models::info_proto::{
    StatusRequest, StatusResponse, info_service_client::InfoServiceClient,
};
use crosscutting::{Component, abstractions::GrpcClient};
use std::error::Error;
use tonic::{
    Request,
    transport::{Channel, ClientTlsConfig},
};

#[async_trait]
#[automock]
pub trait Informer: GrpcClient {
    async fn get_status(&mut self, access_key: String) -> Result<StatusResponse, Box<dyn Error>>;
}

#[derive(Default)]
struct InfoClient {
    client: Option<InfoServiceClient<Channel>>,
}

#[async_trait]
impl Informer for InfoClient {
    async fn get_status(&mut self, access_key: String) -> Result<StatusResponse, Box<dyn Error>> {
        let request = StatusRequest { access_key };
        let response = self
            .client
            .as_mut()
            .unwrap()
            .status(Request::new(request))
            .await?;
        Ok(response.into_inner())
    }
}

#[async_trait]
impl GrpcClient for InfoClient {
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
            .await?;

        self.client = Some(InfoServiceClient::new(channel));

        Ok(())
    }
}

#[derive(Default)]
pub struct InfoClientFactory;

#[automock]
pub trait InformerFactory: Send + Sync {
    fn get_informer(&self) -> Box<dyn Informer>;
}

impl InformerFactory for InfoClientFactory {
    fn get_informer(&self) -> Box<dyn Informer> {
        Box::new(InfoClient::default())
    }
}

#[async_trait]
impl GrpcClient for MockInformer {
    async fn initialize(&mut self) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
}
