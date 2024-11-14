use std::path::PathBuf;

use super::MONOCORE_HOME_ENV_VAR;

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

/// The sub directory where monocore artifacts, configs, etc are stored.
pub const MONOCORE_SUBDIR: &str = ".monocore";

/// The OCI sub directory where the OCI image layers, index, configurations, etc are stored.
pub const OCI_SUBDIR: &str = "oci";

/// The sub directory where monocore OCI image layers are cached.
pub const OCI_LAYER_SUBDIR: &str = "layer";

/// The sub directory where monocore OCI image index, configurations, etc. are cached.
pub const OCI_REPO_SUBDIR: &str = "repo";

/// The filename for the OCI image index JSON file
pub const OCI_INDEX_FILENAME: &str = "index.json";

/// The filename for the OCI image manifest JSON file
pub const OCI_MANIFEST_FILENAME: &str = "manifest.json";

/// The filename for the OCI image config JSON file
pub const OCI_CONFIG_FILENAME: &str = "config.json";

/// The rootfs sub directory where the rootfs and other related files associated with the microvm are stored.
pub const ROOTFS_SUBDIR: &str = "rootfs";

/// The reference sub directory where the reference rootfs is stored.
pub const REFERENCE_SUBDIR: &str = "reference";

/// The services sub directory where the services (rootfs) are stored.
pub const SERVICE_SUBDIR: &str = "service";

/// The merged sub directory where the merged rootfs is stored.
pub const MERGED_SUBDIR: &str = "merged";

/// The sub directory where runtime state is stored.
pub const STATE_SUBDIR: &str = "run";

/// The sub directory where runtime logs are stored.
pub const LOG_SUBDIR: &str = "log";

lazy_static::lazy_static! {
    /// The path where all monocore artifacts, configs, etc are stored.
    pub static ref DEFAULT_MONOCORE_HOME: PathBuf = dirs::home_dir().unwrap().join(MONOCORE_SUBDIR);

    /// The path to the monocore OCI directory
    pub static ref MONOCORE_OCI_DIR: PathBuf = monocore_home_path().join(OCI_SUBDIR);

    /// The path to the monocore rootfs directory
    pub static ref MONOCORE_ROOTFS_DIR: PathBuf = monocore_home_path().join(ROOTFS_SUBDIR);

    /// The path to the monocore service directory
    pub static ref MONOCORE_SERVICE_DIR: PathBuf = monocore_home_path().join(SERVICE_SUBDIR);

    /// The path to the monocore state directory (e.g. for storing service state files)
    pub static ref MONOCORE_STATE_DIR: PathBuf = monocore_home_path().join(STATE_SUBDIR);

    /// The path to the monocore log directory (e.g. for storing service stdout/stderr logs)
    pub static ref MONOCORE_LOG_DIR: PathBuf = monocore_home_path().join(LOG_SUBDIR);
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
