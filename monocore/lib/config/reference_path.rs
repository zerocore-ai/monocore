use std::{
    fmt::{self, Display},
    path::PathBuf,
    str::FromStr,
};

use crate::{oci::Reference, MonocoreError};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Represents either an OCI image reference or a path to a rootfs on disk.
///
/// This type is used to specify the source of a container's root filesystem:
/// - For OCI images (e.g., "docker.io/library/ubuntu:latest"), use `Reference` variant
/// - For local rootfs directories (e.g., "/path/to/rootfs" or "./rootfs"), use `Path` variant
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String")]
#[serde(into = "String")]
pub enum ReferencePath {
    /// An OCI-compliant image reference (e.g., "docker.io/library/ubuntu:latest").
    /// This is used when the rootfs should be pulled from a container registry.
    Reference(Reference),

    /// A path to a rootfs directory on the local filesystem.
    /// This can be either absolute (e.g., "/path/to/rootfs") or relative (e.g., "./rootfs").
    Path(PathBuf),
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl FromStr for ReferencePath {
    type Err = MonocoreError;

    /// Parses a string into a ReferencePath.
    ///
    /// The parsing rules are:
    /// - If the string starts with "." or "/", it is interpreted as a path to a local rootfs
    /// - Otherwise, it is interpreted as an OCI image reference
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::str::FromStr;
    /// # use monocore::config::ReferencePath;
    /// // Parse as local rootfs path
    /// let local = ReferencePath::from_str("./my-rootfs").unwrap();
    /// let absolute = ReferencePath::from_str("/var/lib/my-rootfs").unwrap();
    ///
    /// // Parse as OCI image reference
    /// let image = ReferencePath::from_str("ubuntu:latest").unwrap();
    /// let full = ReferencePath::from_str("docker.io/library/debian:11").unwrap();
    /// ```
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Check if the string starts with "." or "/" to determine if it's a path
        if s.starts_with('.') || s.starts_with('/') {
            Ok(ReferencePath::Path(PathBuf::from(s)))
        } else {
            // Parse as an image reference
            let reference = Reference::from_str(s)?;
            Ok(ReferencePath::Reference(reference))
        }
    }
}

impl Display for ReferencePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReferencePath::Path(path) => write!(f, "{}", path.display()),
            ReferencePath::Reference(reference) => write!(f, "{}", reference),
        }
    }
}

