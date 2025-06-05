mod clients;
mod models;
mod services;

use authorization::auth::start_auth_handler;
use authorization::auth_client::{AuthClientFactory, AuthenticatorFactory, ClientSession};
use crosscutting::settings;
use log::{debug, info};
use models::auth_proto::ComponentType;
use std::error::Error;
use std::sync::Arc;
use tokio::signal;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let log_filename = settings::logging::get_default_log_file_name("proxy");
    settings::logging::setup_logger(&log_filename)?;
    debug!("Starting proxy component...");

    let descriptor = settings::component::DescriptorBuilder::load()?
        .with_component_type(ComponentType::Proxy as u8)
        .with_version("1.0.0")
        .build()?;

    let client_session: Arc<RwLock<ClientSession>> =
        Arc::new(RwLock::new(ClientSession::default()));
    let authenticator_factory = AuthClientFactory {};
    let mut authenticator = authenticator_factory.get_authenticator(client_session, &descriptor);
    authenticator.initialize().await?;
    let authenticator = Arc::new(RwLock::new(authenticator));
    let cancellation_token = CancellationToken::new();

    let auth_handle =
        start_auth_handler(Arc::clone(&authenticator), cancellation_token.child_token());

    let socket_address = descriptor.on_local_socket_address();
    info!("Starting gRPC server on {}...", socket_address);
    let server_handle = services::start_server_handler(socket_address, authenticator);
    debug!("Press Ctrl+C to exit gracefully");
    _ = signal::ctrl_c().await;
    debug!("Received shutdown signal, terminating gracefully...");

    cancellation_token.cancel();
    server_handle.abort();
    _ = auth_handle.await;
    _ = server_handle.await;

    info!("Proxy has been shut down gracefully");
    Ok(())
}
