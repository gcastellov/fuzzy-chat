pub mod landing_service;
use crate::models::{TextMessage, client_proto::landing_service_server::LandingServiceServer};
use crosscutting::{settings::service, tracing};
use landing_service::LandingServiceImpl;
use log::{error, info};
use std::error::Error;
use std::net::SocketAddr;
use tokio::sync::mpsc::Sender;
use tonic::transport::{Server, ServerTlsConfig};
use tonic::{Request, Response, Status};

pub struct ClientGrpcServer {
    socket_address: SocketAddr,
    tx: Sender<TextMessage>,
}

impl ClientGrpcServer {
    pub fn new(socket_address: SocketAddr, tx: Sender<TextMessage>) -> Self {
        Self { socket_address, tx }
    }

    pub async fn start(&self) -> Result<(), Box<dyn Error>> {
        let landing_service = LandingServiceImpl::new(self.tx.clone());
        let identity = service::load_tls_identity("server.crt", "server.key").unwrap();
        let tls_config = ServerTlsConfig::new().identity(identity);

        Server::builder()
            .tls_config(tls_config)
            .unwrap()
            .layer(tracing::UriTracingLayer)
            .add_service(LandingServiceServer::new(landing_service))
            .serve(self.socket_address)
            .await?;

        Ok(())
    }
}

pub fn start_server_handler(
    socket_address: SocketAddr,
    tx: Sender<TextMessage>,
) -> tokio::task::JoinHandle<()> {
    info!("Starting gRPC server on {}...", socket_address);
    let grpc_server = ClientGrpcServer::new(socket_address, tx);
    tokio::spawn(async move {
        if let Err(e) = grpc_server.start().await {
            error!("gRPC server error: {}", e);
        }
    })
}
