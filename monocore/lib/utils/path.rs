use std::path::PathBuf;

use monoutils::SupportedPathType;

use crate::{
    config::{DEFAULT_MONOCORE_ENV, DEFAULT_MONOCORE_HOME},
    MonocoreError, MonocoreResult,
};

use super::{MONOCORE_ENV_VAR, MONOCORE_HOME_ENV_VAR};

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

/// The directory name for monocore's project-specific data
pub const MONOCORE_ENV_DIR: &str = ".menv";

/// The directory name for monocore's global data
pub const MONOCORE_HOME_DIR: &str = ".monocore";

/// The directory where project filesystems are stored
pub const FILESYSTEMS_SUBDIR: &str = "filesystems";

/// The directory where project logs are stored
pub const LOG_SUBDIR: &str = "log";

/// The directory where global image layers are stored
pub const LAYERS_SUBDIR: &str = "layers";

/// The directory where monocore's installed binaries are stored
pub const BIN_SUBDIR: &str = "bin";

/// The filename for the project active database
pub const ACTIVE_DB_FILENAME: &str = "active.db";

/// The filename for the global OCI database
pub const OCI_DB_FILENAME: &str = "oci.db";

/// The filename for the supervisor's log file
pub const SUPERVISOR_LOG_FILENAME: &str = "supervisor.log";

/// The suffix for sandbox log files
pub const LOG_SUFFIX: &str = ".log";

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Returns the path where all global monocore data is stored.
pub fn monocore_home_path() -> PathBuf {
    if let Ok(monocore_home) = std::env::var(MONOCORE_HOME_ENV_VAR) {
        PathBuf::from(monocore_home)
    } else {
        DEFAULT_MONOCORE_HOME.to_owned()
    }
}

/// Returns the path where all monocore project data is stored.
pub fn monocore_env_path() -> PathBuf {
    if let Ok(monocore_env) = std::env::var(MONOCORE_ENV_VAR) {
        PathBuf::from(monocore_env)
    } else {
        DEFAULT_MONOCORE_ENV.to_owned()
    }
}

/// Checks if two paths conflict (one is a parent/child of the other or they are the same)
pub fn paths_overlap(path1: &str, path2: &str) -> bool {
    let path1 = if path1.ends_with('/') {
        path1.to_string()
    } else {
        format!("{}/", path1)
    };

    let path2 = if path2.ends_with('/') {
        path2.to_string()
    } else {
        format!("{}/", path2)
    };

    path1.starts_with(&path2) || path2.starts_with(&path1)
}

/// Helper function to normalize and validate volume paths
pub fn normalize_volume_path(base_path: &str, requested_path: &str) -> MonocoreResult<String> {
    // First normalize both paths
    let normalized_base = monoutils::normalize_path(base_path, SupportedPathType::Absolute)?;

    // If requested path is absolute, verify it's under base_path
    if requested_path.starts_with('/') {
        let normalized_requested =
            monoutils::normalize_path(requested_path, SupportedPathType::Absolute)?;
        // Check if normalized_requested starts with normalized_base
        if !normalized_requested.starts_with(&normalized_base) {
            return Err(MonocoreError::PathValidation(format!(
                "Absolute path '{}' must be under base path '{}'",
                normalized_requested, normalized_base
            )));
        }
        Ok(normalized_requested)
    } else {
        // For relative paths, first normalize the requested path to catch any ../ attempts
        let normalized_requested =
            monoutils::normalize_path(requested_path, SupportedPathType::Relative)?;

        // Then join with base and normalize again
        let full_path = format!("{}/{}", normalized_base, normalized_requested);
        monoutils::normalize_path(&full_path, SupportedPathType::Absolute).map_err(Into::into)
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paths_overlap() {
        // Test cases that should conflict
        assert!(paths_overlap("/data", "/data"));
        assert!(paths_overlap("/data", "/data/app"));
        assert!(paths_overlap("/data/app", "/data"));
        assert!(paths_overlap("/data/app/logs", "/data/app"));

        // Test cases that should not conflict
        assert!(!paths_overlap("/data", "/database"));
        assert!(!paths_overlap("/var/log", "/var/lib"));
        assert!(!paths_overlap("/data/app1", "/data/app2"));
        assert!(!paths_overlap("/data/app/logs", "/data/web/logs"));
    }
}
