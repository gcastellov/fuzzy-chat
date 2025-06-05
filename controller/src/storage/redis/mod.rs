pub mod member_repository;
pub mod route_repository;
pub mod session_repository;

use tokio::sync::RwLock;

pub struct RedisRepository {
    connection: RwLock<redis::Connection>,
}

impl RedisRepository {
    pub fn new(uri: &str) -> Self {
        let connection = RedisRepository::get_connection(uri);
        Self {
            connection: RwLock::new(connection),
        }
    }

    fn get_connection(uri: &str) -> redis::Connection {
        let client = redis::Client::open(uri).unwrap();
        client.get_connection().unwrap()
    }
}
