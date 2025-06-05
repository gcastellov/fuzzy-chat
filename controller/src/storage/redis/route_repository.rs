use crate::models::{Conversation, Route};
use crate::storage::{self, RouteRepository};
use redis::{Commands, FromRedisValue, ToRedisArgs, Value, from_redis_value};
use tonic::async_trait;

use super::RedisRepository;

const CONVERSATIONS_KEY: &str = "cs";
const ROUTES_KEY: &str = "rs";

fn get_conversation_key(conversation_id: &str) -> String {
    format!("{}:{}", CONVERSATIONS_KEY, conversation_id)
}

fn get_routes_key(conversation_id: &str) -> String {
    format!("{}:{}", ROUTES_KEY, conversation_id)
}

#[async_trait]
impl RouteRepository for RedisRepository {
    async fn set_conversation(&self, conversation: &Conversation) -> Option<String> {
        let mut connection = self.connection.write().await;
        let key = get_conversation_key(&conversation.id);
        () = connection
            .set_ex(
                key,
                conversation.to_owned(),
                storage::CONVERSATIONS_EXPIRATION_TIME.as_secs(),
            )
            .unwrap();
        Some(conversation.id.clone())
    }

    async fn remove_conversation(&self, conversation_id: &str) {
        let mut connection = self.connection.write().await;
        let key = get_conversation_key(conversation_id);
        () = connection.del(key).unwrap();
    }

    async fn get_conversation(&self, conversation_id: &str) -> Option<Conversation> {
        let key = get_conversation_key(conversation_id);
        let mut connection = self.connection.write().await;
        connection.get(key).ok()
    }

    async fn set_route(&self, conversation_id: &str, route: &Route) -> Option<String> {
        let mut conversation = self.get_conversation(conversation_id).await.unwrap();
        conversation.routes.push(route.clone());

        let key = get_routes_key(conversation_id);
        let mut connection = self.connection.write().await;
        redis::pipe()
            .atomic()
            .hset(key.to_owned(), route.nonce.to_owned(), route)
            .expire(key, storage::ROUTES_EXPIRATION_TIME.as_secs() as i64)
            .set_ex(
                get_conversation_key(conversation_id),
                conversation.to_owned(),
                storage::CONVERSATIONS_EXPIRATION_TIME.as_secs(),
            )
            .exec(&mut connection)
            .unwrap();

        Some(route.nonce.to_owned())
    }

    async fn get_route(&self, conversation_id: &str, nonce: &str) -> Option<Route> {
        let mut connection = self.connection.write().await;
        let key = get_routes_key(conversation_id);
        connection.hget(key, nonce).ok()
    }

    async fn remove_route(&self, nonce: &str) {
        let mut connection = self.connection.write().await;
        let key = get_routes_key(nonce);
        () = connection.hdel(key, nonce).unwrap();
    }
}

impl ToRedisArgs for Conversation {
    fn write_redis_args<W>(&self, out: &mut W)
    where
        W: ?Sized + redis::RedisWrite,
    {
        let json = serde_json::to_string(self).unwrap();
        out.write_arg(&json.into_bytes());
    }
}

impl ToRedisArgs for Route {
    fn write_redis_args<W>(&self, out: &mut W)
    where
        W: ?Sized + redis::RedisWrite,
    {
        let json = serde_json::to_string(self).unwrap();
        out.write_arg(&json.into_bytes());
    }
}

impl FromRedisValue for Conversation {
    fn from_redis_value(v: &Value) -> redis::RedisResult<Self> {
        let value: String = from_redis_value(v)?;
        let conversation: Conversation = serde_json::from_str(&value).unwrap();
        Ok(conversation)
    }
}

impl FromRedisValue for Route {
    fn from_redis_value(v: &Value) -> redis::RedisResult<Self> {
        let value: String = from_redis_value(v)?;
        let route: Route = serde_json::from_str(&value).unwrap();
        Ok(route)
    }
}
