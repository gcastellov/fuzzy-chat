use tokio::time::Instant;

pub mod client_proto {
    tonic::include_proto!("client");
}

pub mod auth_proto {
    tonic::include_proto!("auth");
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct TextMessage {
    pub from: String,
    pub to: String,
    pub content: String,
    pub at: Instant,
}

impl TextMessage {
    pub fn new(from: String, to: String, content: &[u8]) -> Self {
        Self {
            from,
            to,
            content: String::from_utf8_lossy(content).into(),
            at: Instant::now(),
        }
    }
}
