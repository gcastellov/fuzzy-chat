pub mod auth_proto {
    tonic::include_proto!("auth");
}

pub mod info_proto {
    tonic::include_proto!("info");
}

pub mod route_proto {
    tonic::include_proto!("route");
}

use crosscutting::ConnectionSettings;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    pub on_ip_address: String,
    pub on_port_number: u16,
    pub public_key: Vec<u8>,
    pub domain_name: String,
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
    pub public_key: Vec<u8>,
    pub domain_name: String,
}

impl SessionInfo {
    pub fn to_connection_settings(&self) -> ConnectionSettings {
        ConnectionSettings {
            ip: self.on_ip_address.clone(),
            port: self.on_port_number,
            domain_name: self.domain_name.clone(),
            certificate: self.public_key.clone(),
        }
    }
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
