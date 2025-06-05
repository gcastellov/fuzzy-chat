use crate::models::Member;
use crate::storage::{self, MemberRepository, RepositoryType};
use csv::ReaderBuilder;
use std::fs::File;

pub struct MemberManager {
    repository: Box<dyn MemberRepository>,
}

impl MemberManager {
    pub fn new(repository_type: RepositoryType) -> Self {
        Self {
            repository: storage::create_member_repository(repository_type).unwrap(),
        }
    }

    pub async fn get_member(&self, uid: &str) -> Option<Member> {
        self.repository.get_member(uid).await
    }

    pub async fn seed_members_from_csv(&self, file_path: &str) -> Result<(), String> {
        let members: Vec<Member> = MemberManager::read_members_from_csv(file_path);
        self.set_members(&members).await;
        Ok(())
    }

    pub async fn set_members(&self, members: &[Member]) {
        for member in members {
            self.repository.set_member(member).await;
        }
    }

    fn read_members_from_csv(path: &str) -> Vec<Member> {
        let file = File::open(path).unwrap();
        let mut rdr = ReaderBuilder::new()
            .has_headers(false)
            .delimiter(b';')
            .from_reader(file);

        rdr.records()
            .filter_map(|record| {
                if let Ok(item) = record {
                    let uid = item.get(0).unwrap().to_string();
                    let pwd = item.get(1).unwrap().to_string();
                    Some(Member::new(uid, pwd))
                } else {
                    None
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{MockMemberRepository, RepositoryType};

    const EXPECTED_UID: &str = "1234567890";
    const EXPECTED_PWD: &str = "password";

    impl MemberManager {
        fn with_repository(repository: Box<dyn MemberRepository>) -> Self {
            Self { repository }
        }
    }

    impl PartialEq for Member {
        fn eq(&self, other: &Self) -> bool {
            self.uid == other.uid && self.pwd == other.pwd
        }
    }

    #[tokio::test]
    async fn new_creates_member_manager() {
        _ = MemberManager::new(RepositoryType::InMemory);
    }

    #[tokio::test]
    async fn get_member_returns_none_if_no_member_with_uid() {
        let mut mock_repo = MockMemberRepository::new();
        mock_repo
            .expect_get_member()
            .with(mockall::predicate::eq(EXPECTED_UID))
            .returning(|_| None);

        let member_manager = MemberManager::with_repository(Box::new(mock_repo));
        let member = member_manager.get_member(EXPECTED_UID).await;
        assert!(member.is_none());
    }

    #[tokio::test]
    async fn get_member_returns_member_if_exists() {
        let mut mock_repo = MockMemberRepository::new();
        let expected_member = Member::new(EXPECTED_UID.to_string(), EXPECTED_PWD.to_string());
        let ref_expected_member = expected_member.clone();

        mock_repo
            .expect_get_member()
            .with(mockall::predicate::eq(EXPECTED_UID))
            .returning(move |_| Some(ref_expected_member.clone()));

        let member_manager = MemberManager::with_repository(Box::new(mock_repo));
        let member = member_manager.get_member(EXPECTED_UID).await.unwrap();

        assert_eq!(member, expected_member);
    }
}
