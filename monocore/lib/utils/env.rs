//! Utility functions for working with environment variables.

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

use std::path::PathBuf;

use crate::config::{DEFAULT_MONOCORE_HOME, DEFAULT_OCI_REGISTRY};

/// Environment variable for the monocore home directory
pub const MONOCORE_HOME_ENV_VAR: &str = "MONOCORE_HOME";

/// Environment variable for the OCI registry domain
pub const OCI_REGISTRY_ENV_VAR: &str = "OCI_REGISTRY_DOMAIN";

/// Environment variable for the mcrun binary path
pub const MCRUN_EXE_ENV_VAR: &str = "MCRUN_EXE";

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Returns the path to the monocore home directory.
/// If the MONOCORE_HOME environment variable is set, returns that path.
/// Otherwise, returns the default monocore home path.
pub fn get_monocore_home_path() -> PathBuf {
    if let Ok(monocore_home) = std::env::var(MONOCORE_HOME_ENV_VAR) {
        PathBuf::from(monocore_home)
    } else {
        DEFAULT_MONOCORE_HOME.to_owned()
    }
}

/// Returns the domain for the OCI registry.
/// If the OCI_REGISTRY_DOMAIN environment variable is set, returns that value.
/// Otherwise, returns the default OCI registry domain.
pub fn get_oci_registry() -> String {
    if let Ok(oci_registry_domain) = std::env::var(OCI_REGISTRY_ENV_VAR) {
        oci_registry_domain
    } else {
        DEFAULT_OCI_REGISTRY.to_string()
    }
}
