use crate::models::SessionInfo;
use crate::storage::{self, RepositoryType, SessionRepository};
use std::net::SocketAddr;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub struct SessionManager {
    repository: Box<dyn SessionRepository>,
}

impl SessionManager {
    pub fn new(repository_type: RepositoryType, cancellation_token: CancellationToken) -> Self {
        Self {
            repository: storage::create_session_repository(repository_type, cancellation_token)
                .unwrap(),
        }
    }

    pub async fn set_session(
        &self,
        component_type: u8,
        uid: &str,
        client_ip: SocketAddr,
        on_ip_address: &str,
        on_port_number: u16,
    ) -> String {
        let access_key = Self::generate_access_key();
        let session_info = SessionInfo {
            access_key: access_key.clone(),
            uid: uid.to_string(),
            client_ip,
            component_type,
            on_ip_address: on_ip_address.to_string(),
            on_port_number,
        };

        self.repository.set_session(&session_info).await;
        access_key
    }

    pub async fn get_session(&self, access_key: &str) -> Option<SessionInfo> {
        self.repository.get_session(access_key).await
    }

    pub async fn remove_session(&self, access_key: &str) {
        self.repository.remove_session(access_key).await;
    }

    pub async fn get_proxies(&self, access_key: &str) -> Option<Vec<SessionInfo>> {
        self.repository.get_proxies(access_key).await
    }

    pub async fn get_client(&self, uid: &str) -> Option<SessionInfo> {
        self.repository.get_client(uid).await
    }

    pub async fn count_proxies(&self) -> usize {
        self.repository.count_proxies().await
    }

    pub async fn count_clients(&self) -> usize {
        self.repository.count_clients().await
    }

    pub async fn count_controllers(&self) -> usize {
        self.repository.count_controllers().await
    }

