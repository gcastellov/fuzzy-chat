use super::*;
use std::{env, ffi::OsString, str::FromStr};

const LOGS_DIR_KEY: &str = "LOGS_DIR";
const CERTS_DIR_KEY: &str = "CERTS_DIR";

pub fn get_certificates_dir() -> OsString {
    self::get_env_variable(CERTS_DIR_KEY)
        .map(OsString::from)
        .unwrap_or(env::current_dir().unwrap().into_os_string())
}

pub fn get_logs_dir() -> OsString {
    self::get_env_variable(LOGS_DIR_KEY)
        .map(OsString::from)
        .unwrap_or(env::current_dir().unwrap().into_os_string())
}

pub fn get_env_variable(var_name: &str) -> Result<String, Box<dyn Error>> {
    env::var(var_name).map_err(|_| format!("Environment variable {} not set", var_name).into())
}

pub fn get_hostname() -> String {
    hostname::get()
        .unwrap_or_else(|_| OsString::from_str("localhost").unwrap())
        .to_string_lossy()
        .into_owned()
}
