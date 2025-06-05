use crosscutting::settings::service;
use std::env;

#[test]
fn get_controller_connection_settings_valid() {
    unsafe {
        env::set_var("CONTROLLER_IP", "192.168.1.1");
        env::set_var("CONTROLLER_PORT", "8080");
    }
    let result = service::get_controller_connection_settings();
    assert!(result.is_ok());
    let (ip, port) = result.unwrap();
    assert_eq!(ip, "192.168.1.1");
    assert_eq!(port, 8080);
}

#[test]
fn get_controller_connection_settings_missing_ip() {
    unsafe {
        env::remove_var("CONTROLLER_IP");
        env::set_var("CONTROLLER_PORT", "8080");
    }
    let result = service::get_controller_connection_settings();
    assert!(result.is_err());
}

#[test]
fn get_controller_connection_settings_invalid_port() {
    unsafe {
        env::set_var("CONTROLLER_IP", "192.168.1.1");
        env::set_var("CONTROLLER_PORT", "invalid_port");
    }
    let result = service::get_controller_connection_settings();
    assert!(result.is_err());
}

#[test]
fn get_service_connection_settings_valid() {
    unsafe {
        env::set_var("LISTENING_IP", "127.0.0.1");
        env::set_var("LISTENING_PORT", "9090");
    }
    let result = service::get_service_connection_settings();
    assert!(result.is_ok());
    let (ip, port) = result.unwrap();
    assert_eq!(ip, "127.0.0.1");
    assert_eq!(port, 9090);
}

#[test]
fn get_service_connection_settings_missing_ip() {
    unsafe {
        env::remove_var("LISTENING_IP");
        env::set_var("LISTENING_PORT", "9090");
    }
    let result = service::get_service_connection_settings();
    assert!(result.is_err());
}

#[test]
fn get_service_connection_settings_invalid_port() {
    unsafe {
        env::set_var("LISTENING_IP", "127.0.0.1");
        env::set_var("LISTENING_PORT", "invalid_port");
    }
    let result = service::get_service_connection_settings();
    assert!(result.is_err());
}

#[test]
fn get_controller_domain_name_valid() {
    unsafe {
        env::set_var("CONTROLLER_DOMAIN_NAME", "example.com");
    }
    let result = service::get_controller_domain_name();
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "example.com");
}

#[test]
fn get_controller_domain_name_missing() {
    unsafe {
        env::remove_var("CONTROLLER_DOMAIN_NAME");
    }
    let result = service::get_controller_domain_name();
    assert!(result.is_err());
}
