use std::path::PathBuf;

use typed_path::{Utf8UnixComponent, Utf8UnixPathBuf};

use crate::{config::{DEFAULT_MONOCORE_ENV, DEFAULT_MONOCORE_HOME}, MonocoreError, MonocoreResult};

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

/// Normalizes a path string for volume mount comparison.
///
/// Rules:
/// - Resolves . and .. components where possible
/// - Prevents path traversal that would escape the root
/// - Removes redundant separators and trailing slashes
/// - Case-sensitive comparison (Unix standard)
/// - Can require absolute paths (for host mounts)
///
/// # Arguments
/// * `path` - The path to normalize
/// * `require_absolute` - If true, requires path to be absolute (start with '/')
///
/// # Returns
/// An error if the path is invalid, would escape root, or doesn't meet absolute requirement
pub fn normalize_path(path: &str, require_absolute: bool) -> MonocoreResult<String> {
    if path.is_empty() {
        return Err(MonocoreError::PathValidation(
            "Path cannot be empty".to_string(),
        ));
    }

    let path = Utf8UnixPathBuf::from(path);
    let mut normalized = Vec::new();
    let mut is_absolute = false;
    let mut depth = 0;

    for component in path.components() {
        match component {
            // Root component must come first if present
            Utf8UnixComponent::RootDir => {
                if normalized.is_empty() {
                    is_absolute = true;
                    normalized.push("/".to_string());
                } else {
                    return Err(MonocoreError::PathValidation(
                        "Invalid path: root component '/' found in middle of path".to_string(),
                    ));
                }
            }
            // Handle parent directory references
            Utf8UnixComponent::ParentDir => {
                if depth > 0 {
                    // Can go up if we have depth
                    normalized.pop();
                    depth -= 1;
                } else {
                    // Trying to go above root
                    return Err(MonocoreError::PathValidation(
                        "Invalid path: cannot traverse above root directory".to_string(),
                    ));
                }
            }
            // Skip current dir components
            Utf8UnixComponent::CurDir => continue,
            // Normal components are fine
            Utf8UnixComponent::Normal(c) => {
                if !c.is_empty() {
                    normalized.push(c.to_string());
                    depth += 1;
                }
            }
        }
    }

    // Check absolute path requirement if enabled
    if require_absolute && !is_absolute {
        return Err(MonocoreError::PathValidation(
            "Host mount paths must be absolute (start with '/')".to_string(),
        ));
    }

    if is_absolute {
        if normalized.len() == 1 {
            // Just root
            Ok("/".to_string())
        } else {
            // Join all components with "/" and add root at start
            Ok(format!("/{}", normalized[1..].join("/")))
        }
    } else {
        // For relative paths, just join all components
        Ok(normalized.join("/"))
    }
}

/// Helper function to normalize and validate volume paths
pub fn normalize_volume_path(base_path: &str, requested_path: &str) -> MonocoreResult<String> {
    // First normalize both paths
    let normalized_base = normalize_path(base_path, true)?;

    // If requested path is absolute, verify it's under base_path
    if requested_path.starts_with('/') {
        let normalized_requested = normalize_path(requested_path, true)?;
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
        let normalized_requested = normalize_path(requested_path, false)?;

        // Then join with base and normalize again
        let full_path = format!("{}/{}", normalized_base, normalized_requested);
        normalize_path(&full_path, true)
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

    #[test]
    fn test_normalize_path() {
        // Test with require_absolute = true
        assert_eq!(normalize_path("/data/app/", true).unwrap(), "/data/app");
        assert_eq!(normalize_path("/data//app", true).unwrap(), "/data/app");
        assert_eq!(normalize_path("/data/./app", true).unwrap(), "/data/app");

        // Test with require_absolute = false
        assert_eq!(normalize_path("data/app/", false).unwrap(), "data/app");
        assert_eq!(normalize_path("./data/app", false).unwrap(), "data/app");
        assert_eq!(normalize_path("data//app", false).unwrap(), "data/app");

        // Path traversal within bounds
        assert_eq!(
            normalize_path("/data/temp/../app", true).unwrap(),
            "/data/app"
        );
        assert_eq!(
            normalize_path("data/temp/../app", false).unwrap(),
            "data/app"
        );

        // Invalid paths
        assert!(matches!(
            normalize_path("data/app", true),
            Err(MonocoreError::PathValidation(e)) if e.contains("must be absolute")
        ));
        assert!(matches!(
            normalize_path("/data/../..", true),
            Err(MonocoreError::PathValidation(e)) if e.contains("cannot traverse above root")
        ));
        assert!(matches!(
            normalize_path("data/../..", false),
            Err(MonocoreError::PathValidation(e)) if e.contains("cannot traverse above root")
        ));
    }

    #[test]
    fn test_normalize_path_complex() {
        // Complex but valid paths
        assert_eq!(
            normalize_path("/data/./temp/../logs/app/./config/../", true).unwrap(),
            "/data/logs/app"
        );
        assert_eq!(
            normalize_path("/data///temp/././../app//./test/..", true).unwrap(),
            "/data/app"
        );

        // Edge cases
        assert_eq!(normalize_path("/data/./././.", true).unwrap(), "/data");
        assert_eq!(
            normalize_path("/data/test/../../data/app", true).unwrap(),
            "/data/app"
        );

        // Invalid complex paths
        assert!(matches!(
            normalize_path("/data/test/../../../root", true),
            Err(MonocoreError::PathValidation(e)) if e.contains("cannot traverse above root")
        ));
        assert!(matches!(
            normalize_path("/./data/../..", true),
            Err(MonocoreError::PathValidation(e)) if e.contains("cannot traverse above root")
        ));
    }
}
