use std::error::Error;
use std::path::{Path, PathBuf};

pub mod environment {
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
}

pub mod logging {
    use super::*;
    use std::{fs, str::FromStr, time::SystemTime};

    const LOG_LEVEL_KEY: &str = "LOG_LEVEL";
    const DEFAULT_LOG_LEVEL: &str = "debug";
    const LOG_DIR: &str = "logs";

    pub fn setup_logger(file_name: &str) -> Result<(), fern::InitError> {
        let log_dir = self::get_logs_path();

        if !log_dir.exists() {
            fs::create_dir_all(log_dir.clone())?;
        }

        let file_name = log_dir.join(file_name);
        let log_level =
            environment::get_env_variable(LOG_LEVEL_KEY).unwrap_or(DEFAULT_LOG_LEVEL.to_string());
        let log_level = log::LevelFilter::from_str(&log_level).unwrap_or(log::LevelFilter::Debug);

        fern::Dispatch::new()
            .format(|out, message, record| {
                out.finish(format_args!(
                    "[{} {} {}] {}",
                    humantime::format_rfc3339_seconds(SystemTime::now()),
                    record.level(),
                    record.target(),
                    message
                ))
            })
            .level(log_level)
            .chain(std::io::stdout())
            .chain(fern::log_file(file_name)?)
            .apply()?;
        Ok(())
    }

    pub fn get_default_log_file_name(component_type: &str) -> String {
        let hostname = super::environment::get_hostname();
        format!("{}-{}.log", hostname, component_type)
    }

    fn get_logs_path() -> PathBuf {
        let base_path = super::environment::get_logs_dir();
        let base_path = Path::new(base_path.to_str().unwrap());
        if !base_path.ends_with(LOG_DIR) {
            return base_path.join(LOG_DIR);
        }

        base_path.to_path_buf()
    }
}

pub mod auth {
    use super::*;

    pub fn get_credentials() -> Result<(String, String), Box<dyn Error>> {
        let uid = super::environment::get_env_variable("UID").map_err(|_| "UID not set")?;
        let pwd = super::environment::get_env_variable("PWD").map_err(|_| "PWD not set")?;

        Ok((uid, pwd))
    }
}

pub mod service {
    use super::*;

    pub fn get_controller_connection_settings() -> Result<(String, u16), Box<dyn Error>> {
        let controller_ip = super::environment::get_env_variable("CONTROLLER_IP")
            .map_err(|_| "Controller IP not set")?;
        let controller_port = super::environment::get_env_variable("CONTROLLER_PORT")
            .map_err(|_| "Controller port not set")?
            .parse::<u16>()
            .map_err(|_| "Invalid port number")?;

        Ok((controller_ip, controller_port))
    }

    pub fn get_service_connection_settings() -> Result<(String, u16), Box<dyn Error>> {
        let ip = super::environment::get_env_variable("LISTENING_IP").map_err(|_| "IP not set")?;
        let port = super::environment::get_env_variable("LISTENING_PORT")
            .map_err(|_| "port not set")?
            .parse::<u16>()
            .map_err(|_| "Invalid port number")?;

        Ok((ip, port))
    }

    pub fn get_controller_domain_name() -> Result<String, Box<dyn Error>> {
        let domain_name = super::environment::get_env_variable("CONTROLLER_DOMAIN_NAME")
            .map_err(|_| "Controller domain name not set")?;
        Ok(domain_name)
    }
}

pub mod component {

    use std::net::SocketAddr;

    use super::*;
    use crate::networking::to_socket_address;

    pub struct DescriptorBuilder {
        pub uid: String,
        pub pwd: String,
        pub on_ip: String,
        pub on_port: u16,
        pub version: Option<String>,
        pub component_type: Option<u8>,
    }

    #[derive(Clone)]
    pub struct Descriptor {
        pub uid: String,
        pub pwd: String,
        pub on_ip: String,
        pub on_port: u16,
        pub version: String,
        pub component_type: u8,
    }

    impl Descriptor {
        pub fn on_public_socket_address(&self) -> SocketAddr {
            to_socket_address(self.on_ip.as_str(), self.on_port).unwrap()
        }

        pub fn on_local_socket_address(&self) -> SocketAddr {
            to_socket_address("0.0.0.0", self.on_port).unwrap()
        }
    }

    impl DescriptorBuilder {
        pub fn load() -> Result<Self, Box<dyn Error>> {
            let (on_ip, on_port) = super::service::get_service_connection_settings()?;
            let (uid, pwd) = super::auth::get_credentials()?;

            Ok(DescriptorBuilder {
                uid,
                pwd,
                on_ip,
                on_port,
                component_type: None,
                version: None,
            })
        }

        pub fn with_component_type(&mut self, component_type: u8) -> &mut Self {
            self.component_type = Some(component_type);
            self
        }

        pub fn with_version(&mut self, version: &str) -> &mut Self {
            self.version = Some(version.to_string());
            self
        }

        pub fn build(&self) -> Result<Descriptor, Box<dyn Error>> {
            if self.component_type.is_none() {
                return Err("Provide the component type first".into());
            }

            Ok(Descriptor {
                uid: self.uid.clone(),
                pwd: self.pwd.clone(),
                on_ip: self.on_ip.clone(),
                on_port: self.on_port,
                version: self.version.clone().unwrap_or_else(|| "0.0.0".to_string()),
                component_type: self.component_type.unwrap(),
            })
        }
    }
}
