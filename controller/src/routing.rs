use crate::models::{Conversation, Route, SessionInfo};
use crate::storage::{self, RepositoryType, RouteRepository};
use crosscutting::ConnectionSettings;
use mockall::automock;
use rand::seq::IteratorRandom;
use tokio_util::sync::CancellationToken;
use tonic::async_trait;
use uuid::Uuid;

#[automock]
#[async_trait]
trait RouteStrategy: Sync + Send {
    fn get_id(&self) -> u8;
    fn has_reached_final_route(&self, conversation: &Conversation) -> bool;
    async fn get_next_route(
        &self,
        conversation: &Conversation,
        proxies: &[SessionInfo],
    ) -> Option<SessionInfo>;
}

#[derive(Default)]
struct RandomRouteStrategy;

struct RouteStrategyFactory {
    strategies: Vec<Box<dyn RouteStrategy>>,
}

pub struct RouteManager {
    repository: Box<dyn RouteRepository>,
    route_strategy_factory: RouteStrategyFactory,
}

#[async_trait]
impl RouteStrategy for RandomRouteStrategy {
    fn get_id(&self) -> u8 {
        1
    }

    fn has_reached_final_route(&self, conversation: &Conversation) -> bool {
        conversation.routes.len() == 3
    }

    async fn get_next_route(
        &self,
        _: &Conversation,
        proxies: &[SessionInfo],
    ) -> Option<SessionInfo> {
        let numbers: Vec<usize> = (0..proxies.len() - 1).collect();
        let index = tokio::task::spawn_blocking(move || {
            let mut rng = rand::rng();
            numbers.into_iter().choose(&mut rng).unwrap_or_default()
        })
        .await
        .unwrap();

        proxies.get(index).cloned()
    }
}

impl RouteStrategyFactory {
    fn new() -> Self {
        Self {
            strategies: vec![Box::new(RandomRouteStrategy)],
        }
    }

    fn get_routing_id(&self, _from: &str, _to: &str) -> u8 {
        // TODO: Detect routing strategy
        self.strategies.first().unwrap().get_id()
    }

    fn get_strategy(&self, conversation: &Conversation) -> &dyn RouteStrategy {
        self.strategies
            .iter()
            .find(|strategy| strategy.get_id() == conversation.routing_id)
            .unwrap()
            .as_ref()
    }
}

impl RouteManager {
    pub fn new(repository_type: RepositoryType, cancellation_token: CancellationToken) -> Self {
        Self {
            route_strategy_factory: RouteStrategyFactory::new(),
            repository: storage::create_route_repository(repository_type, cancellation_token)
                .unwrap(),
        }
    }

    pub async fn initialize(&self, from: &str, to: &str) -> Option<String> {
        let conversation_id = Self::create_conversation_id();
        let routing_id = self.route_strategy_factory.get_routing_id(from, to);
        let conversation = Conversation::new(
            conversation_id,
            from.to_string(),
            to.to_string(),
            routing_id,
        );
        self.repository.set_conversation(&conversation).await
    }

    pub async fn finalize(&self, conversation_id: &str) {
        self.repository.remove_conversation(conversation_id).await;
    }

    pub async fn get_conversation(&self, conversation_id: &str) -> Option<Conversation> {
        self.repository.get_conversation(conversation_id).await
    }

    pub async fn store_route(
        &self,
        conversation_id: &str,
        connection_settings: &ConnectionSettings,
        end_route: bool,
    ) -> Option<String> {
        let route = Route {
            on_ip_address: connection_settings.ip.clone(),
            on_port_number: connection_settings.port,
            public_key: connection_settings.certificate.clone(),
            domain_name: connection_settings.domain_name.clone(),
            nonce: Self::create_nonce(),
            end_route,
        };

        self.repository.set_route(conversation_id, &route).await
    }

    pub async fn redeem_route(&self, conversation_id: &str, nonce: &str) -> Option<Route> {
        if let Some(route) = self.repository.get_route(conversation_id, nonce).await {
            self.repository.remove_route(nonce).await;
            return Some(route);
        }

        None
    }

