use crate::Credentials;

use super::*;

pub fn get_credentials() -> Result<Credentials, Box<dyn Error>> {
    let uid = super::environment::get_env_variable("UID").map_err(|_| "UID not set")?;
    let pwd = super::environment::get_env_variable("PWD").map_err(|_| "PWD not set")?;

    Ok(Credentials { uid, pwd })
}
