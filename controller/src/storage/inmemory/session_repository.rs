use super::ExpirationWrapper;
use crate::models::{SessionInfo, auth_proto::ComponentType};
use crate::storage;
use crate::storage::SessionRepository;
use log::debug;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;
use tokio::time::Duration;
use tokio_util::sync::CancellationToken;
use tonic::async_trait;

const EXPIRATION_TIME_CHECK: Duration = Duration::from_millis(5000);

type SessionsCollection = HashMap<String, ExpirationWrapper<SessionInfo>>;
type ClientsCollection = HashMap<String, String>;
type ControllersCollection = HashMap<String, String>;
type ProxiesCollection = HashMap<String, String>;

pub struct InMemoryRepository {
    sessions: Arc<RwLock<SessionsCollection>>,
    clients: Arc<RwLock<ClientsCollection>>,
    controllers: Arc<RwLock<ControllersCollection>>,
    proxies: Arc<RwLock<ProxiesCollection>>,
    _handle: tokio::task::JoinHandle<()>,
}

impl InMemoryRepository {
    pub fn new(cancellation_token: CancellationToken) -> Self {
        let sessions = Arc::new(RwLock::new(HashMap::new()));
        let clients = Arc::new(RwLock::new(HashMap::new()));
        let controllers = Arc::new(RwLock::new(HashMap::new()));
        let proxies = Arc::new(RwLock::new(HashMap::new()));

        Self {
            sessions: Arc::clone(&sessions),
            clients: Arc::clone(&clients),
            controllers: Arc::clone(&controllers),
            proxies: Arc::clone(&proxies),
            _handle: Self::kill_expired_sessions(
                Arc::clone(&sessions),
                Arc::clone(&clients),
                Arc::clone(&controllers),
                Arc::clone(&proxies),
                cancellation_token,
            ),
        }
    }

    fn kill_expired_sessions(
        sessions: Arc<RwLock<SessionsCollection>>,
        clients: Arc<RwLock<ClientsCollection>>,
        controllers: Arc<RwLock<ControllersCollection>>,
        proxies: Arc<RwLock<ProxiesCollection>>,
        cancellation_token: CancellationToken,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            while !cancellation_token.is_cancelled() {
                tokio::time::sleep(EXPIRATION_TIME_CHECK).await;
                debug!("Checking for expired sessions...");

                let mut sessions = sessions.write().await;
                let mut clients = clients.write().await;
                let mut controllers = controllers.write().await;
                let mut proxies = proxies.write().await;

                let expired_sessions: Vec<(String, String)> = sessions
                    .iter()
                    .filter(|(_, wrapper)| wrapper.is_expired())
                    .map(|(_, wrapper)| {
                        (
                            wrapper.value.access_key.to_owned(),
                            wrapper.value.uid.to_owned(),
                        )
                    })
                    .collect();

                if !expired_sessions.is_empty() {
                    expired_sessions.iter().for_each(|(access_key, uid)| {
                        sessions.remove(access_key.as_str());
                        clients.remove(uid.as_str());
                        controllers.remove(uid.as_str());
                        proxies.remove(uid.as_str());
                    });
                }
            }

            debug!("Expired sessions check terminated.");
        })
    }
}

#[async_trait]
impl SessionRepository for InMemoryRepository {
    async fn set_session(&self, session_info: &SessionInfo) {
        let comonent_type = ComponentType::from(session_info.component_type);

        match comonent_type {
            ComponentType::Client => {
                let mut clients = self.clients.write().await;
                clients.insert(session_info.uid.clone(), session_info.access_key.clone());
            }
            ComponentType::Proxy => {
                let mut proxies = self.proxies.write().await;
                proxies.insert(session_info.uid.clone(), session_info.access_key.clone());
            }
            ComponentType::Controller => {
                let mut controllers = self.controllers.write().await;
                controllers.insert(session_info.uid.clone(), session_info.access_key.clone());
            }
        }

        let mut sessions = self.sessions.write().await;
        sessions.insert(
            session_info.access_key.clone(),
            ExpirationWrapper::new(session_info.to_owned(), storage::SESSIONS_EXPIRATION_TIME),
        );
    }

    async fn get_session(&self, access_key: &str) -> Option<SessionInfo> {
        let mut sessions = self.sessions.write().await;
        sessions.get_mut(access_key).map(|session| {
            session.renew();
            session.value.clone()
        })
    }

    async fn remove_session(&self, access_key: &str) {
        let mut sessions = self.sessions.write().await;

        if let Some(wrapper) = sessions.remove(access_key) {
            let mut clients = self.clients.write().await;
            clients.remove(wrapper.value.uid.as_str());
        }
    }

    async fn get_proxies(&self, access_key: &str) -> Option<Vec<SessionInfo>> {
        let proxies = self.proxies.read().await;
        let sessions = self.sessions.read().await;

        let result: Vec<SessionInfo> = proxies
            .iter()
            .filter_map(|(_, key)| {
                if *key == access_key {
                    None
                } else {
                    sessions.get(key).map(|session| session.value.clone())
                }
            })
            .collect();

        if result.is_empty() {
            return None;
        }

        Some(result)
    }

    async fn get_client(&self, uid: &str) -> Option<SessionInfo> {
        let clients = self.clients.read().await;
        if let Some(client) = clients.get(uid) {
            let sessions = self.sessions.read().await;
            if let Some(wrapper) = sessions.get(client) {
                return Some(wrapper.value.clone());
            }
        }

        None
    }

    async fn count_proxies(&self) -> usize {
        let sessions = self.proxies.read().await;
        sessions.iter().count()
    }

    async fn count_clients(&self) -> usize {
        let sessions = self.clients.read().await;
        sessions.iter().count()
    }

    async fn count_controllers(&self) -> usize {
        let sessions = self.controllers.read().await;
        sessions.iter().count()
    }
}
