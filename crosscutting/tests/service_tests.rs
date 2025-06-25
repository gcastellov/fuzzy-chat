use crosscutting::settings::service;
use std::env;

const CERTS_DIR_KEY: &str = "CERTS_DIR";
const DOMAIN_NAME_KEY: &str = "DOMAIN_NAME";
const LISTENING_IP_KEY: &str = "LISTENING_IP";
const LISTENING_PORT_KEY: &str = "LISTENING_PORT";
const CONTROLLER_IP_KEY: &str = "CONTROLLER_IP";
const CONTROLLER_PORT_KEY: &str = "CONTROLLER_PORT";
const CONTROLLER_CERT_FILE_KEY: &str = "CONTROLLER_CERT_FILE";
const CONTROLLER_DOMAIN_NAME_KEY: &str = "CONTROLLER_DOMAIN_NAME";

#[test]
fn get_service_endpoint_valid() {
    unsafe {
        env::set_var(LISTENING_IP_KEY, "127.0.0.1");
        env::set_var(LISTENING_PORT_KEY, "9090");
        env::set_var(DOMAIN_NAME_KEY, "localhost");
        env::set_var(CERTS_DIR_KEY, "../assets/certs");
    }

    let result = service::get_connection_settings();
    assert!(result.is_ok());
}

#[test]
fn get_controller_connection_settings_valid() {
    unsafe {
        env::set_var(CONTROLLER_IP_KEY, "127.0.0.1");
        env::set_var(CONTROLLER_PORT_KEY, "9090");
        env::set_var(CONTROLLER_DOMAIN_NAME_KEY, "localhost");
        env::set_var(CONTROLLER_CERT_FILE_KEY, "../assets/certs/ca.crt");
    }

    let result = service::get_controller_connection_settings();
    assert!(result.is_ok());
}
