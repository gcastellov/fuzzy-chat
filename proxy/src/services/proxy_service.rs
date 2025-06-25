use super::*;
use crate::models::proxy_proto::{
    CommandRequest, CommandResponse, proxy_service_server::ProxyService,
};
use crate::{
    clients::{
        info_client::{InfoClientFactory, InformerFactory},
        landing_client::{LanderFactory, LandingClientFactory},
    },
    models::info_proto::StatusResponse,
};
use authorization::auth_client::Authenticator;
use crosscutting::ConnectionSettings;
use routing::proxy_client::{ProxyClientFactory, ProxyFactory, proxy::CommandType};
use routing::route_client::{RouteClientFactory, RouterFactory};

pub struct ProxyServiceImpl {
    authenticator: Arc<RwLock<Box<dyn Authenticator>>>,
    router_factory: Box<dyn RouterFactory>,
    informer_factory: Box<dyn InformerFactory>,
    lander_factory: Box<dyn LanderFactory>,
    proxy_factory: Box<dyn ProxyFactory>,
}

#[tonic::async_trait]
impl ProxyService for ProxyServiceImpl {
    async fn execute_command(
        &self,
        request: Request<CommandRequest>,
    ) -> Result<Response<CommandResponse>, Status> {
        let req = request.into_inner();
        let access_key = self.check_authentication().await?;
        let content = req.content.unwrap_or_default();
        let nonce = req.nonce;
        let conversation_id = req.conversation_id;

        self.redeem(conversation_id.clone(), access_key.clone(), nonce)
            .await?;

        let result: Option<String> = match CommandType::try_from(req.command) {
            Ok(CommandType::Status) => {
                let status = self.status(access_key).await?;
                Some(format!("Status: {:?}", status))
            }
            Ok(CommandType::Send) => {
                self.send(conversation_id, access_key, &content).await?;
                Some("Message sent".into())
            }
            _ => None,
        };

        let response = CommandResponse { result };
        Ok(Response::new(response))
    }
}

impl ProxyServiceImpl {
    pub fn new(authenticator: Arc<RwLock<Box<dyn Authenticator>>>) -> Self {
        ProxyServiceImpl {
            authenticator,
            router_factory: Box::new(RouteClientFactory),
            informer_factory: Box::new(InfoClientFactory),
            lander_factory: Box::new(LandingClientFactory),
            proxy_factory: Box::new(ProxyClientFactory),
        }
    }

    async fn check_authentication(&self) -> Result<String, Status> {
        let authenticator = self.authenticator.write().await;
        if !authenticator.is_authenticated().await {
            return Err(Status::unauthenticated("Not authenticated"));
        }

        let access_key = authenticator.get_session().await.access_key.unwrap();
        Ok(access_key.to_owned())
    }

    async fn status(&self, access_key: String) -> Result<StatusResponse, Status> {
        let mut informer = self.informer_factory.get_informer();
        informer
            .initialize()
            .await
            .map_err(|_| Status::internal("Failed to initialize the informer"))?;
        informer
            .get_status(access_key)
            .await
            .map_err(|_| Status::internal("Failed to get status"))
    }

    async fn redeem(
        &self,
        conversation_id: String,
        access_key: String,
        nonce: String,
    ) -> Result<(), Status> {
        let mut router = self.router_factory.get_router();
        router
            .initialize()
            .await
            .map_err(|_| Status::internal("Failed to initialize the router"))?;

        _ = router
            .redeem(conversation_id, access_key, nonce)
            .await
            .map_err(|_| Status::internal("Failed to redeem route"))?;

        Ok(())
    }

    async fn send(
        &self,
        conversation_id: String,
        access_key: String,
        content: &[u8],
    ) -> Result<(), Status> {
        let mut router = self.router_factory.get_router();
        router
            .initialize()
            .await
            .map_err(|_| Status::internal("Failed to initialize the router"))?;
        let response = router
            .get_route(conversation_id.clone(), access_key.clone())
            .await;

        let route = response.map_err(|_| Status::internal("Failed to get route"))?;

        let connection_settings = ConnectionSettings {
            ip: route.ip_address.clone(),
            port: route.port_number as u16,
            domain_name: route.domain_name.clone(),
            certificate: route.public_key.clone(),
        };

        debug!(
            "Routing command to: {:}",
            connection_settings.get_public_endpoint()
        );

        if route.end_route {
            return self
                .land_command(
                    &connection_settings,
                    conversation_id,
                    route.nonce,
                    access_key,
                    content,
                )
                .await;
        }

        self.route_command(&connection_settings, conversation_id, route.nonce, content)
            .await
    }

