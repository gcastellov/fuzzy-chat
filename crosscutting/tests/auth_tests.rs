use crosscutting::settings::auth;
use std::env;

#[test]
fn get_credentials_valid() {
    unsafe {
        env::set_var("UID", "user");
        env::set_var("PWD", "password");
    }
    let result = auth::get_credentials();
    assert!(result.is_ok());
    let credentials = result.unwrap();
    assert_eq!(credentials.uid, "user");
    assert_eq!(credentials.pwd, "password");
}

#[test]
fn get_credentials_missing_uid() {
    unsafe {
        env::remove_var("UID");
        env::set_var("PWD", "password");
    }
    let result = auth::get_credentials();
    assert!(result.is_err());
}

#[test]
fn get_credentials_missing_pwd() {
    unsafe {
        env::set_var("UID", "user");
        env::remove_var("PWD");
    }
    let result = auth::get_credentials();
    assert!(result.is_err());
}

#[test]
fn get_credentials_missing_both() {
    unsafe {
        env::remove_var("UID");
        env::remove_var("PWD");
    }
    let result = auth::get_credentials();
    assert!(result.is_err());
}