    fn generate_access_key() -> String {
        Uuid::new_v4().to_string()
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::storage::MockSessionRepository;
    use crosscutting::networking;

    const EXPECTED_UID: &str = "1234567890";
    const EXPECTED_IP: &str = "127.0.0.1";
    const EXPECTED_PORT: u16 = 8080;
    const EXPECTED_ACCESS_KEY: &str = "test_access_key";

    impl SessionManager {
        fn with_repository(repository: Box<dyn SessionRepository>) -> Self {
            Self { repository }
        }
    }

    impl PartialEq for SessionInfo {
        fn eq(&self, other: &Self) -> bool {
            self.access_key == other.access_key
                && self.uid == other.uid
                && self.client_ip == other.client_ip
                && self.component_type == other.component_type
                && self.on_ip_address == other.on_ip_address
                && self.on_port_number == other.on_port_number
        }
    }

    #[tokio::test]
    async fn new_creates_session_manager() {
        let cancellation_token = CancellationToken::new();
        _ = SessionManager::new(RepositoryType::InMemory, cancellation_token);
    }

    #[tokio::test]
    async fn set_session_creates_and_stores_session_returning_access_key() {
        let mut mock_repo = MockSessionRepository::new();
        mock_repo
            .expect_set_session()
            .withf(|session_info: &SessionInfo| {
                session_info.uid == EXPECTED_UID
                    && session_info.client_ip
                        == networking::to_socket_address(EXPECTED_IP, EXPECTED_PORT).unwrap()
                    && session_info.on_ip_address == EXPECTED_IP
                    && session_info.on_port_number == EXPECTED_PORT
            })
            .returning(|_| ());

        let session_manager = SessionManager::with_repository(Box::new(mock_repo));
        let access_key = session_manager
            .set_session(
                1,
                EXPECTED_UID,
                networking::to_socket_address(EXPECTED_IP, EXPECTED_PORT).unwrap(),
                EXPECTED_IP,
                EXPECTED_PORT,
            )
            .await;

        assert!(!access_key.is_empty());
    }

    #[tokio::test]
    async fn get_session_returns_session_info() {
        let expected_session_info = SessionInfo {
            access_key: EXPECTED_ACCESS_KEY.to_string(),
            uid: EXPECTED_UID.to_string(),
            client_ip: networking::to_socket_address(EXPECTED_IP, EXPECTED_PORT).unwrap(),
            component_type: 1,
            on_ip_address: EXPECTED_IP.to_string(),
            on_port_number: EXPECTED_PORT,
        };

        let ref_expected_session_info = expected_session_info.clone();
        let mut mock_repo = MockSessionRepository::new();
        mock_repo
            .expect_get_session()
            .withf(|key| key == EXPECTED_ACCESS_KEY)
            .returning(move |_| Some(expected_session_info.clone()));

        let session_manager = SessionManager::with_repository(Box::new(mock_repo));
        let session_info = session_manager
            .get_session(EXPECTED_ACCESS_KEY)
            .await
            .unwrap();

        assert_eq!(session_info, ref_expected_session_info);
    }

    #[tokio::test]
    async fn remove_session_removes_session() {
        let mut mock_repo = MockSessionRepository::new();
        mock_repo
            .expect_remove_session()
            .withf(|key| key == EXPECTED_ACCESS_KEY)
            .returning(|_| ());

        let session_manager = SessionManager::with_repository(Box::new(mock_repo));
        session_manager.remove_session(EXPECTED_ACCESS_KEY).await;
    }

    #[tokio::test]
    async fn get_proxies_returns_proxies() {
        let expected_proxies = vec![
            SessionInfo {
                access_key: "other key".to_string(),
                uid: "other uid".to_string(),
                client_ip: networking::to_socket_address(EXPECTED_IP, EXPECTED_PORT).unwrap(),
                component_type: 2,
                on_ip_address: EXPECTED_IP.to_string(),
                on_port_number: EXPECTED_PORT,
            },
            SessionInfo {
                access_key: EXPECTED_ACCESS_KEY.to_string(),
                uid: EXPECTED_UID.to_string(),
                client_ip: networking::to_socket_address(EXPECTED_IP, EXPECTED_PORT).unwrap(),
                component_type: 2,
                on_ip_address: EXPECTED_IP.to_string(),
                on_port_number: EXPECTED_PORT,
            },
        ];

        let ref_expected_proxies = expected_proxies[0..1].to_vec();
        let mut mock_repo = MockSessionRepository::new();
        mock_repo
            .expect_get_proxies()
            .withf(|key| key == EXPECTED_ACCESS_KEY)
            .returning(move |_| {
                Some(
                    expected_proxies
                        .iter()
                        .filter_map(|p| {
                            if p.access_key == EXPECTED_ACCESS_KEY {
                                None
                            } else {
                                Some(p.clone())
                            }
                        })
                        .collect(),
                )
            });

        let session_manager = SessionManager::with_repository(Box::new(mock_repo));
        let proxies = session_manager
            .get_proxies(EXPECTED_ACCESS_KEY)
            .await
            .unwrap();

        assert_eq!(proxies, ref_expected_proxies);
    }

    #[tokio::test]
    async fn get_client_returns_client() {
        let expected_client = SessionInfo {
            access_key: EXPECTED_ACCESS_KEY.to_string(),
            uid: EXPECTED_UID.to_string(),
            client_ip: networking::to_socket_address(EXPECTED_IP, EXPECTED_PORT).unwrap(),
            component_type: 1,
            on_ip_address: EXPECTED_IP.to_string(),
            on_port_number: EXPECTED_PORT,
        };

        let ref_expected_client = expected_client.clone();
        let mut mock_repo = MockSessionRepository::new();
        mock_repo
            .expect_get_client()
            .withf(|key| key == EXPECTED_UID)
            .returning(move |_| Some(expected_client.clone()));

        let session_manager = SessionManager::with_repository(Box::new(mock_repo));
        let client = session_manager.get_client(EXPECTED_UID).await.unwrap();

        assert_eq!(client, ref_expected_client);
    }

    #[tokio::test]
    async fn count_proxies_returns_count() {
        let expected_count = 5;

        let mut mock_repo = MockSessionRepository::new();
        mock_repo
            .expect_count_proxies()
            .returning(move || expected_count);

        let session_manager = SessionManager::with_repository(Box::new(mock_repo));
        let count = session_manager.count_proxies().await;

        assert_eq!(count, expected_count);
    }

    #[tokio::test]
    async fn count_clients_returns_count() {
        let expected_count = 10;

        let mut mock_repo = MockSessionRepository::new();
        mock_repo
            .expect_count_clients()
            .returning(move || expected_count);

        let session_manager = SessionManager::with_repository(Box::new(mock_repo));
        let count = session_manager.count_clients().await;

        assert_eq!(count, expected_count);
    }

    #[tokio::test]
    async fn count_controllers_returns_count() {
        let expected_count = 3;

        let mut mock_repo = MockSessionRepository::new();
        mock_repo
            .expect_count_controllers()
            .returning(move || expected_count);

        let session_manager = SessionManager::with_repository(Box::new(mock_repo));
        let count = session_manager.count_controllers().await;

        assert_eq!(count, expected_count);
    }
}