impl TryFrom<String> for ReferencePath {
    type Error = MonocoreError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl Into<String> for ReferencePath {
    fn into(self) -> String {
        self.to_string()
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_relative() {
        // Test relative paths with different formats
        let cases = vec![
            "./path/to/file",
            "./single",
            ".",
            "./path/with/multiple/segments",
            "./path.with.dots",
            "./path-with-dashes",
            "./path_with_underscores",
        ];

        for case in cases {
            let reference = ReferencePath::from_str(case).unwrap();
            match &reference {
                ReferencePath::Path(path) => {
                    assert_eq!(path, &PathBuf::from(case));
                    assert_eq!(reference.to_string(), case);
                }
                _ => panic!("Expected Path variant for {}", case),
            }
        }
    }

    #[test]
    fn test_path_absolute() {
        // Test absolute paths with different formats
        let cases = vec![
            "/absolute/path",
            "/root",
            "/path/with/multiple/segments",
            "/path.with.dots",
            "/path-with-dashes",
            "/path_with_underscores",
        ];

        for case in cases {
            let reference = ReferencePath::from_str(case).unwrap();
            match &reference {
                ReferencePath::Path(path) => {
                    assert_eq!(path, &PathBuf::from(case));
                    assert_eq!(reference.to_string(), case);
                }
                _ => panic!("Expected Path variant for {}", case),
            }
        }
    }

    #[test]
    fn test_image_reference_simple() {
        // Test simple image references
        let cases = vec![
            "alpine:latest",
            "ubuntu:20.04",
            "nginx:1.19",
            "redis:6",
            "postgres:13-alpine",
        ];

        for case in cases {
            let reference = ReferencePath::from_str(case).unwrap();
            match &reference {
                ReferencePath::Reference(ref_) => {
                    assert_eq!(reference.to_string(), ref_.to_string());
                }
                _ => panic!("Expected Reference variant for {}", case),
            }
        }
    }

    #[test]
    fn test_image_reference_with_registry() {
        // Test image references with registry
        let cases = vec![
            "docker.io/library/alpine:latest",
            "registry.example.com/myapp:v1.0",
            "ghcr.io/owner/repo:tag",
            "k8s.gcr.io/pause:3.2",
            "quay.io/organization/image:1.0",
        ];

        for case in cases {
            let reference = ReferencePath::from_str(case).unwrap();
            match &reference {
                ReferencePath::Reference(ref_) => {
                    assert_eq!(reference.to_string(), ref_.to_string());
                }
                _ => panic!("Expected Reference variant for {}", case),
            }
        }
    }

    #[test]
    fn test_image_reference_with_digest() {
        let valid_digest = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";
        let cases = vec![
            format!("alpine@sha256:{}", valid_digest),
            format!("docker.io/library/ubuntu@sha256:{}", valid_digest),
            format!("registry.example.com/myapp:v1.0@sha256:{}", valid_digest),
        ];

        for case in cases {
            let reference = ReferencePath::from_str(&case).unwrap();
            match &reference {
                ReferencePath::Reference(ref_) => {
                    assert_eq!(reference.to_string(), ref_.to_string());
                }
                _ => panic!("Expected Reference variant for {}", case),
            }
        }
    }

    #[test]
    fn test_image_reference_with_port() {
        // Test image references with registry port
        let cases = vec![
            "localhost:5000/myapp:latest",
            "registry.example.com:5000/app:v1",
            "192.168.1.1:5000/image:tag",
        ];

        for case in cases {
            let reference = ReferencePath::from_str(case).unwrap();
            match &reference {
                ReferencePath::Reference(ref_) => {
                    assert_eq!(reference.to_string(), ref_.to_string());
                }
                _ => panic!("Expected Reference variant for {}", case),
            }
        }
    }

    #[test]
    fn test_empty_input() {
        // Test empty input
        assert!(ReferencePath::from_str("").is_err());
    }

    #[test]
    fn test_display_formatting() {
        // Test display formatting for both variants
        let test_cases = vec![
            ("./local/path", "./local/path"),
            ("/absolute/path", "/absolute/path"),
            ("alpine:latest", "sandboxes.io/library/alpine:latest"),
            (
                "registry.example.com/app:v1.0",
                "registry.example.com/library/app:v1.0",
            ),
        ];

        for (input, expected) in test_cases {
            let reference = ReferencePath::from_str(input).unwrap();
            assert_eq!(reference.to_string(), expected);
        }
    }

    #[test]
    fn test_serde_path_roundtrip() {
        let test_cases = vec![
            ReferencePath::Path(PathBuf::from("./local/rootfs")),
            ReferencePath::Path(PathBuf::from("/absolute/path/to/rootfs")),
            ReferencePath::Path(PathBuf::from(".")),
            ReferencePath::Path(PathBuf::from("/root")),
        ];

        for case in test_cases {
            let serialized = serde_yaml::to_string(&case).unwrap();
            let deserialized: ReferencePath = serde_yaml::from_str(&serialized).unwrap();
            assert_eq!(case, deserialized);
        }
    }

    #[test]
    fn test_serde_reference_roundtrip() {
        let test_cases = vec![
            "alpine:latest",
            "docker.io/library/ubuntu:20.04",
            "registry.example.com:5000/myapp:v1.0",
            "ghcr.io/owner/repo:tag",
        ];

        for case in test_cases {
            let reference = ReferencePath::from_str(case).unwrap();
            let serialized = serde_yaml::to_string(&reference).unwrap();
            let deserialized: ReferencePath = serde_yaml::from_str(&serialized).unwrap();
            assert_eq!(reference, deserialized);
        }
    }

    #[test]
    fn test_serde_yaml_format() {
        // Test Path variant serialization format
        let path = ReferencePath::Path(PathBuf::from("/test/rootfs"));
        let serialized = serde_yaml::to_string(&path).unwrap();
        assert_eq!(serialized.trim(), "/test/rootfs");

        // Test Reference variant serialization format
        let reference = ReferencePath::from_str("ubuntu:latest").unwrap();
        let serialized = serde_yaml::to_string(&reference).unwrap();
        assert!(serialized.trim().contains("ubuntu:latest"));
    }

    #[test]
    fn test_serde_invalid_input() {
        // Test deserializing invalid YAML
        let invalid_yaml = "- not a valid reference path";
        assert!(serde_yaml::from_str::<ReferencePath>(invalid_yaml).is_err());

        // Test deserializing invalid reference format
        let invalid_reference = "invalid!reference:format";
        assert!(serde_yaml::from_str::<ReferencePath>(invalid_reference).is_err());
    }
}
