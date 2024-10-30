use std::path::PathBuf;

use super::MONOCORE_HOME_ENV_VAR;

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

/// The sub directory where monocore artifacts, configs, etc are stored.
pub const MONOCORE_SUBDIR: &str = ".monocore";

/// The OCI sub directory.
pub const OCI_SUBDIR: &str = "oci";

/// The sub directory where monocore OCI image layers are cached.
pub const IMAGE_LAYERS_SUBDIR: &str = "layers";

/// The sub directory where monocore OCI image index, configurations, etc. are cached.
pub const IMAGE_REPOS_SUBDIR: &str = "repos";

/// The sub directory where runtime state is stored.
pub const STATE_SUBDIR: &str = "run";

/// The sub directory where runtime logs are stored.
pub const LOG_SUBDIR: &str = "log";

lazy_static::lazy_static! {
    /// The path where all monocore artifacts, configs, etc are stored.
    pub static ref DEFAULT_MONOCORE_HOME: PathBuf = dirs::home_dir().unwrap().join(MONOCORE_SUBDIR);

    /// The directory where the micro VM state is stored.
    pub static ref MICROVM_STATE_DIR: PathBuf = monocore_home_path().join(STATE_SUBDIR);

    /// The directory where the micro VM logs are stored.
    pub static ref MICROVM_LOG_DIR: PathBuf = monocore_home_path().join(LOG_SUBDIR);
}

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Returns the path where all monocore artifacts, configs, etc are stored.
pub fn monocore_home_path() -> PathBuf {
    if let Ok(monocore_home) = std::env::var(MONOCORE_HOME_ENV_VAR) {
        PathBuf::from(monocore_home)
    } else {
        DEFAULT_MONOCORE_HOME.to_owned()
    }
}
