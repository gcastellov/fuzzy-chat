use super::RedisRepository;
use crate::models::Member;
use crate::storage::MemberRepository;
use redis::{Commands, FromRedisValue, ToRedisArgs, Value, from_redis_value};
use tonic::async_trait;

const MEMBER_KEY: &str = "mb";

#[async_trait]
impl MemberRepository for RedisRepository {
    async fn set_member(&self, member: &Member) {
        let mut connection = self.connection.write().await;
        () = connection
            .hset(MEMBER_KEY, member.uid.to_owned(), member)
            .unwrap();
    }

    async fn get_member(&self, uid: &str) -> Option<Member> {
        let mut connection = self.connection.write().await;
        connection.hget(MEMBER_KEY, uid).ok()
    }
}

impl ToRedisArgs for Member {
    fn write_redis_args<W>(&self, out: &mut W)
    where
        W: ?Sized + redis::RedisWrite,
    {
        let json = serde_json::to_string(self).unwrap();
        out.write_arg(&json.into_bytes());
    }
}

impl FromRedisValue for Member {
    fn from_redis_value(v: &Value) -> redis::RedisResult<Self> {
        let value: String = from_redis_value(v)?;
        let member: Member = serde_json::from_str(&value).unwrap();
        Ok(member)
    }
}
