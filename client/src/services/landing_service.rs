use super::*;
use crate::models::{
    TextMessage,
    client_proto::{TextRequest, TextResponse, landing_service_server::LandingService},
};

use routing::route_client::{RouteClientFactory, RouterFactory};

pub struct LandingServiceImpl {
    tx: Sender<TextMessage>,
    router_factory: Box<dyn RouterFactory>,
}

impl LandingServiceImpl {
    pub fn new(tx: Sender<TextMessage>) -> Self {
        Self {
            tx,
            router_factory: Box::new(RouteClientFactory),
        }
    }
}

#[tonic::async_trait]
impl LandingService for LandingServiceImpl {
    async fn receive(
        &self,
        request: Request<TextRequest>,
    ) -> Result<Response<TextResponse>, Status> {
        let text_request = request.into_inner();
        let mut router = self.router_factory.get_router();
        router
            .initialize()
            .await
            .map_err(|_| Status::internal("Failed to initialize the router"))?;

        let redeem_response = router
            .redeem(
                text_request.conversation_id.clone(),
                text_request.access_key.clone(),
                text_request.nonce.clone(),
            )
            .await
            .map_err(|_| Status::internal("Failed to redeem"))?;

        info!("A new message has been received");
        let source = redeem_response.source_info.unwrap();
        let message = TextMessage::new(source.from.clone(), "myself".into(), &text_request.content);

        _ = self.tx.send(message).await;
        Ok(Response::new(TextResponse {}))
    }
}

#[cfg(test)]
mod tests {
    
    use super::*;
    use std::io::ErrorKind;
    use protoc_rust::Error;
    use routing::route_client::{route::{RedeemResponse, SourceInfo}, MockRouter, MockRouterFactory};

    const EXPECTED_ACCESS_KEY: &str = "test_access_key";
    const EXPECTED_NONCE: &str = "test_nonce";
    const EXPECTED_CONVERSATION_ID: &str = "test_conversation";
    const EXPECTED_SENDER_UID: &str = "sender_uid";
    const EXPECTED_MESSAGE: &str = "test_message";

    #[tokio::test]
    async fn given_redeem_fails_when_receiving_message_then_returns_internal_error() {
        let (tx, _rx) = tokio::sync::mpsc::channel(100);

        let mut router_factory = MockRouterFactory::new();
        router_factory
            .expect_get_router()
            .returning(||{
                let mut mock_router = MockRouter::new();
                mock_router.expect_redeem().returning(|_, _, _| {
                    Box::pin(async {
                        Err(Box::<dyn std::error::Error>::from(Error::new(
                            ErrorKind::Other,
                            "Something went wrong",
                        )))
                    })
                });

                Box::new(mock_router)
            });

        let service = LandingServiceImpl {
            tx,
            router_factory: Box::new(router_factory),
        };

        let request = Request::new(TextRequest {
            conversation_id: EXPECTED_CONVERSATION_ID.into(),
            access_key: EXPECTED_ACCESS_KEY.into(),
            nonce: EXPECTED_NONCE.into(),
            content: EXPECTED_MESSAGE.into(),
        });

        let response = service.receive(request).await;
        assert!(response.is_err());

        let error = response.unwrap_err();
        assert_eq!(error.code(), tonic::Code::Internal);
        assert_eq!(error.message(), "Failed to redeem");
    }

    #[tokio::test]
    async fn given_redeem_succeeds_when_receiving_message_then_channels_message() {
        let (tx, mut rx) = tokio::sync::mpsc::channel(100);

        let mut router_factory = MockRouterFactory::new();
        router_factory
            .expect_get_router()
            .returning(||{
                let mut mock_router = MockRouter::new();
                mock_router.expect_redeem().returning(|_, _, _| {
                    Box::pin(async {
                        Ok(RedeemResponse {
                            source_info: Some(SourceInfo {
                                from: EXPECTED_SENDER_UID.into(),
                            })
                        })
                    })
                });

                Box::new(mock_router)
            });

        let service = LandingServiceImpl {
            tx,
            router_factory: Box::new(router_factory),
        };

        let request = Request::new(TextRequest {
            conversation_id: EXPECTED_CONVERSATION_ID.into(),
            access_key: EXPECTED_ACCESS_KEY.into(),
            nonce: EXPECTED_NONCE.into(),
            content: EXPECTED_MESSAGE.into(),
        });

        let response = service.receive(request).await;
        assert!(response.is_ok());

        rx.recv().await.map(|message| {
            assert_eq!(message.from, EXPECTED_SENDER_UID);
            assert_eq!(message.to, "myself");
            assert_eq!(message.content, EXPECTED_MESSAGE);
        }).expect("Failed to receive message from channel");
    }
}
