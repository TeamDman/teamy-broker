mod app_home;
mod cache;

pub use app_home::*;
pub use cache::*;

// TODO(template): replace these environment variable names and directory names in generated repositories.
pub const APP_HOME_ENV_VAR: &str = "TEAMY_BROKER_HOME_DIR"; // TODO: REPLACE ME
pub const TEAMY_BROKER_HOME_DIR_NAME: &str = "teamy-broker"; // TODO: REPLACE ME

pub const APP_CACHE_ENV_VAR: &str = "TEAMY_BROKER_CACHE_DIR"; // TODO: REPLACE ME
pub const TEAMY_BROKER_CACHE_DIR_NAME: &str = "teamy-broker"; // TODO: REPLACE ME
