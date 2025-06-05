use super::*;
use crate::models::info_proto::{
    StatusRequest, StatusResponse, info_service_client::InfoServiceClient,
};
use crosscutting::{abstractions::GrpcClient, networking, settings};
use std::error::Error;
use std::fs;
use std::path::PathBuf;
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
        let cert_path = PathBuf::from(settings::environment::get_certificates_dir());
        let cert_path = cert_path.join("ca.crt");
        let ca_cert = fs::read(cert_path)?;

        let domain_name = settings::service::get_controller_domain_name()?;
        let tls_config = ClientTlsConfig::new()
            .ca_certificate(tonic::transport::Certificate::from_pem(ca_cert))
            .domain_name(domain_name);

        let (ip, port) = settings::service::get_controller_connection_settings()?;
        let channel_endpoint = networking::to_https_endpoint(&ip, port as u32)?;
        let channel = Channel::builder(channel_endpoint)
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