    pub async fn get_next_route(
        &self,
        conversation: &Conversation,
        proxies: &[SessionInfo],
    ) -> Option<SessionInfo> {
        let strategy = self.route_strategy_factory.get_strategy(conversation);
        strategy.get_next_route(conversation, proxies).await
    }

    pub fn check_for_final_route(&self, conversation: &Conversation) -> bool {
        let strategy = self.route_strategy_factory.get_strategy(conversation);
        strategy.has_reached_final_route(conversation)
    }

    fn create_conversation_id() -> String {
        Uuid::new_v4().to_string().replace('-', "")
    }

    fn create_nonce() -> String {
        Uuid::new_v4().to_string()
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::storage::MockRouteRepository;
    use crosscutting::networking::to_socket_address;

    const EXPECTED_CONVERSATION_ID: &str = "test_conversation_id";
    const EXPECTED_NONCE: &str = "test_nonce";
    const EXPECTED_IP: &str = "127.0.0.1";
    const EXPECTED_PORT: u16 = 8080;
    const EXPECTED_FROM: &str = "from";
    const EXPECTED_TO: &str = "to";
    const EXPECTED_SESSION_ID: &str = "test_session_id";
    const EXPECTED_STRATEGY_ID: u8 = 1;
    const EXPECTED_PUBLIC_KEY: &[u8] = b"test_public_key";
    const EXPECTED_DOMAIN_NAME: &str = "test_domain_name";

    impl RouteManager {
        fn with_repository(repository: Box<dyn RouteRepository>) -> Self {
            Self {
                route_strategy_factory: RouteStrategyFactory::new(),
                repository,
            }
        }

        fn with_strategy_factory(strategy_factory: RouteStrategyFactory) -> Self {
            Self {
                route_strategy_factory: strategy_factory,
                repository: Box::new(MockRouteRepository::new()),
            }
        }
    }

    fn get_connection_settings() -> ConnectionSettings {
        ConnectionSettings {
            ip: EXPECTED_IP.to_string(),
            port: EXPECTED_PORT,
            domain_name: EXPECTED_DOMAIN_NAME.to_string(),
            certificate: EXPECTED_PUBLIC_KEY.to_vec(),
        }
    }

    #[tokio::test]
    async fn new_creates_route_manager_with_routing_strategies() {
        let cancellation_token = CancellationToken::new();
        let manager = RouteManager::new(RepositoryType::InMemory, cancellation_token);

        assert!(!manager.route_strategy_factory.strategies.is_empty());
    }

    #[tokio::test]
    async fn initialize_calls_set_conversation() {
        let mut mock_repo = MockRouteRepository::new();
        mock_repo
            .expect_set_conversation()
            .returning(|_| Some(EXPECTED_CONVERSATION_ID.to_string()));

        let manager = RouteManager::with_repository(Box::new(mock_repo));
        let result = manager.initialize(EXPECTED_FROM, EXPECTED_TO).await;

        assert_eq!(result, Some(EXPECTED_CONVERSATION_ID.to_string()));
    }

    #[tokio::test]
    async fn store_route_calls_set_route() {
        let mut mock_repo = MockRouteRepository::new();
        mock_repo
            .expect_set_route()
            .withf(|conversation_id, route| {
                conversation_id == EXPECTED_CONVERSATION_ID
                    && route.on_ip_address == EXPECTED_IP
                    && route.on_port_number == EXPECTED_PORT
                    && route.end_route == false
            })
            .returning(|_, _| Some(EXPECTED_NONCE.to_string()));

        let manager = RouteManager::with_repository(Box::new(mock_repo));
        let result = manager
            .store_route(EXPECTED_CONVERSATION_ID, &get_connection_settings(), false)
            .await;

        assert_eq!(result, Some(EXPECTED_NONCE.to_string()));
    }

    #[tokio::test]
    async fn redeem_route_calls_get_route_and_remove_route() {
        let mut mock_repo = MockRouteRepository::new();
        mock_repo
            .expect_get_route()
            .withf(|conversation_id, nonce| {
                conversation_id == EXPECTED_CONVERSATION_ID && nonce == EXPECTED_NONCE
            })
            .returning(|_, _| {
                Some(Route {
                    on_ip_address: EXPECTED_IP.to_string(),
                    on_port_number: EXPECTED_PORT,
                    public_key: EXPECTED_PUBLIC_KEY.to_vec(),
                    domain_name: EXPECTED_DOMAIN_NAME.to_string(),
                    nonce: EXPECTED_NONCE.to_string(),
                    end_route: false,
                })
            });

        mock_repo
            .expect_remove_route()
            .withf(|nonce| nonce == EXPECTED_NONCE)
            .returning(|_| ());

        let manager = RouteManager::with_repository(Box::new(mock_repo));
        let result = manager
            .redeem_route(EXPECTED_CONVERSATION_ID, EXPECTED_NONCE)
            .await;

        assert!(result.is_some());
        assert_eq!(result.unwrap().on_ip_address, EXPECTED_IP);
    }

    #[tokio::test]
    async fn redeem_route_avoids_removing_route_when_get_route_returns_none() {
        let mut mock_repo = MockRouteRepository::new();
        mock_repo
            .expect_get_route()
            .withf(|conversation_id, nonce| {
                conversation_id == EXPECTED_CONVERSATION_ID && nonce == EXPECTED_NONCE
            })
            .returning(|_, _| None);

        mock_repo.expect_remove_route().times(0);

        let manager = RouteManager::with_repository(Box::new(mock_repo));
        let result = manager
            .redeem_route(EXPECTED_CONVERSATION_ID, EXPECTED_NONCE)
            .await;

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn get_next_route_calls_get_next_route_on_strategy() {
        let mut mock_strategy = MockRouteStrategy::new();
        mock_strategy
            .expect_get_id()
            .returning(|| EXPECTED_STRATEGY_ID);

        mock_strategy
            .expect_get_next_route()
            .returning(|_, proxies| proxies.first().cloned());

        let factory = RouteStrategyFactory {
            strategies: vec![Box::new(mock_strategy) as Box<dyn RouteStrategy>],
        };

        let conversation = Conversation::new(
            EXPECTED_CONVERSATION_ID.to_string(),
            EXPECTED_FROM.to_string(),
            EXPECTED_TO.to_string(),
            EXPECTED_STRATEGY_ID,
        );

        let session_info = SessionInfo {
            access_key: EXPECTED_SESSION_ID.to_string(),
            uid: EXPECTED_FROM.to_string(),
            on_ip_address: EXPECTED_IP.to_string(),
            on_port_number: EXPECTED_PORT,
            client_ip: to_socket_address(EXPECTED_IP, EXPECTED_PORT).unwrap(),
            component_type: 2,
            public_key: EXPECTED_PUBLIC_KEY.to_vec(),
            domain_name: EXPECTED_DOMAIN_NAME.to_string(),
        };

        let available_proxies = vec![session_info.clone()];
        let manager = RouteManager::with_strategy_factory(factory);
        let result = manager
            .get_next_route(&conversation, &available_proxies)
            .await;

        assert!(result.is_some());
    }

    #[tokio::test]
    async fn check_for_final_route_calls_has_reached_final_route_on_strategy() {
        let mut mock_strategy = MockRouteStrategy::new();
        mock_strategy
            .expect_get_id()
            .returning(|| EXPECTED_STRATEGY_ID);

        mock_strategy
            .expect_has_reached_final_route()
            .returning(|_| true);

        let factory = RouteStrategyFactory {
            strategies: vec![Box::new(mock_strategy) as Box<dyn RouteStrategy>],
        };

        let conversation = Conversation::new(
            EXPECTED_CONVERSATION_ID.to_string(),
            EXPECTED_FROM.to_string(),
            EXPECTED_TO.to_string(),
            EXPECTED_STRATEGY_ID,
        );

        let manager = RouteManager::with_strategy_factory(factory);
        let result = manager.check_for_final_route(&conversation);

        assert!(result);
    }
}
