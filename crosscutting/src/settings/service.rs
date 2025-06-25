use crate::ConnectionSettings;

use super::*;
use std::{fs, path::PathBuf};
use tonic::transport::Identity;

const CONTROLLER_CERT_FILE_KEY: &str = "CONTROLLER_CERT_FILE";
const CONTROLLER_DOMAIN_NAME_KEY: &str = "CONTROLLER_DOMAIN_NAME";
const DOMAIN_NAME_KEY: &str = "DOMAIN_NAME";
const LISTENING_IP_KEY: &str = "LISTENING_IP";
const LISTENING_PORT_KEY: &str = "LISTENING_PORT";
const CONTROLLER_IP_KEY: &str = "CONTROLLER_IP";
const CONTROLLER_PORT_KEY: &str = "CONTROLLER_PORT";

pub fn load_tls_identity(cert_file: &str, key_file: &str) -> Result<Identity, Box<dyn Error>> {
    let path = PathBuf::from(crate::settings::environment::get_certificates_dir());
    let cert_path = path.join(cert_file);
    let key_path = path.join(key_file);
    let cert = std::fs::read(cert_path)?;
    let key = std::fs::read(key_path)?;
    Ok(Identity::from_pem(cert, key))
}

pub fn get_controller_connection_settings() -> Result<ConnectionSettings, Box<dyn Error>> {
    let (ip, port) = get_service_endpoint(CONTROLLER_IP_KEY, CONTROLLER_PORT_KEY)?;
    Ok(ConnectionSettings {
        ip: ip.clone(),
        port,
        domain_name: get_domain_name(CONTROLLER_DOMAIN_NAME_KEY)?,
        certificate: get_controller_cert_file()?,
    })
}

pub fn get_connection_settings() -> Result<ConnectionSettings, Box<dyn Error>> {
    let (ip, port) = get_service_endpoint(LISTENING_IP_KEY, LISTENING_PORT_KEY)?;
    Ok(ConnectionSettings {
        ip: ip.clone(),
        port,
        domain_name: get_domain_name(DOMAIN_NAME_KEY)?,
        certificate: get_cert_file()?,
    })
}

fn get_service_endpoint(ip_env_var: &str, port_env_var: &str) -> Result<(String, u16), Box<dyn Error>> {
    let ip = super::environment::get_env_variable(ip_env_var)
        .map_err(|_| "IP not set")?;
    let port = super::environment::get_env_variable(port_env_var)
        .map_err(|_| "port not set")?
        .parse::<u16>()
        .map_err(|_| "Invalid port number")?;

    Ok((ip, port))
}

fn get_domain_name(domain_env_var: &str) -> Result<String, Box<dyn Error>> {
    let domain_name =
        super::environment::get_env_variable(domain_env_var).map_err(|_| "Domain name not set")?;
    Ok(domain_name)
}

fn get_controller_cert_file() -> Result<Vec<u8>, Box<dyn Error>> {
    super::environment::get_env_variable(CONTROLLER_CERT_FILE_KEY)
        .map(|cert_path| fs::read(cert_path).unwrap_or_default())
}

fn get_cert_file() -> Result<Vec<u8>, Box<dyn Error>> {
    let path = PathBuf::from(super::environment::get_certificates_dir());
    let cert_path = path.join("ca.crt").to_string_lossy().to_string();
    let cert = std::fs::read(cert_path)?;
    Ok(cert)
}

#[cfg(test)]
mod tests {
    use std::env;

    use super::*;

    #[test]
    fn get_controller_endpoint_valid() {
        unsafe {
            env::set_var(CONTROLLER_IP_KEY, "192.168.1.1");
            env::set_var(CONTROLLER_PORT_KEY, "8080");
        }
        let result = service::get_service_endpoint(CONTROLLER_IP_KEY, CONTROLLER_PORT_KEY);
        assert!(result.is_ok());
        let (ip, port) = result.unwrap();
        assert_eq!(ip, "192.168.1.1");
        assert_eq!(port, 8080);
    }

    #[test]
    fn get_controller_connection_settings_missing_ip() {
        unsafe {
            env::remove_var(CONTROLLER_IP_KEY);
            env::set_var(CONTROLLER_PORT_KEY, "8080");
        }
        let result = service::get_controller_connection_settings();
        assert!(result.is_err());
    }

    #[test]
    fn get_controller_connection_settings_invalid_port() {
        unsafe {
            env::set_var(CONTROLLER_IP_KEY, "192.168.1.1");
            env::set_var(CONTROLLER_PORT_KEY, "invalid_port");
        }
        let result = service::get_controller_connection_settings();
        assert!(result.is_err());
    }

    #[test]
    fn get_controller_domain_name_valid() {
        unsafe {
            env::set_var(CONTROLLER_DOMAIN_NAME_KEY, "example.com");
        }
        let result = service::get_domain_name(CONTROLLER_DOMAIN_NAME_KEY);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "example.com");
    }

    #[test]
    fn get_controller_domain_name_missing() {
        unsafe {
            env::remove_var(CONTROLLER_DOMAIN_NAME_KEY);
        }
        let result = service::get_domain_name(CONTROLLER_DOMAIN_NAME_KEY);
        assert!(result.is_err());
    }
}
