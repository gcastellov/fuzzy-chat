use crosscutting::{Component, abstractions::GrpcClient};
use mockall::automock;
use std::error::Error;
use tonic::{
    Request, async_trait,
    transport::{Channel, ClientTlsConfig},
};

pub mod route {
    tonic::include_proto!("route");
}

use route::{
    InitRequest, InitResponse, RedeemRequest, RedeemResponse, RouteRequest, RouteResponse,
    route_service_client::RouteServiceClient,
};

#[async_trait]
#[automock]
pub trait Router: GrpcClient {
    async fn init_conversation(
        &mut self,
        access_key: String,
        to: String,
    ) -> Result<InitResponse, Box<dyn Error>>;

    async fn get_route(
        &mut self,
        conversation_id: String,
        access_key: String,
    ) -> Result<RouteResponse, Box<dyn Error>>;

    async fn redeem(
        &mut self,
        conversation_id: String,
        access_key: String,
        nonce: String,
    ) -> Result<RedeemResponse, Box<dyn Error>>;
}

#[derive(Default)]
struct RouteClient {
    client: Option<RouteServiceClient<Channel>>,
}

#[async_trait]
impl GrpcClient for RouteClient {
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

        self.client = Some(RouteServiceClient::new(channel));
        Ok(())
    }
}

#[async_trait]
impl Router for RouteClient {
    async fn init_conversation(
        &mut self,
        access_key: String,
        to: String,
    ) -> Result<InitResponse, Box<dyn Error>> {
        let request = InitRequest { access_key, to };

        let response = self
            .client
            .as_mut()
            .unwrap()
            .initialize(Request::new(request))
            .await
            .map_err(|status| format!("Impossible to initialize the conversation: {}", status))?;

        Ok(response.into_inner())
    }

    async fn get_route(
        &mut self,
        conversation_id: String,
        access_key: String,
    ) -> Result<RouteResponse, Box<dyn Error>> {
        let request = RouteRequest {
            access_key,
            conversation_id,
        };

        match self
            .client
            .as_mut()
            .unwrap()
            .route(Request::new(request))
            .await
        {
            Ok(response) => {
                let response = response.into_inner();
                Ok(response)
            }
            Err(status) => Err(format!("Impossible to get a route: {}", status).into()),
        }
    }

    async fn redeem(
        &mut self,
        conversation_id: String,
        access_key: String,
        nonce: String,
    ) -> Result<RedeemResponse, Box<dyn Error>> {
        let request = RedeemRequest {
            access_key,
            conversation_id,
            nonce,
        };

        match self
            .client
            .as_mut()
            .unwrap()
            .redeem(Request::new(request))
            .await
        {
            Ok(response) => {
                let response = response.into_inner();
                Ok(response)
            }
            Err(_) => Err("Impossible to redeem a route".into()),
        }
    }
}

#[derive(Default)]
pub struct RouteClientFactory;

#[automock]
pub trait RouterFactory: Send + Sync {
    fn get_router(&self) -> Box<dyn Router>;
}

impl RouterFactory for RouteClientFactory {
    fn get_router(&self) -> Box<dyn Router> {
        Box::new(RouteClient::default())
    }
}

#[async_trait]
impl GrpcClient for MockRouter {
    async fn initialize(&mut self) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
}
