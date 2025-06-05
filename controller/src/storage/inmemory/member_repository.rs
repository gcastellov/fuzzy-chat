use crate::models::Member;
use crate::storage::MemberRepository;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;
use tonic::async_trait;

type MembersCollection = HashMap<String, Member>;

pub struct InMemorMemberRepository {
    members: Arc<RwLock<MembersCollection>>,
}

impl InMemorMemberRepository {
    pub fn new() -> Self {
        Self {
            members: Arc::new(RwLock::new(MembersCollection::new())),
        }
    }
}

#[async_trait]
impl MemberRepository for InMemorMemberRepository {
    async fn set_member(&self, member: &Member) {
        let mut members = self.members.write().await;
        members.insert(member.uid.to_owned(), member.to_owned());
    }

    async fn get_member(&self, uid: &str) -> Option<Member> {
        let members = self.members.read().await;
        members.get(uid).cloned()
    }
}
