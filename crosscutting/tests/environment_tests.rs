use crosscutting::settings::environment;
use std::env;

#[test]
fn get_certificates_dir_default() {
    unsafe {
        env::remove_var("CERTS_DIR");
    }
    let certs_dir = environment::get_certificates_dir();
    assert!(certs_dir.to_str().is_some());
}

#[test]
fn get_certificates_dir_custom() {
    unsafe {
        env::set_var("CERTS_DIR", "/custom/certs");
    }
    let certs_dir = environment::get_certificates_dir();
    assert_eq!(certs_dir.to_str().unwrap(), "/custom/certs");
}

#[test]
fn get_logs_dir_default() {
    unsafe {
        env::remove_var("LOGS_DIR");
    }
    let logs_dir = environment::get_logs_dir();
    assert!(logs_dir.to_str().is_some());
}

#[test]
fn get_logs_dir_custom() {
    unsafe {
        env::set_var("LOGS_DIR", "/custom/logs");
    }
    let logs_dir = environment::get_logs_dir();
    assert_eq!(logs_dir.to_str().unwrap(), "/custom/logs");
}

#[test]
fn get_env_variable_existing() {
    unsafe {
        env::set_var("TEST_VAR", "value");
    }
    let result = environment::get_env_variable("TEST_VAR");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "value");
}

#[test]
fn get_env_variable_missing() {
    unsafe {
        env::remove_var("MISSING_VAR");
    }
    let result = environment::get_env_variable("MISSING_VAR");
    assert!(result.is_err());
}

#[test]
fn get_hostname_default() {
    let hostname = environment::get_hostname();
    assert!(!hostname.is_empty());
}
