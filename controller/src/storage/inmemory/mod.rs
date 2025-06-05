pub mod member_repository;
pub mod route_repository;
pub mod session_repository;

use std::time::Duration;
use tokio::time::Instant;

pub struct ExpirationWrapper<T> {
    pub value: T,
    pub expires_at: Instant,
    expiration_time: Duration,
}

impl<T> ExpirationWrapper<T> {
    pub fn new(value: T, expiration_time: Duration) -> Self {
        ExpirationWrapper {
            value,
            expires_at: Instant::now() + expiration_time,
            expiration_time,
        }
    }

    pub fn is_expired(&self) -> bool {
        self.expires_at < Instant::now()
    }

    pub fn renew(&mut self) {
        self.expires_at = Instant::now() + self.expiration_time;
    }
}
