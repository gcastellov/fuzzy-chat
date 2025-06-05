use mockall::automock;
use std::error::Error;
use tonic::async_trait;

#[async_trait]
#[automock]
pub trait GrpcClient: Send + Sync {
    async fn initialize(&mut self) -> Result<(), Box<dyn Error>>;
}
