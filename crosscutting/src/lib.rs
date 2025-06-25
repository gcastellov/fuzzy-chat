pub mod abstractions;
pub mod networking;
pub mod settings;
pub mod tracing;

use http::Uri;
use std::{error::Error, net::SocketAddr};

#[derive(Clone, PartialEq, Debug)]
pub enum Component {
    Controller,
    Proxy,
    Client,
}

#[derive(Clone)]
pub enum ComponentDescriptor {
    Controller {
        credentials: Credentials,
        connection_settings: ConnectionSettings,
        version: String,
    },
    Proxy {
        credentials: Credentials,
        connection_settings: ConnectionSettings,
    },
    Client {
        credentials: Credentials,
        connection_settings: ConnectionSettings,
    },
}

#[derive(Clone)]
pub struct ConnectionSettings {
    pub ip: String,
    pub port: u16,
    pub domain_name: String,
    pub certificate: Vec<u8>,
}

#[derive(Clone)]
pub struct Credentials {
    pub uid: String,
    pub pwd: String,
}

impl From<u8> for Component {
    fn from(value: u8) -> Self {
        Component::from(value as i32)
    }
}

impl From<i32> for Component {
    fn from(value: i32) -> Self {
        match value {
            0 => Component::Controller,
            1 => Component::Proxy,
            2 => Component::Client,
            _ => panic!("Invalid value for ComponentType: {}", value),
        }
    }
}

impl From<Component> for u8 {
    fn from(value: Component) -> Self {
        match value {
            Component::Controller => 0,
            Component::Proxy => 1,
            Component::Client => 2,
        }
    }
}

impl From<Component> for i32 {
    fn from(value: Component) -> Self {
        u8::from(value) as i32
    }
}

impl std::fmt::Display for Component {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Component::Controller => write!(f, "Controller"),
            Component::Proxy => write!(f, "Proxy"),
            Component::Client => write!(f, "Client"),
        }
    }
}

impl Component {
    pub fn get_connection_settings(&self) -> Result<ConnectionSettings, Box<dyn Error>> {
        match self {
            Component::Controller => settings::service::get_controller_connection_settings()
                .or(settings::service::get_connection_settings()),
            _ => settings::service::get_connection_settings(),
        }
    }
}

impl ComponentDescriptor {
    pub fn load(component_type: Component) -> Result<Self, Box<dyn Error>> {
        let connection_settings = component_type.get_connection_settings()?;
        let credentials = settings::auth::get_credentials()?;

        let descriptor = match component_type {
            Component::Controller => ComponentDescriptor::Controller {
                credentials,
                connection_settings,
                version: String::from("1.0.0"),
            },
            Component::Proxy => ComponentDescriptor::Proxy {
                credentials,
                connection_settings,
            },
            Component::Client => ComponentDescriptor::Client {
                credentials,
                connection_settings,
            },
        };

        Ok(descriptor)
    }

    pub fn get_connection_settings(&self) -> &ConnectionSettings {
        match self {
            ComponentDescriptor::Controller {
                connection_settings,
                ..
            } => connection_settings,
            ComponentDescriptor::Proxy {
                connection_settings,
                ..
            } => connection_settings,
            ComponentDescriptor::Client {
                connection_settings,
                ..
            } => connection_settings,
        }
    }

    pub fn get_credentials(&self) -> &Credentials {
        match self {
            ComponentDescriptor::Controller { credentials, .. } => credentials,
            ComponentDescriptor::Proxy { credentials, .. } => credentials,
            ComponentDescriptor::Client { credentials, .. } => credentials,
        }
    }
}

impl From<&ComponentDescriptor> for Component {
    fn from(value: &ComponentDescriptor) -> Self {
        match value {
            ComponentDescriptor::Controller { .. } => Component::Controller,
            ComponentDescriptor::Proxy { .. } => Component::Proxy,
            ComponentDescriptor::Client { .. } => Component::Client,
        }
    }
}

impl ConnectionSettings {
    pub fn get_public_endpoint(&self) -> Uri {
        networking::to_https_endpoint(self.ip.as_str(), self.port as u32).unwrap()
    }

    pub fn get_public_socket_address(&self) -> SocketAddr {
        networking::to_socket_address(self.ip.as_str(), self.port).unwrap()
    }

    pub fn get_local_socket_address(&self) -> SocketAddr {
        networking::to_socket_address("0.0.0.0", self.port).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_byte_to_component_type() {
        assert_eq!(Component::from(0), Component::Controller);
        assert_eq!(Component::from(1), Component::Proxy);
        assert_eq!(Component::from(2), Component::Client);
        assert!(std::panic::catch_unwind(|| Component::from(3)).is_err());
    }
}
