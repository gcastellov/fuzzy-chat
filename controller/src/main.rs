mod membership;
mod models;
mod routing;
mod services;
mod session;
mod storage;

use crosscutting::{Component, ComponentDescriptor, settings::environment, settings::logging};
use log::{debug, error};
use membership::MemberManager;
use routing::RouteManager;
use session::SessionManager;
use std::{error::Error, sync::Arc, time::Duration};
use storage::RepositoryType;
use tokio::{signal, time::sleep};
use tokio_util::sync::CancellationToken;

const EXPIRATION_SESSION_TIME: Duration = Duration::from_secs(2);
const MEMBERS_CSV_FILE_KEY: &str = "MEMBERS_CSV_FILE";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let log_filename = logging::get_default_log_file_name("controller");
    logging::setup_logger(&log_filename)?;

    debug!("Starting Controller component...");

    let descriptor: ComponentDescriptor = ComponentDescriptor::load(Component::Controller)?;
    let cancellation_token = CancellationToken::new();
    let (session_manager, route_manager, member_manager) =
        create_domain_components(&cancellation_token);

    initialize(
        descriptor.clone(),
        Arc::clone(&member_manager),
        Arc::clone(&session_manager),
        cancellation_token.child_token(),
    )
    .await?;

    let server_handle =
        services::start_server_handler(descriptor, session_manager, route_manager, member_manager);

    _ = signal::ctrl_c().await;
    debug!("Received shutdown signal, terminating gracefully...");

    cancellation_token.cancel();
    server_handle.abort();
    _ = server_handle.await;

    Ok(())
}

async fn initialize(
    descriptor: ComponentDescriptor,
    member_manager: Arc<MemberManager>,
    session_manager: Arc<SessionManager>,
    cancellation_token: CancellationToken,
) -> Result<(), Box<dyn Error>> {
    let file_path = environment::get_env_variable(MEMBERS_CSV_FILE_KEY).unwrap();
    if let Err(e) = member_manager.seed_members_from_csv(&file_path).await {
        error!("Failed to seed members: {}", e);
        return Err("Initialization failed".into());
    }

    tokio::spawn(async move {
        let credentials = descriptor.get_credentials();
        let connection_settings = descriptor.get_connection_settings();

        while !cancellation_token.is_cancelled() {
            session_manager
                .set_session(
                    &credentials.uid,
                    Component::Controller,
                    &connection_settings.get_public_socket_address(),
                    connection_settings,
                )
                .await;

            sleep(EXPIRATION_SESSION_TIME).await;
        }
    });

    Ok(())
}

fn create_domain_components(
    cancellation_token: &CancellationToken,
) -> (Arc<SessionManager>, Arc<RouteManager>, Arc<MemberManager>) {
    let repository_type = RepositoryType::get_from_env();
    let member_manager = Arc::new(MemberManager::new(repository_type));
    let route_manager = Arc::new(RouteManager::new(
        repository_type,
        cancellation_token.child_token(),
    ));
    let session_manager = Arc::new(SessionManager::new(
        repository_type,
        cancellation_token.child_token(),
    ));

    (session_manager, route_manager, member_manager)
}
