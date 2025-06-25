mod command;
mod models;
mod services;

use authorization::auth::start_auth_handler;
use authorization::auth_client::AuthClientFactory;
use authorization::auth_client::AuthenticatorFactory;
use authorization::auth_client::ClientSession;
use command::Command;
use command::Commander;
use crosscutting::{Component, ComponentDescriptor, settings::logging};
use log::{debug, error, warn};
use models::TextMessage;
use routing::proxy_client::proxy::CommandResponse;
use std::error::Error;
use std::sync::Arc;
use std::thread;
use tokio::signal;
use tokio::sync::RwLock;
use tokio::time::Duration;
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let log_filename = logging::get_default_log_file_name("client");
    logging::setup_logger(&log_filename)?;
    debug!("Starting Client component...");

    let descriptor = ComponentDescriptor::load(Component::Client)?;
    let socket_address = descriptor
        .get_connection_settings()
        .get_local_socket_address();
    let (tx, rx) = tokio::sync::mpsc::channel::<TextMessage>(100);
    start_listener_handler(rx);
    let server_handle = services::start_server_handler(socket_address, tx);
    let client_session = Arc::new(RwLock::new(ClientSession::default()));
    let cmd_session = Arc::clone(&client_session);
    let cancellation_token = CancellationToken::new();
    let authenticator_factory = AuthClientFactory {};
    let mut authenticator = authenticator_factory.get_authenticator(client_session, &descriptor);
    authenticator.initialize().await?;
    let authenticator = Arc::new(RwLock::new(authenticator));

    let auth_handler = start_auth_handler(authenticator, cancellation_token.child_token());

    let (sender, receiver) = std::sync::mpsc::channel();
    start_input_thread(sender);

    let cmd_handler = start_cmd_handler(cmd_session, receiver, cancellation_token.child_token());

    _ = signal::ctrl_c().await;
    debug!("Received shutdown signal, terminating gracefully...");

    cancellation_token.cancel();
    server_handle.abort();
    _ = auth_handler.await;
    _ = server_handle.await;
    _ = cmd_handler.await;

    Ok(())
}

fn start_cmd_handler(
    client_session: Arc<RwLock<ClientSession>>,
    receiver: std::sync::mpsc::Receiver<String>,
    cancellation_token: CancellationToken,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            if cancellation_token.is_cancelled() {
                break;
            }

            if let Ok(input) = receiver.recv_timeout(Duration::from_millis(500)) {
                let cmd = Command::from_str(&input);
                if cmd.is_err() {
                    warn!("Invalid command: {}", cmd.err().unwrap());
                    continue;
                }

                let access_key = client_session.read().await.access_key.to_owned();
                if access_key.is_none() {
                    error!("Access key is not set. Exiting...");
                    return;
                }

                let mut commander = Commander::new(access_key.unwrap());
                let response: Result<CommandResponse, Box<dyn Error>> = match cmd.unwrap() {
                    Command::Status => commander.get_status().await,
                    Command::Send(to, content) => commander.send_message(&to, &content).await,
                };

                if let Ok(response) = response {
                    debug!("Command response: {:?}", response);
                } else {
                    warn!("Error executing command: {}", response.err().unwrap());
                }
            }
        }
    })
}

fn start_listener_handler(mut rx: tokio::sync::mpsc::Receiver<TextMessage>) {
    tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            println!("Message received: {:?}", message);
        }
    });
}

fn start_input_thread(sender: std::sync::mpsc::Sender<String>) {
    thread::spawn(move || {
        let stdin = std::io::stdin();
        loop {
            let mut buffer = String::new();
            if stdin.read_line(&mut buffer).is_ok() {
                let cmd = buffer.trim();
                if cmd.is_empty() {
                    continue;
                }

                debug!("Received command: {}", cmd);
                _ = sender.send(buffer.clone());
            }
        }
    });
}
