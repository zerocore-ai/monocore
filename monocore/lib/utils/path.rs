//! Utility functions for working with paths.

use monoutils::SupportedPathType;

use crate::{MonocoreError, MonocoreResult};

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

/// The directory name for monocore's project-specific data
pub const MONOCORE_ENV_DIR: &str = ".menv";

/// The directory name for monocore's global data
pub const MONOCORE_HOME_DIR: &str = ".monocore";

/// The directory where project read-write layers are stored
///
/// Example: <PROJECT_ROOT>/<MONOCORE_ENV_DIR>/<RW_SUBDIR>
pub const RW_SUBDIR: &str = "rw";

/// The directory where project patch layers are stored
///
/// Example: <PROJECT_ROOT>/<MONOCORE_ENV_DIR>/<PATCH_SUBDIR>
pub const PATCH_SUBDIR: &str = "patch";

/// The directory where base store blocks are stored
///
/// Example: <PROJECT_ROOT>/<MONOCORE_ENV_DIR>/<BLOCKS_SUBDIR>
pub const BLOCKS_SUBDIR: &str = "blocks";

/// The directory where project logs are stored
///
/// Example: <PROJECT_ROOT>/<MONOCORE_ENV_DIR>/<LOG_SUBDIR>
pub const LOG_SUBDIR: &str = "log";

/// The directory where global image layers are stored
///
/// Example: <MONOCORE_HOME_DIR>/<LAYERS_SUBDIR>
pub const LAYERS_SUBDIR: &str = "layers";

/// The directory where monocore's installed binaries are stored
///
/// Example: <MONOCORE_HOME_DIR>/<BIN_SUBDIR>
pub const BIN_SUBDIR: &str = "bin";

/// The filename for the project active sandbox database
///
/// Example: <PROJECT_ROOT>/<MONOCORE_ENV_DIR>/<SANDBOX_DB_FILENAME>
pub const SANDBOX_DB_FILENAME: &str = "sandbox.db";

/// The filename for the global OCI database
///
/// Example: <MONOCORE_HOME_DIR>/<OCI_DB_FILENAME>
pub const OCI_DB_FILENAME: &str = "oci.db";

/// The directory on the microvm where sandbox scripts are stored
pub const SANDBOX_SCRIPT_DIR: &str = ".sandbox_scripts";

/// The suffix added to extracted layer directories
///
/// Example: <MONOCORE_HOME_DIR>/<LAYERS_SUBDIR>/<LAYER_ID>.<EXTRACTED_LAYER_SUFFIX>
pub const EXTRACTED_LAYER_SUFFIX: &str = "extracted";

/// The monocore config file name.
///
/// Example: <PROJECT_ROOT>/<MONOCORE_ENV_DIR>/<SANDBOX_DB_FILENAME>
pub const MONOCORE_CONFIG_FILENAME: &str = "Sandboxfile";

/// The shell script name.
///
/// Example: <PROJECT_ROOT>/<MONOCORE_ENV_DIR>/<PATCH_SUBDIR>/<CONFIG_NAME>/<SHELL_SCRIPT_NAME>
pub const SHELL_SCRIPT_NAME: &str = "shell";

/// The directory for server namespaces
///
/// Example: <MONOCORE_HOME_DIR>/<NAMESPACES_SUBDIR>
pub const NAMESPACES_SUBDIR: &str = "namespaces";

/// The PID file for the server
///
/// Example: <MONOCORE_HOME_DIR>/<SERVER_PID_FILE>
pub const SERVER_PID_FILE: &str = "server.pid";

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

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
