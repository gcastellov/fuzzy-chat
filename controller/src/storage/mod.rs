mod inmemory;
mod redis;

use crate::models::{Conversation, Member, Route, SessionInfo};
use crosscutting::settings;
use inmemory::route_repository as route_in_memory_repository;
use inmemory::session_repository as session_in_memory_repository;
use mockall::automock;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use tonic::async_trait;

const REPOSITORY_TYPE_KEY: &str = "REPOSITORY";
const REDIS_SESSION_REPO_ERROR: &str = "Failed to create the session's Redis repository";
const REDIS_ROUTE_REPO_ERROR: &str = "Failed to create the route's Redis repository";
const REDIS_MEMBER_REPO_ERROR: &str = "Failed to create the member's Redis repository";
const REDIS_URL_KEY: &str = "REDIS_URL";

const ROUTES_EXPIRATION_TIME: Duration = Duration::from_millis(60000);
const CONVERSATIONS_EXPIRATION_TIME: Duration = Duration::from_millis(60000);
const SESSIONS_EXPIRATION_TIME: Duration = Duration::from_millis(10000);

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum RepositoryType {
    #[default]
    InMemory,
    Redis,
}

impl From<u8> for RepositoryType {
    fn from(value: u8) -> Self {
        match value {
            1 => RepositoryType::Redis,
            _ => RepositoryType::default(),
        }
    }
}

impl RepositoryType {
    pub fn get_from_env() -> RepositoryType {
        settings::environment::get_env_variable(REPOSITORY_TYPE_KEY)
            .unwrap_or_default()
            .parse::<u8>()
            .map(RepositoryType::from)
            .unwrap_or_default()
    }
}

#[automock]
#[async_trait]
pub trait SessionRepository: Sync + Send {
    async fn set_session(&self, session_info: &SessionInfo);
    async fn get_session(&self, access_key: &str) -> Option<SessionInfo>;
    async fn remove_session(&self, access_key: &str);
    async fn get_proxies(&self, access_key: &str) -> Option<Vec<SessionInfo>>;
    async fn get_client(&self, uid: &str) -> Option<SessionInfo>;
    async fn count_proxies(&self) -> usize;
    async fn count_clients(&self) -> usize;
    async fn count_controllers(&self) -> usize;
}

#[automock]
#[async_trait]
pub trait RouteRepository: Send + Sync {
    async fn set_conversation(&self, conversation: &Conversation) -> Option<String>;
    async fn remove_conversation(&self, conversation_id: &str);
    async fn get_conversation(&self, conversation_id: &str) -> Option<Conversation>;
    async fn set_route(&self, conversation_id: &str, route: &Route) -> Option<String>;
    async fn get_route(&self, conversation_id: &str, nonce: &str) -> Option<Route>;
    async fn remove_route(&self, nonce: &str);
}

#[automock]
#[async_trait]
pub trait MemberRepository: Send + Sync {
    async fn set_member(&self, member: &Member);
    async fn get_member(&self, uid: &str) -> Option<Member>;
}

pub fn create_session_repository(
    repo_type: RepositoryType,
    cancellation_token: CancellationToken,
) -> Result<Box<dyn SessionRepository>, String> {
    match repo_type {
        RepositoryType::InMemory => Ok(Box::new(
            session_in_memory_repository::InMemoryRepository::new(cancellation_token),
        )),
        RepositoryType::Redis => {
            let redis_url =
                settings::environment::get_env_variable(REDIS_URL_KEY).unwrap_or_default();
            let result = std::panic::catch_unwind(|| redis::RedisRepository::new(&redis_url));
            match result {
                Ok(redis_repo) => Ok(Box::new(redis_repo)),
                Err(_) => Err(String::from(REDIS_SESSION_REPO_ERROR)),
            }
        }
    }
}

pub fn create_route_repository(
    repo_type: RepositoryType,
    cancellation_token: CancellationToken,
) -> Result<Box<dyn RouteRepository>, String> {
    match repo_type {
        RepositoryType::InMemory => Ok(Box::new(
            route_in_memory_repository::InMemoryRepository::new(cancellation_token),
        )),
        RepositoryType::Redis => {
            let redis_url =
                settings::environment::get_env_variable(REDIS_URL_KEY).unwrap_or_default();
            let result = std::panic::catch_unwind(|| redis::RedisRepository::new(&redis_url));
            match result {
                Ok(redis_repo) => Ok(Box::new(redis_repo)),
                Err(_) => Err(String::from(REDIS_ROUTE_REPO_ERROR)),
            }
        }
    }
}

pub fn create_member_repository(
    repo_type: RepositoryType,
) -> Result<Box<dyn MemberRepository>, String> {
    match repo_type {
        RepositoryType::InMemory => Ok(Box::new(
            inmemory::member_repository::InMemorMemberRepository::new(),
        )),
        RepositoryType::Redis => {
            let redis_url =
                settings::environment::get_env_variable(REDIS_URL_KEY).unwrap_or_default();
            let result = std::panic::catch_unwind(|| redis::RedisRepository::new(&redis_url));
            match result {
                Ok(redis_repo) => Ok(Box::new(redis_repo)),
                Err(_) => Err(String::from(REDIS_MEMBER_REPO_ERROR)),
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use tokio_util::sync::CancellationToken;

    use super::*;

    #[test]
    fn get_from_no_env_returns_default_repository_type() {
        let repo_type = RepositoryType::get_from_env();
        assert_eq!(repo_type, RepositoryType::default());
    }

    #[test]
    fn default_repository_type_returns_inmemory() {
        let default_repo = RepositoryType::default();
        assert_eq!(default_repo, RepositoryType::InMemory);
    }

    #[test]
    fn from_u8_to_redis_repository_type() {
        let repo_type = RepositoryType::from(1);
        assert_eq!(repo_type, RepositoryType::Redis);
    }

    #[test]
    fn from_u8_to_inmemory_repository_type() {
        let repo_type = RepositoryType::from(0);
        assert_eq!(repo_type, RepositoryType::InMemory);
    }

    #[tokio::test]
    async fn create_in_memory_session_repository() {
        let cancellation_token = CancellationToken::new();
        let in_memory_repo =
            create_session_repository(RepositoryType::InMemory, cancellation_token);
        assert!(in_memory_repo.is_ok());
    }

    #[tokio::test]
    async fn create_redis_session_repository() {
        let cancellation_token = CancellationToken::new();
        let redis_repo = create_session_repository(RepositoryType::Redis, cancellation_token);
        assert!(redis_repo.is_err());
    }

    #[tokio::test]
    async fn create_in_memory_route_repository() {
        let cancellation_token = CancellationToken::new();
        let in_memory_repo = create_route_repository(RepositoryType::InMemory, cancellation_token);
        assert!(in_memory_repo.is_ok());
    }

    #[tokio::test]
    async fn create_redis_route_repository() {
        let cancellation_token = CancellationToken::new();
        let redis_repo = create_route_repository(RepositoryType::Redis, cancellation_token);
        assert!(redis_repo.is_err());
    }

    #[tokio::test]
    async fn create_in_memory_member_repository() {
        let in_memory_repo = create_member_repository(RepositoryType::InMemory);
        assert!(in_memory_repo.is_ok());
    }

    #[tokio::test]
    async fn create_redis_member_repository() {
        let redis_repo = create_member_repository(RepositoryType::Redis);
        assert!(redis_repo.is_err());
    }
}
