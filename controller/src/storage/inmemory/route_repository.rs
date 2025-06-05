use super::ExpirationWrapper;
use crate::models::{Conversation, Route};
use crate::storage::{self, RouteRepository};
use log::debug;
use tonic::async_trait;

use std::time::Duration;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

const EXPIRATION_TIME_CHECK: Duration = Duration::from_millis(30000);

type RoutesCollection = HashMap<String, ExpirationWrapper<Route>>;
type ConversationsCollection = HashMap<String, ExpirationWrapper<Conversation>>;

pub struct InMemoryRepository {
    routes: Arc<RwLock<RoutesCollection>>,
    conversations: Arc<RwLock<ConversationsCollection>>,
    _handle: tokio::task::JoinHandle<()>,
}

impl InMemoryRepository {
    pub fn new(cancellation_token: CancellationToken) -> Self {
        let routes = Arc::new(RwLock::new(HashMap::new()));
        let conversations = Arc::new(RwLock::new(HashMap::new()));

        Self {
            routes: Arc::clone(&routes),
            conversations: Arc::clone(&conversations),
            _handle: Self::kill_expired_sessions(routes, conversations, cancellation_token),
        }
    }

    fn kill_expired_sessions(
        routes: Arc<RwLock<RoutesCollection>>,
        conversations: Arc<RwLock<ConversationsCollection>>,
        cancellation_token: CancellationToken,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            while !cancellation_token.is_cancelled() {
                tokio::time::sleep(EXPIRATION_TIME_CHECK).await;
                debug!("Checking for expired routes and conversations...");

                let mut routes = routes.write().await;
                let mut conversations = conversations.write().await;
                routes.retain(|_, wrapper| !wrapper.is_expired());
                conversations.retain(|_, wrapper| !wrapper.is_expired());
            }

            debug!("Expired routes and conversations check terminated.");
        })
    }
}

#[async_trait]
impl RouteRepository for InMemoryRepository {
    async fn set_conversation(&self, conversation: &Conversation) -> Option<String> {
        let mut paths = self.conversations.write().await;
        let conversation_id = conversation.id.to_owned();
        let found = paths
            .insert(
                conversation_id.to_owned(),
                ExpirationWrapper::new(
                    conversation.to_owned(),
                    storage::CONVERSATIONS_EXPIRATION_TIME,
                ),
            )
            .is_some();

        if found { None } else { Some(conversation_id) }
    }

    async fn remove_conversation(&self, conversation_id: &str) {
        let mut paths = self.conversations.write().await;
        paths.remove(conversation_id);
    }

    async fn get_conversation(&self, conversation_id: &str) -> Option<Conversation> {
        let paths = self.conversations.read().await;
        paths
            .get(conversation_id)
            .map(|wrapper| wrapper.value.to_owned())
    }

    async fn set_route(&self, conversation_id: &str, route: &Route) -> Option<String> {
        let mut routes = self.routes.write().await;
        let mut paths = self.conversations.write().await;

        if routes
            .insert(
                route.nonce.clone(),
                ExpirationWrapper::new(route.clone(), storage::ROUTES_EXPIRATION_TIME),
            )
            .is_some()
        {
            return None;
        }

        Some(route.to_owned())
            .and(
                paths
                    .get_mut(conversation_id)
                    .map(|wrapper| wrapper.value.routes.push(route.clone())),
            )
            .and(Some(route.nonce.to_owned()))
    }

    async fn get_route(&self, _: &str, nonce: &str) -> Option<Route> {
        let routes = self.routes.read().await;
        routes.get(nonce).map(|wrapper| wrapper.value.to_owned())
    }

    async fn remove_route(&self, nonce: &str) {
        let mut routes = self.routes.write().await;
        routes.remove(nonce);
    }
}
