use super::RedisRepository;
use crate::models::SessionInfo;
use crate::storage;
use crate::storage::SessionRepository;
use crosscutting::Component;
use redis::{Commands, FromRedisValue, ToRedisArgs, Value, from_redis_value};
use tonic::async_trait;

const CONTROLLER_SESSION_KEY: &str = "ctrl_ss";
const CLIENT_SESSION_KEY: &str = "c_ss";
const PROXY_SESSION_KEY: &str = "p_ss";
const SESSIONS_KEY: &str = "ss";
const EXPIRY_TIME: redis::Expiry = redis::Expiry::EX(storage::SESSIONS_EXPIRATION_TIME.as_secs());

fn get_session_key(key: &str) -> String {
    format!("{}:{}", SESSIONS_KEY, key)
}

fn get_member_session_key(component_type: &Component, uid: &str) -> String {
    let key = match component_type {
        Component::Controller => CONTROLLER_SESSION_KEY,
        Component::Client => CLIENT_SESSION_KEY,
        Component::Proxy => PROXY_SESSION_KEY,
    };

    format!("{}:{}", key, uid)
}

#[async_trait]
impl SessionRepository for RedisRepository {
    async fn set_session(&self, session_info: &SessionInfo) {
        let mut connection = self.connection.write().await;
        let component_type = Component::from(session_info.component_type);

        redis::pipe()
            .atomic()
            .set_ex(
                get_member_session_key(&component_type, &session_info.uid),
                session_info.access_key.clone(),
                storage::SESSIONS_EXPIRATION_TIME.as_secs(),
            )
            .set_ex(
                get_session_key(&session_info.access_key),
                session_info.to_owned(),
                storage::SESSIONS_EXPIRATION_TIME.as_secs(),
            )
            .exec(&mut connection)
            .unwrap();
    }

    async fn get_session(&self, access_key: &str) -> Option<SessionInfo> {
        let key = get_session_key(access_key);
        let mut connection = self.connection.write().await;

        match connection.get_ex::<String, SessionInfo>(key, EXPIRY_TIME) {
            Ok(session_info) => {
                let component_type = Component::from(session_info.component_type);
                let c_key = get_member_session_key(&component_type, &session_info.uid);
                () = connection
                    .expire(c_key, storage::SESSIONS_EXPIRATION_TIME.as_secs() as i64)
                    .unwrap();
                Some(session_info)
            }
            _ => None,
        }
    }

    async fn remove_session(&self, access_key: &str) {
        let key = get_session_key(access_key);
        let mut connection = self.connection.write().await;
        () = connection.del(key).unwrap();
    }

    async fn get_proxies(&self, access_key: &str) -> Option<Vec<SessionInfo>> {
        let mut connection = self.connection.write().await;
        let key = get_member_session_key(&Component::Proxy, "*");
        let all: Vec<String> = connection.scan_match(&key).unwrap().collect();

        if all.is_empty() {
            return None;
        }

        let mut proxies: Vec<SessionInfo> = Vec::new();
        all.iter().for_each(|key| {
            let key: String = connection.get(key).unwrap();
            let key = get_session_key(&key);
            let session: SessionInfo = connection.get(key).unwrap();
            if session.access_key != access_key {
                proxies.push(session);
            }
        });

        if proxies.is_empty() {
            return None;
        }

        Some(proxies)
    }

    async fn get_client(&self, uid: &str) -> Option<SessionInfo> {
        let key = get_member_session_key(&Component::Client, uid);
        let mut connection = self.connection.write().await;
        let access_key: String = connection.get(key).unwrap();
        let key = get_session_key(&access_key);
        connection.get(key).ok()
    }

    async fn count_proxies(&self) -> usize {
        let mut connection = self.connection.write().await;
        let key = get_member_session_key(&Component::Proxy, "*");
        let keys: Vec<String> = connection.scan_match(&key).unwrap().collect();
        keys.len()
    }

    async fn count_clients(&self) -> usize {
        let mut connection = self.connection.write().await;
        let key = get_member_session_key(&Component::Client, "*");
        let keys: Vec<String> = connection.scan_match(&key).unwrap().collect();
        keys.len()
    }

    async fn count_controllers(&self) -> usize {
        let mut connection = self.connection.write().await;
        let key = get_member_session_key(&Component::Controller, "*");
        let keys: Vec<String> = connection.scan_match(&key).unwrap().collect();
        keys.len()
    }
}

impl ToRedisArgs for SessionInfo {
    fn write_redis_args<W>(&self, out: &mut W)
    where
        W: ?Sized + redis::RedisWrite,
    {
        let json = serde_json::to_string(self).unwrap();
        out.write_arg(&json.into_bytes());
    }
}

impl FromRedisValue for SessionInfo {
    fn from_redis_value(v: &Value) -> redis::RedisResult<Self> {
        let value: String = from_redis_value(v)?;
        let session: SessionInfo = serde_json::from_str(&value).unwrap();
        Ok(session)
    }
}
