use std::{
    fs,
    path::{Path, PathBuf},
    str::FromStr,
    time::SystemTime,
};

const LOG_LEVEL_KEY: &str = "LOG_LEVEL";
const DEFAULT_LOG_LEVEL: &str = "debug";
const LOG_DIR: &str = "logs";

pub fn setup_logger(file_name: &str) -> Result<(), fern::InitError> {
    let log_dir = self::get_logs_path();

    if !log_dir.exists() {
        fs::create_dir_all(log_dir.clone())?;
    }

    let file_name = log_dir.join(file_name);
    let log_level = super::environment::get_env_variable(LOG_LEVEL_KEY)
        .unwrap_or(DEFAULT_LOG_LEVEL.to_string());
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
