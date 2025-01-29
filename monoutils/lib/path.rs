//! `monoutils::path` is a module containing path utilities for the monocore project.

use typed_path::{Utf8UnixComponent, Utf8UnixPathBuf};

use crate::{MonoutilsError, MonoutilsResult};

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

/// The suffix for log files
pub const LOG_SUFFIX: &str = "log";

/// The filename for the supervisor's log file
pub const SUPERVISOR_LOG_FILENAME: &str = "supervisor.log";

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// The type of a supported path.
pub enum SupportedPathType {
    /// Any path type.
    Any,

    /// An absolute path.
    Absolute,

    /// A relative path.
    Relative,
}

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Normalizes a path string for volume mount comparison.
///
/// Rules:
/// - Resolves . and .. components where possible
/// - Prevents path traversal that would escape the root
/// - Removes redundant separators and trailing slashes
/// - Case-sensitive comparison (Unix standard)
/// - Can enforce path type requirements (absolute, relative, or any)
///
/// # Arguments
/// * `path` - The path to normalize
/// * `path_type` - The required path type (absolute, relative, or any)
///
/// # Returns
/// An error if the path is invalid, would escape root, or doesn't meet path type requirement
pub fn normalize_path(path: &str, path_type: SupportedPathType) -> MonoutilsResult<String> {
    if path.is_empty() {
        return Err(MonoutilsError::PathValidation(
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
                    return Err(MonoutilsError::PathValidation(
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
                    return Err(MonoutilsError::PathValidation(
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

    // Check path type requirements
    match path_type {
        SupportedPathType::Absolute if !is_absolute => {
            return Err(MonoutilsError::PathValidation(
                "Path must be absolute (start with '/')".to_string(),
            ));
        }
        SupportedPathType::Relative if is_absolute => {
            return Err(MonoutilsError::PathValidation(
                "Path must be relative (must not start with '/')".to_string(),
            ));
        }
        _ => {}
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

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_path() {
        // Test with SupportedPathType::Absolute
        assert_eq!(
            normalize_path("/data/app/", SupportedPathType::Absolute).unwrap(),
            "/data/app"
        );
        assert_eq!(
            normalize_path("/data//app", SupportedPathType::Absolute).unwrap(),
            "/data/app"
        );
        assert_eq!(
            normalize_path("/data/./app", SupportedPathType::Absolute).unwrap(),
            "/data/app"
        );

        // Test with SupportedPathType::Relative
        assert_eq!(
            normalize_path("data/app/", SupportedPathType::Relative).unwrap(),
            "data/app"
        );
        assert_eq!(
            normalize_path("./data/app", SupportedPathType::Relative).unwrap(),
            "data/app"
        );
        assert_eq!(
            normalize_path("data//app", SupportedPathType::Relative).unwrap(),
            "data/app"
        );

        // Test with SupportedPathType::Any
        assert_eq!(
            normalize_path("/data/app", SupportedPathType::Any).unwrap(),
            "/data/app"
        );
        assert_eq!(
            normalize_path("data/app", SupportedPathType::Any).unwrap(),
            "data/app"
        );

        // Path traversal within bounds
        assert_eq!(
            normalize_path("/data/temp/../app", SupportedPathType::Absolute).unwrap(),
            "/data/app"
        );
        assert_eq!(
            normalize_path("data/temp/../app", SupportedPathType::Relative).unwrap(),
            "data/app"
        );

        // Invalid paths
        assert!(matches!(
            normalize_path("data/app", SupportedPathType::Absolute),
            Err(MonoutilsError::PathValidation(e)) if e.contains("must be absolute")
        ));
        assert!(matches!(
            normalize_path("/data/app", SupportedPathType::Relative),
            Err(MonoutilsError::PathValidation(e)) if e.contains("must be relative")
        ));
        assert!(matches!(
            normalize_path("/data/../..", SupportedPathType::Any),
            Err(MonoutilsError::PathValidation(e)) if e.contains("cannot traverse above root")
        ));
    }

    #[test]
    fn test_normalize_path_complex() {
        // Complex but valid paths
        assert_eq!(
            normalize_path(
                "/data/./temp/../logs/app/./config/../",
                SupportedPathType::Absolute
            )
            .unwrap(),
            "/data/logs/app"
        );
        assert_eq!(
            normalize_path(
                "/data///temp/././../app//./test/..",
                SupportedPathType::Absolute
            )
            .unwrap(),
            "/data/app"
        );

        // Edge cases
        assert_eq!(
            normalize_path("/data/./././.", SupportedPathType::Absolute).unwrap(),
            "/data"
        );
        assert_eq!(
            normalize_path("/data/test/../../data/app", SupportedPathType::Absolute).unwrap(),
            "/data/app"
        );

        // Invalid complex paths
        assert!(matches!(
            normalize_path("/data/test/../../../root", SupportedPathType::Any),
            Err(MonoutilsError::PathValidation(e)) if e.contains("cannot traverse above root")
        ));
        assert!(matches!(
            normalize_path("/./data/../..", SupportedPathType::Any),
            Err(MonoutilsError::PathValidation(e)) if e.contains("cannot traverse above root")
        ));
    }
}
