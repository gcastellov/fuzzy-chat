pub mod auth_proto {
    tonic::include_proto!("auth");
}

pub mod info_proto {
    tonic::include_proto!("info");
}

pub mod route_proto {
    tonic::include_proto!("route");
}

use auth_proto::ComponentType;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    pub on_ip_address: String,
    pub on_port_number: u16,
    pub nonce: String,
    pub end_route: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub from: String,
    pub to: String,
    pub routing_id: u8,
    pub routes: Vec<Route>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Member {
    pub uid: String,
    pub pwd: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub access_key: String,
    pub uid: String,
    pub client_ip: SocketAddr,
    pub on_port_number: u16,
    pub on_ip_address: String,
    pub component_type: u8,
}

impl Conversation {
    pub fn new(id: String, from: String, to: String, routing_id: u8) -> Self {
        Self {
            id,
            from,
            to,
            routing_id,
            routes: Vec::new(),
        }
    }
}

impl Member {
    pub fn new(uid: String, pwd: String) -> Self {
        Self { uid, pwd }
    }
}

impl From<u8> for ComponentType {
    fn from(value: u8) -> Self {
        match value {
            0 => ComponentType::Controller,
            1 => ComponentType::Proxy,
            2 => ComponentType::Client,
            _ => panic!("Invalid value for ComponentType: {}", value),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_byte_to_component_type() {
        assert_eq!(ComponentType::from(0), ComponentType::Controller);
        assert_eq!(ComponentType::from(1), ComponentType::Proxy);
        assert_eq!(ComponentType::from(2), ComponentType::Client);
        assert!(std::panic::catch_unwind(|| ComponentType::from(3)).is_err());
    }
}
