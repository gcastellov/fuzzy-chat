use crate::auth_client::Authenticator;
use log::{debug, info, warn};
use std::error::Error;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, Instant, sleep};
use tokio_util::sync::CancellationToken;

const PING_INTERVAL: Duration = Duration::from_millis(5000);
const AUTH_DELAY: Duration = Duration::from_millis(1000);

struct AuthManager {
    auth_client: Arc<RwLock<Box<dyn Authenticator>>>,
    cancellation_token: CancellationToken,
}

impl AuthManager {
    fn new(
        auth_client: Arc<RwLock<Box<dyn Authenticator>>>,
        cancellation_token: CancellationToken,
    ) -> Self {
        Self {
            auth_client,
            cancellation_token,
        }
    }

    async fn authenticate(&mut self) -> Result<String, Box<dyn Error>> {
        let mut auth_client = self.auth_client.write().await;

        auth_client
            .login()
            .await
            .map_err(|_| "Impossible to login")?;

        let session = auth_client.get_session().await;
        if !auth_client.is_authenticated().await {
            return Err("Not authenticated".into());
        }

        Ok(session.access_key.unwrap())
    }

    async fn keep_alive(&mut self) -> Result<(), Box<dyn Error>> {
        let mut last_ping = Instant::now();

        while !self.cancellation_token.is_cancelled() {
            let next_ping_in = get_next_ping_duration(&last_ping);

            sleep(next_ping_in).await;

            let mut auth_client = self.auth_client.write().await;

            match auth_client.ping().await {
                Ok((status, timestamp)) => {
                    debug!(
                        "Ping successful: Status={}, Timestamp={}",
                        status, timestamp
                    );
                    last_ping = Instant::now();
                }
                Err(e) => {
                    debug!("Ping failed: {}", e);
                    return Err(e);
                }
            }
        }

        Ok(())
    }

    async fn logout(&mut self) -> Result<(), Box<dyn Error>> {
        let mut auth_client = self.auth_client.write().await;
        auth_client.logout().await?;
        Ok(())
    }
}

pub fn start_auth_handler(
    authenticator: Arc<RwLock<Box<dyn Authenticator>>>,
    cancellation_token: CancellationToken,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut auth_manager = AuthManager::new(authenticator, cancellation_token.child_token());

        while !cancellation_token.is_cancelled() {
            if let Ok(access_key) = auth_manager.authenticate().await {
                debug!("Currently logged in with: {}", access_key);
            } else {
                sleep(AUTH_DELAY).await;
                warn!("Authentication failed. Retrying ...");
                continue;
            }

            if auth_manager.keep_alive().await.is_ok() {
                debug!("Operation has been cancelled");
                break;
            }
        }

        debug!("Logging out ...");
        match auth_manager.logout().await {
            Ok(_) => info!("Successfully logged out"),
            Err(e) => info!("Error during logout: {}", e),
        }
    })
}

fn get_next_ping_duration(last_ping: &Instant) -> Duration {
    let elapsed = last_ping.elapsed();
    if elapsed >= PING_INTERVAL {
        Duration::ZERO
    } else {
        PING_INTERVAL - elapsed
    }
}