    async fn land_command(
        &self,
        connection_settings: &ConnectionSettings,
        conversation_id: String,
        nonce: String,
        access_key: String,
        content: &[u8],
    ) -> Result<(), Status> {
        let mut landing_client = self.lander_factory.get_lander(
            connection_settings.get_public_endpoint(),
            connection_settings.certificate.clone(),
            connection_settings.domain_name.clone(),
        );
        landing_client.initialize().await.map_err(|e| {
            Status::internal(format!("Impossible to initialize landing client: {}", e))
        })?;

        _ = landing_client
            .send_message(conversation_id, access_key, nonce, content.to_vec())
            .await
            .map_err(|e| Status::internal(format!("Failed to deliver the message: {}", e)))?;

        Ok(())
    }

    async fn route_command(
        &self,
        connection_settings: &ConnectionSettings,
        conversation_id: String,
        nonce: String,
        content: &[u8],
    ) -> Result<(), Status> {
        let mut proxy_client = self.proxy_factory.get_proxy(
            connection_settings.get_public_endpoint(),
            connection_settings.certificate.clone(),
            connection_settings.domain_name.clone(),
        );
        proxy_client.initialize().await.map_err(|e| {
            Status::internal(format!("Impossible to initialize proxy client: {}", e))
        })?;

        _ = proxy_client
            .send_command(conversation_id, nonce, CommandType::Send, content.to_vec())
            .await
            .map_err(|e| Status::internal(format!("Failed to route command: {}", e)))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use crate::{
        clients::{
            info_client::{MockInformer, MockInformerFactory},
            landing_client::{MockLander, MockLanderFactory, client::TextResponse},
        },
        models::info_proto::StatusResponse,
    };

    use super::*;
    use authorization::auth_client::{ClientSession, MockAuthenticator};
    use protoc_rust::Error;
    use routing::{
        proxy_client::{MockProxy, MockProxyFactory},
        route_client::{
            MockRouter, MockRouterFactory,
            route::{RedeemResponse, RouteResponse, SourceInfo},
        },
    };
    use std::io::ErrorKind;

    const EXPECTED_UID: &str = "L.KD<FCjkSA6AEg@";
    const EXPECTED_ACCESS_KEY: &str = "test_access_key";
    const EXPECTED_NONCE: &str = "test_nonce";
    const EXPECTED_CONVERSATION_ID: &str = "test_conversation";
    const EXPECTED_SENDER_UID: &str = "sender_uid";
    const EXPECTED_PUBLIC_KEY: &[u8] = b"test_public_key";
    const EXPECTED_DOMAIN_NAME: &str = "test_domain_name";

    #[tokio::test]
    async fn given_proxy_not_authenticated_when_execute_command_then_returns_error() {
        let mut mock_authenticator = MockAuthenticator::new();
        mock_authenticator
            .expect_is_authenticated()
            .returning(|| Box::pin(async { false }));

        let authenticator: Arc<RwLock<Box<dyn Authenticator>>> =
            Arc::new(RwLock::new(Box::new(mock_authenticator)));

        let proxy_service = ProxyServiceImpl {
            authenticator: authenticator,
            router_factory: Box::new(MockRouterFactory::new()),
            informer_factory: Box::new(MockInformerFactory::new()),
            lander_factory: Box::new(MockLanderFactory::new()),
            proxy_factory: Box::new(MockProxyFactory::new()),
        };

        let request = Request::new(CommandRequest {
            command: CommandType::Status as i32,
            content: None,
            nonce: EXPECTED_NONCE.to_string(),
            conversation_id: EXPECTED_CONVERSATION_ID.to_string(),
        });

        let response = proxy_service.execute_command(request).await;

        assert!(response.is_err());
        assert_eq!(response.unwrap_err().code(), tonic::Code::Unauthenticated);
    }

    #[tokio::test]
    async fn given_proxy_authenticated_when_execute_command_and_redeem_fails_then_returns_error() {
        let mut mock_authenticator = MockAuthenticator::new();
        mock_authenticator
            .expect_is_authenticated()
            .returning(|| Box::pin(async { true }));

        mock_authenticator.expect_get_session().returning(move || {
            Box::pin(async {
                ClientSession {
                    uid: Some(EXPECTED_UID.to_string()),
                    access_key: Some(EXPECTED_ACCESS_KEY.to_string()),
                }
            })
        });

        let mut router_factory = MockRouterFactory::new();
        router_factory.expect_get_router().returning(|| {
            let mut mock_router = MockRouter::new();
            mock_router
                .expect_redeem()
                .with(
                    mockall::predicate::eq(EXPECTED_CONVERSATION_ID.to_string()),
                    mockall::predicate::eq(EXPECTED_ACCESS_KEY.to_string()),
                    mockall::predicate::eq(EXPECTED_NONCE.to_string()),
                )
                .returning(|_, _, _| {
                    Box::pin(async {
                        Err(Box::<dyn std::error::Error>::from(Error::new(
                            ErrorKind::Other,
                            "Redeem failed",
                        )))
                    })
                });
            Box::new(mock_router)
        });

        let proxy_service = ProxyServiceImpl {
            authenticator: Arc::new(RwLock::new(Box::new(mock_authenticator))),
            router_factory: Box::new(router_factory),
            informer_factory: Box::new(MockInformerFactory::new()),
            lander_factory: Box::new(MockLanderFactory::new()),
            proxy_factory: Box::new(MockProxyFactory::new()),
        };

        let request = Request::new(CommandRequest {
            command: CommandType::Unknown as i32,
            content: None,
            nonce: EXPECTED_NONCE.to_string(),
            conversation_id: EXPECTED_CONVERSATION_ID.to_string(),
        });

        let response = proxy_service.execute_command(request).await;

        assert!(response.is_err());
        assert_eq!(response.unwrap_err().code(), tonic::Code::Internal);
    }

    #[tokio::test]
    async fn given_authenticated_proxy_and_redeem_succeeds_when_execute_unknown_command_then_returns_success()
     {
        let mut mock_authenticator = MockAuthenticator::new();
        mock_authenticator
            .expect_is_authenticated()
            .returning(|| Box::pin(async { true }));

        mock_authenticator.expect_get_session().returning(move || {
            Box::pin(async {
                ClientSession {
                    uid: Some(EXPECTED_UID.to_string()),
                    access_key: Some(EXPECTED_ACCESS_KEY.to_string()),
                }
            })
        });

        let mut router_factory = MockRouterFactory::new();
        router_factory.expect_get_router().returning(|| {
            let mut mock_router = MockRouter::new();
            mock_router.expect_redeem().returning(|_, _, _| {
                Box::pin(async {
                    Ok(RedeemResponse {
                        source_info: Some(SourceInfo {
                            from: EXPECTED_SENDER_UID.to_string(),
                        }),
                    })
                })
            });
            Box::new(mock_router)
        });

        let proxy_service = ProxyServiceImpl {
            authenticator: Arc::new(RwLock::new(Box::new(mock_authenticator))),
            router_factory: Box::new(router_factory),
            informer_factory: Box::new(MockInformerFactory::new()),
            lander_factory: Box::new(MockLanderFactory::new()),
            proxy_factory: Box::new(MockProxyFactory::new()),
        };

        let request = Request::new(CommandRequest {
            command: CommandType::Unknown as i32,
            content: None,
            nonce: EXPECTED_NONCE.to_string(),
            conversation_id: EXPECTED_CONVERSATION_ID.to_string(),
        });

        let response = proxy_service.execute_command(request).await;

        assert!(response.is_ok());
        let command_response = response.unwrap().into_inner();
        assert!(command_response.result.is_none());
    }

    #[tokio::test]
    async fn given_authenticated_proxy_and_redeem_succeeds_when_execute_status_command_then_returns_success()
     {
        let mut mock_authenticator = MockAuthenticator::new();
        mock_authenticator
            .expect_is_authenticated()
            .returning(|| Box::pin(async { true }));

        mock_authenticator.expect_get_session().returning(move || {
            Box::pin(async {
                ClientSession {
                    uid: Some(EXPECTED_UID.to_string()),
                    access_key: Some(EXPECTED_ACCESS_KEY.to_string()),
                }
            })
        });

        let mut router_factory = MockRouterFactory::new();
        router_factory.expect_get_router().returning(|| {
            let mut mock_router = MockRouter::new();
            mock_router.expect_redeem().returning(|_, _, _| {
                Box::pin(async {
                    Ok(RedeemResponse {
                        source_info: Some(SourceInfo {
                            from: EXPECTED_SENDER_UID.to_string(),
                        }),
                    })
                })
            });
            Box::new(mock_router)
        });

        let mut informer_factory = MockInformerFactory::new();
        informer_factory.expect_get_informer().returning(|| {
            let mut mock_informer = MockInformer::new();
            mock_informer
                .expect_get_status()
                .with(mockall::predicate::eq(EXPECTED_ACCESS_KEY.to_string()))
                .returning(|_| {
                    Box::pin(async {
                        Ok(StatusResponse {
                            connected_clients: 5,
                            connected_controllers: 3,
                            connected_proxies: 10,
                            version: "1.0.0".to_string(),
                        })
                    })
                });
            Box::new(mock_informer)
        });

        let proxy_service = ProxyServiceImpl {
            authenticator: Arc::new(RwLock::new(Box::new(mock_authenticator))),
            router_factory: Box::new(router_factory),
            informer_factory: Box::new(informer_factory),
            lander_factory: Box::new(MockLanderFactory::new()),
            proxy_factory: Box::new(MockProxyFactory::new()),
        };

        let request = Request::new(CommandRequest {
            command: CommandType::Status as i32,
            content: None,
            nonce: EXPECTED_NONCE.to_string(),
            conversation_id: EXPECTED_CONVERSATION_ID.to_string(),
        });

        let response = proxy_service.execute_command(request).await;
        assert!(response.is_ok());

        let cmd_response = response.unwrap().into_inner();
        assert!(cmd_response.result.is_some());
    }

    #[tokio::test]
    async fn given_authenticated_proxy_and_redeem_succeeds_without_final_destination_when_execute_send_command_then_returns_success()
     {
        let mut mock_authenticator = MockAuthenticator::new();
        mock_authenticator
            .expect_is_authenticated()
            .returning(|| Box::pin(async { true }));

        mock_authenticator.expect_get_session().returning(move || {
            Box::pin(async {
                ClientSession {
                    uid: Some(EXPECTED_UID.to_string()),
                    access_key: Some(EXPECTED_ACCESS_KEY.to_string()),
                }
            })
        });

        let mut router_factory = MockRouterFactory::new();
        router_factory.expect_get_router().returning(|| {
            let mut mock_router = MockRouter::new();
            mock_router.expect_redeem().returning(|_, _, _| {
                Box::pin(async {
                    Ok(RedeemResponse {
                        source_info: Some(SourceInfo {
                            from: EXPECTED_SENDER_UID.to_string(),
                        }),
                    })
                })
            });

            mock_router.expect_get_route().returning(|_, _| {
                Box::pin(async {
                    Ok(RouteResponse {
                        end_route: false,
                        ip_address: "127.0.0.1".to_string(),
                        port_number: 8080,
                        nonce: EXPECTED_NONCE.to_string(),
                        public_key: EXPECTED_PUBLIC_KEY.to_vec(),
                        domain_name: EXPECTED_DOMAIN_NAME.to_string(),
                    })
                })
            });

            Box::new(mock_router)
        });

        let mut proxy_factory = MockProxyFactory::new();
        proxy_factory.expect_get_proxy().returning(move |_, _, _| {
            let mut mock_proxy = MockProxy::new();
            mock_proxy
                .expect_send_command()
                .with(
                    mockall::predicate::eq(EXPECTED_CONVERSATION_ID.to_string()),
                    mockall::predicate::eq(EXPECTED_NONCE.to_string()),
                    mockall::predicate::eq(CommandType::Send),
                    mockall::predicate::eq(b"Test message".to_vec()),
                )
                .returning(|_, _, _, _| {
                    Box::pin(async {
                        Ok(routing::proxy_client::CommandResponse {
                            result: Some("Message sent".to_string()),
                        })
                    })
                });
            Box::new(mock_proxy)
        });

        let proxy_service = ProxyServiceImpl {
            authenticator: Arc::new(RwLock::new(Box::new(mock_authenticator))),
            router_factory: Box::new(router_factory),
            informer_factory: Box::new(MockInformerFactory::new()),
            lander_factory: Box::new(MockLanderFactory::new()),
            proxy_factory: Box::new(proxy_factory),
        };

        let request = Request::new(CommandRequest {
            command: CommandType::Send as i32,
            content: Some(b"Test message".to_vec()),
            nonce: EXPECTED_NONCE.to_string(),
            conversation_id: EXPECTED_CONVERSATION_ID.to_string(),
        });

        let response = proxy_service.execute_command(request).await;

        assert!(response.is_ok());
        let command_response = response.unwrap().into_inner();
        assert!(command_response.result.is_some());
        assert_eq!(command_response.result.unwrap(), "Message sent");
    }

    #[tokio::test]
    async fn given_authenticated_proxy_and_redeem_succeeds_with_final_destination_when_execute_send_command_then_returns_success()
     {
        let mut mock_authenticator = MockAuthenticator::new();
        mock_authenticator
            .expect_is_authenticated()
            .returning(|| Box::pin(async { true }));

        mock_authenticator.expect_get_session().returning(move || {
            Box::pin(async {
                ClientSession {
                    uid: Some(EXPECTED_UID.to_string()),
                    access_key: Some(EXPECTED_ACCESS_KEY.to_string()),
                }
            })
        });

        let mut router_factory = MockRouterFactory::new();
        router_factory.expect_get_router().returning(|| {
            let mut mock_router = MockRouter::new();
            mock_router.expect_redeem().returning(|_, _, _| {
                Box::pin(async {
                    Ok(RedeemResponse {
                        source_info: Some(SourceInfo {
                            from: EXPECTED_SENDER_UID.to_string(),
                        }),
                    })
                })
            });

            mock_router.expect_get_route().returning(|_, _| {
                Box::pin(async {
                    Ok(RouteResponse {
                        end_route: true,
                        ip_address: "127.0.0.1".to_string(),
                        port_number: 8080,
                        nonce: EXPECTED_NONCE.to_string(),
                        public_key: EXPECTED_PUBLIC_KEY.to_vec(),
                        domain_name: EXPECTED_DOMAIN_NAME.to_string(),
                    })
                })
            });

            Box::new(mock_router)
        });

        let mut lander_factory = MockLanderFactory::new();
        lander_factory
            .expect_get_lander()
            .returning(move |_, _, _| {
                let mut mock_lander = MockLander::new();
                mock_lander
                    .expect_send_message()
                    .with(
                        mockall::predicate::eq(EXPECTED_CONVERSATION_ID.to_string()),
                        mockall::predicate::eq(EXPECTED_ACCESS_KEY.to_string()),
                        mockall::predicate::eq(EXPECTED_NONCE.to_string()),
                        mockall::predicate::eq(b"Test message".to_vec()),
                    )
                    .returning(|_, _, _, _| Box::pin(async { Ok(TextResponse {}) }));
                Box::new(mock_lander)
            });

        let proxy_service = ProxyServiceImpl {
            authenticator: Arc::new(RwLock::new(Box::new(mock_authenticator))),
            router_factory: Box::new(router_factory),
            informer_factory: Box::new(MockInformerFactory::new()),
            lander_factory: Box::new(lander_factory),
            proxy_factory: Box::new(MockProxyFactory::new()),
        };

        let request = Request::new(CommandRequest {
            command: CommandType::Send as i32,
            content: Some(b"Test message".to_vec()),
            nonce: EXPECTED_NONCE.to_string(),
            conversation_id: EXPECTED_CONVERSATION_ID.to_string(),
        });

        let response = proxy_service.execute_command(request).await;

        assert!(response.is_ok());
        let command_response = response.unwrap().into_inner();
        assert!(command_response.result.is_some());
        assert_eq!(command_response.result.unwrap(), "Message sent");
    }
}
