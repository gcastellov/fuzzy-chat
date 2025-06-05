pub mod proxy_service;

use crate::models::proxy_proto::proxy_service_server::ProxyServiceServer;
use authorization::auth_client::Authenticator;
use crosscutting::tracing;
use log::{debug, error};
use proxy_service::ProxyServiceImpl;
use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tonic::transport::Server;
use tonic::{Request, Response, Status};

pub struct ProxyGrpcServer {
    authenticator: Arc<RwLock<Box<dyn Authenticator>>>,
    socket_address: SocketAddr,
}

impl ProxyGrpcServer {
    pub fn new(
        authenticator: Arc<RwLock<Box<dyn Authenticator>>>,
        socket_address: SocketAddr,
    ) -> Self {
        Self {
            authenticator,
            socket_address,
        }
    }

    pub async fn start(&self) -> Result<(), Box<dyn Error>> {
        let proxy_service = ProxyServiceImpl::new(Arc::clone(&self.authenticator));

        Server::builder()
            .layer(tracing::UriTracingLayer)
            .add_service(ProxyServiceServer::new(proxy_service))
            .serve(self.socket_address)
            .await?;

        Ok(())
    }
}

pub fn start_server_handler(
    socket_address: SocketAddr,
    authenticator: Arc<RwLock<Box<dyn Authenticator>>>,
) -> tokio::task::JoinHandle<()> {
    let grpc_server = ProxyGrpcServer::new(authenticator, socket_address);
    tokio::spawn(async move {
        if let Err(e) = grpc_server.start().await {
            error!("gRPC server error: {}", e);
        }
    })
}
