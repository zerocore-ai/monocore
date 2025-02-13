use crate::{
    config::{DEFAULT_OCI_REFERENCE_REPO_NAMESPACE, DEFAULT_OCI_REFERENCE_TAG},
    error::MonocoreError,
    utils::env::get_oci_registry,
};
use getset::{Getters, Setters};
use oci_spec::image::Digest;
use regex::Regex;
use std::{fmt, str::FromStr};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Represents an OCI-compliant image reference.
///
/// This struct includes the registry, repository, and a selector that combines a tag and an optional digest.
/// If no registry or tag is provided in the input string, default values will be used.
#[derive(Debug, Clone, PartialEq, Eq, Getters, Setters)]
#[getset(get = "pub with_prefix", set = "pub with_prefix")]
pub struct Reference {
    /// The registry where the image is hosted.
    registry: String,

    /// The repository name of the image.
    repository: String,

    /// The selector specifying either a tag and an optional digest, or a digest only.
    selector: ReferenceSelector,
}

/// Represents the selector part of an OCI image reference.
///
/// It can either be a tag (with an optional digest) or a standalone digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferenceSelector {
    /// Tag variant containing the image tag and an optional digest.
    Tag {
        /// The image tag.
        tag: String,

        /// The optional digest.
        digest: Option<Digest>,
    },
    /// Digest variant containing only a digest.
    Digest(Digest),
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl ReferenceSelector {
    /// Creates a new ReferenceSelector with the specified tag and no digest.
    pub fn tag(tag: impl Into<String>) -> Self {
        Self::Tag {
            tag: tag.into(),
            digest: None,
        }
    }

    /// Creates a new ReferenceSelector with both a tag and an associated digest.
    pub fn tag_with_digest(tag: impl Into<String>, digest: impl Into<Digest>) -> Self {
        Self::Tag {
            tag: tag.into(),
            digest: Some(digest.into()),
        }
    }

    /// Creates a new ReferenceSelector using the specified digest.
    pub fn digest(digest: impl Into<Digest>) -> Self {
        Self::Digest(digest.into())
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl FromStr for Reference {
    type Err = MonocoreError;

    /// Parses a string slice into an OCI image Reference.
    ///
    /// Supported formats include:
    /// - "registry/repository:tag"
    /// - "repository:tag"
    /// - "repository"
    /// - "registry/repository@digest"
    /// - "registry/repository:tag@digest"
    ///
    /// If the registry is omitted, it defaults to the value from [`get_oci_registry`].
    /// If the tag is omitted, it defaults to [`DEFAULT_OCI_REFERENCE_TAG`].
    ///
    /// ## Returns
    ///
    /// Returns a [`MonocoreError::ImageReferenceError`] for parse failures.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        let default_registry = get_oci_registry();

        if s.is_empty() {
            return Err(MonocoreError::ImageReferenceError(
                "input string is empty".into(),
            ));
        }

        if let Some(at_idx) = s.find('@') {
            let potential_digest = &s[at_idx + 1..];
            if potential_digest.contains(":") {
                // Treat as digest branch
                let (pre, digest_part) = s.split_at(at_idx);
                let digest_str = &digest_part[1..]; // Skip '@'
                let parsed_digest = digest_str.parse::<Digest>().map_err(|e| {
                    MonocoreError::ImageReferenceError(format!("invalid digest: {}", e))
                })?;

                let (registry, remainder) = extract_registry_and_path(pre, &default_registry);
                let (repository, tag) = extract_repository_and_tag(remainder)?;

                // Validate registry, repository and tag
                validate_registry(&registry)?;
                validate_repository(&repository)?;
                validate_tag(&tag)?;

                Ok(Reference {
                    registry,
                    repository,
                    selector: ReferenceSelector::tag_with_digest(tag, parsed_digest),
                })
            } else {
                return Err(MonocoreError::ImageReferenceError(format!(
                    "invalid digest: {}",
                    potential_digest
                )));
            }
        } else {
            let (registry, remainder) = extract_registry_and_path(s, &default_registry);
            let (repository, tag) = extract_repository_and_tag(remainder)?;

            // Validate registry, repository and tag
            validate_registry(&registry)?;
            validate_repository(&repository)?;
            validate_tag(&tag)?;

            Ok(Reference {
                registry,
                repository,
                selector: ReferenceSelector::tag(tag),
            })
        }
    }
}

impl fmt::Display for Reference {
    /// Formats the OCI image Reference into a string.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.registry, self.repository)?;
        match &self.selector {
            ReferenceSelector::Tag {
                tag,
                digest: Some(d),
            } => write!(f, ":{}@{}", tag, d),
            ReferenceSelector::Tag { tag, digest: None } => write!(f, ":{}", tag),
            ReferenceSelector::Digest(d) => write!(f, "@{}", d),
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Validates the given registry string.
///
/// This function checks that the registry contains only alphanumeric characters, dashes, dots,
/// and optionally a port number. It returns Ok(()) if the registry is valid, or an ImageReferenceError otherwise.
fn validate_registry(registry: &str) -> Result<(), MonocoreError> {
    let re = Regex::new(r"^[a-zA-Z0-9.-]+(:[0-9]+)?$").unwrap();
    if re.is_match(registry) {
        Ok(())
    } else {
        Err(MonocoreError::ImageReferenceError(format!(
            "invalid registry: {}",
            registry
        )))
    }
}

/// Validates the repository name.
///
/// The repository name must match a specific pattern that allows lowercase letters, numbers,
/// and certain punctuation (._-) as well as slashes. Returns Ok if valid, and an error if invalid.
fn validate_repository(repository: &str) -> Result<(), MonocoreError> {
    let repo_re =
        Regex::new(r"^([a-z0-9]+(?:[._-][a-z0-9]+)*)(/[a-z0-9]+(?:[._-][a-z0-9]+)*)*$").unwrap();
    if repo_re.is_match(repository) {
        Ok(())
    } else {
        Err(MonocoreError::ImageReferenceError(format!(
            "invalid repository: {}",
            repository
        )))
    }
}

/// Validates the tag string.
///
/// Ensures that the tag starts with a word character and is followed by up to 127 characters
/// that can be alphanumeric, underscores, dashes, or dots. Returns Ok if the tag is valid, or an error otherwise.
fn validate_tag(tag: &str) -> Result<(), MonocoreError> {
    let tag_re = Regex::new(r"^\w[\w.-]{0,127}$").unwrap();
    if tag_re.is_match(tag) {
        Ok(())
    } else {
        Err(MonocoreError::ImageReferenceError(format!(
            "invalid tag: {}",
            tag
        )))
    }
}

/// Extracts the registry and the remaining path from the OCI reference string.
/// If the registry is not specified, returns the provided default registry.
fn extract_registry_and_path<'a>(reference: &'a str, default_registry: &str) -> (String, &'a str) {
    let segments: Vec<&str> = reference.splitn(2, '/').collect();
    if segments.len() > 1
        && (segments[0].contains('.') || segments[0].contains(':') || segments[0] == "localhost")
    {
        (segments[0].to_string(), segments[1])
    } else {
        (default_registry.to_string(), reference)
    }
}

/// Extracts the repository and tag from the given path string.
/// If the repository part does not contain a '/', the default namespace is prepended.
/// If no tag is provided, the default tag is used.
fn extract_repository_and_tag(path: &str) -> Result<(String, String), MonocoreError> {
    if let Some(idx) = path.rfind(':') {
        let repo_part = &path[..idx];
        let tag_part = &path[idx + 1..];
        if repo_part.is_empty() {
            return Err(MonocoreError::ImageReferenceError(
                "repository is empty".into(),
            ));
        }
        let repository = if !repo_part.contains('/') {
            format!("{}/{}", DEFAULT_OCI_REFERENCE_REPO_NAMESPACE, repo_part)
        } else {
            repo_part.to_string()
        };
        Ok((repository, tag_part.to_string()))
    } else {
        let repository = if !path.contains('/') {
            format!("{}/{}", DEFAULT_OCI_REFERENCE_REPO_NAMESPACE, path)
        } else {
            path.to_string()
        };
        Ok((repository, DEFAULT_OCI_REFERENCE_TAG.to_string()))
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{DEFAULT_OCI_REFERENCE_REPO_NAMESPACE, DEFAULT_OCI_REFERENCE_TAG},
        utils::env::get_oci_registry,
    };

    #[test]
    fn test_reference_valid_reference_with_registry_and_tag() {
        let s = "docker.io/library/alpine:3.12";
        let reference = s.parse::<Reference>().unwrap();
        assert_eq!(reference.registry, "docker.io");
        assert_eq!(reference.repository, "library/alpine");
        match reference.selector {
            ReferenceSelector::Tag {
                ref tag,
                ref digest,
            } => {
                assert_eq!(tag, "3.12");
                assert!(digest.is_none());
            }
            _ => panic!("Expected Tag variant without digest"),
        }
        assert_eq!(reference.to_string(), "docker.io/library/alpine:3.12");
    }

    #[test]
    fn test_reference_default_registry_and_tag() {
        let s = "library/alpine";
        let reference = s.parse::<Reference>().unwrap();
        let expected_registry = get_oci_registry();
        assert_eq!(reference.registry, expected_registry);
        assert_eq!(reference.repository, "library/alpine");
        match reference.selector {
            ReferenceSelector::Tag {
                ref tag,
                ref digest,
            } => {
                assert_eq!(tag, DEFAULT_OCI_REFERENCE_TAG);
                assert!(digest.is_none());
            }
            _ => panic!("Expected Tag variant without digest"),
        }
        let expected_string = format!(
            "{}/library/alpine:{}",
            expected_registry, DEFAULT_OCI_REFERENCE_TAG
        );
        assert_eq!(reference.to_string(), expected_string);
    }

    #[test]
    fn test_reference_without_tag() {
        let s = "docker.io/library/alpine";
        let reference = s.parse::<Reference>().unwrap();
        assert_eq!(reference.registry, "docker.io");
        assert_eq!(reference.repository, "library/alpine");
        match reference.selector {
            ReferenceSelector::Tag {
                ref tag,
                ref digest,
            } => {
                assert_eq!(tag, DEFAULT_OCI_REFERENCE_TAG);
                assert!(digest.is_none());
            }
            _ => panic!("Expected Tag variant without digest"),
        }
        let expected = format!("docker.io/library/alpine:{}", DEFAULT_OCI_REFERENCE_TAG);
        assert_eq!(reference.to_string(), expected);
    }

    #[test]
    fn test_reference_with_digest_and_tag() {
        let valid_digest = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";
        let s = format!("registry.example.com/myrepo:mytag@sha256:{}", valid_digest);
        let reference = s.parse::<Reference>().unwrap();
        assert_eq!(reference.registry, "registry.example.com");
        assert_eq!(reference.repository, "library/myrepo");
        match reference.selector {
            ReferenceSelector::Tag {
                ref tag,
                ref digest,
            } => {
                assert_eq!(tag, "mytag");
                let d = digest.as_ref().expect("Expected a digest");
                assert_eq!(d.to_string(), format!("sha256:{}", valid_digest));
            }
            _ => panic!("Expected Tag variant with digest"),
        }
        let expected = format!(
            "registry.example.com/library/myrepo:mytag@sha256:{}",
            valid_digest
        );
        assert_eq!(reference.to_string(), expected);
    }

    #[test]
    fn test_reference_with_digest_only() {
        let valid_digest = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";
        let s = format!("registry.example.com/myrepo@sha256:{}", valid_digest);
        let reference = s.parse::<Reference>().unwrap();
        assert_eq!(reference.registry, "registry.example.com");
        assert_eq!(reference.repository, "library/myrepo");
        match reference.selector {
            ReferenceSelector::Tag {
                ref tag,
                ref digest,
            } => {
                assert_eq!(tag, DEFAULT_OCI_REFERENCE_TAG);
                let d = digest.as_ref().expect("Expected a digest");
                assert_eq!(d.to_string(), format!("sha256:{}", valid_digest));
            }
            _ => panic!("Expected Tag variant with digest"),
        }
        let expected = format!(
            "registry.example.com/library/myrepo:{}@sha256:{}",
            DEFAULT_OCI_REFERENCE_TAG, valid_digest
        );
        assert_eq!(reference.to_string(), expected);
    }

    #[test]
    fn test_reference_registry_with_port() {
        let s = "registry.example.com:5000/myrepo:1.0";
        let reference = s.parse::<Reference>().unwrap();
        assert_eq!(reference.registry, "registry.example.com:5000");
        assert_eq!(reference.repository, "library/myrepo");
        match reference.selector {
            ReferenceSelector::Tag {
                ref tag,
                ref digest,
            } => {
                assert_eq!(tag, "1.0");
                assert!(digest.is_none());
            }
            _ => panic!("Expected Tag variant without digest"),
        }
        assert_eq!(
            reference.to_string(),
            "registry.example.com:5000/library/myrepo:1.0"
        );
    }

    #[test]
    fn test_reference_single_segment_registry() {
        let s = "docker.io/alpine";
        let reference = s.parse::<Reference>().unwrap();
        assert_eq!(reference.registry, "docker.io");
        assert_eq!(
            reference.repository,
            format!("{}/alpine", DEFAULT_OCI_REFERENCE_REPO_NAMESPACE)
        );
        match reference.selector {
            ReferenceSelector::Tag {
                ref tag,
                ref digest,
            } => {
                assert_eq!(tag, DEFAULT_OCI_REFERENCE_TAG);
                assert!(digest.is_none());
            }
            _ => panic!("Expected Tag variant"),
        }
        assert_eq!(
            reference.to_string(),
            format!(
                "docker.io/{}/alpine:{}",
                DEFAULT_OCI_REFERENCE_REPO_NAMESPACE, DEFAULT_OCI_REFERENCE_TAG
            )
        );
    }

    #[test]
    fn test_reference_no_registry_single_segment() {
        let s = "alpine";
        let reference = s.parse::<Reference>().unwrap();
        let default_registry = get_oci_registry();
        assert_eq!(reference.registry, default_registry);
        assert_eq!(
            reference.repository,
            format!("{}/alpine", DEFAULT_OCI_REFERENCE_REPO_NAMESPACE)
        );
        match reference.selector {
            ReferenceSelector::Tag {
                ref tag,
                ref digest,
            } => {
                assert_eq!(tag, DEFAULT_OCI_REFERENCE_TAG);
                assert!(digest.is_none());
            }
            _ => panic!("Expected Tag variant"),
        }
        let expected = format!(
            "{}/{}:{}",
            default_registry,
            format!("{}/alpine", DEFAULT_OCI_REFERENCE_REPO_NAMESPACE),
            DEFAULT_OCI_REFERENCE_TAG
        );
        assert_eq!(reference.to_string(), expected);
    }

    #[test]
    fn test_reference_no_registry_multi_segment() {
        let s = "myorg/myrepo:stable";
        let reference = s.parse::<Reference>().unwrap();
        let default_registry = get_oci_registry();
        assert_eq!(reference.registry, default_registry);
        assert_eq!(reference.repository, "myorg/myrepo");
        match reference.selector {
            ReferenceSelector::Tag {
                ref tag,
                ref digest,
            } => {
                assert_eq!(tag, "stable");
                assert!(digest.is_none());
            }
            _ => panic!("Expected Tag variant"),
        }
        let expected = format!("{}/myorg/myrepo:stable", default_registry);
        assert_eq!(reference.to_string(), expected);
    }

    #[test]
    fn test_reference_digest_single_segment() {
        let valid_digest = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";
        let s = format!("docker.io/alpine@sha256:{}", valid_digest);
        let reference = s.parse::<Reference>().unwrap();
        assert_eq!(reference.registry, "docker.io");
        assert_eq!(
            reference.repository,
            format!("{}/alpine", DEFAULT_OCI_REFERENCE_REPO_NAMESPACE)
        );
        match reference.selector {
            ReferenceSelector::Tag {
                ref tag,
                ref digest,
            } => {
                assert_eq!(tag, DEFAULT_OCI_REFERENCE_TAG);
                let d = digest.as_ref().expect("Expected digest");
                assert_eq!(d.to_string(), format!("sha256:{}", valid_digest));
            }
            _ => panic!("Expected Tag variant with digest"),
        }
        let expected = format!(
            "docker.io/{}/alpine:{}@sha256:{}",
            DEFAULT_OCI_REFERENCE_REPO_NAMESPACE, DEFAULT_OCI_REFERENCE_TAG, valid_digest
        );
        assert_eq!(reference.to_string(), expected);
    }

    #[test]
    fn test_reference_digest_multi_segment() {
        let valid_digest = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";
        let s = format!("docker.io/myorg/myrepo:stable@sha256:{}", valid_digest);
        let reference = s.parse::<Reference>().unwrap();
        assert_eq!(reference.registry, "docker.io");
        assert_eq!(reference.repository, "myorg/myrepo");
        match reference.selector {
            ReferenceSelector::Tag {
                ref tag,
                ref digest,
            } => {
                assert_eq!(tag, "stable");
                let d = digest.as_ref().expect("Expected digest");
                assert_eq!(d.to_string(), format!("sha256:{}", valid_digest));
            }
            _ => panic!("Expected Tag variant with digest"),
        }
        let expected = format!("docker.io/myorg/myrepo:stable@sha256:{}", valid_digest);
        assert_eq!(reference.to_string(), expected);
    }

    #[test]
    fn test_reference_complex_path() {
        let s = "registry.io/v2/image:tag";
        let reference = s.parse::<Reference>().unwrap();
        assert_eq!(reference.registry, "registry.io");
        assert_eq!(reference.repository, "v2/image");
        match reference.selector {
            ReferenceSelector::Tag {
                ref tag,
                ref digest,
            } => {
                assert_eq!(tag, "tag");
                assert!(digest.is_none());
            }
            _ => panic!("Expected Tag variant"),
        }
        assert_eq!(reference.to_string(), "registry.io/v2/image:tag");
    }

    #[test]
    fn test_reference_multi_slash_repository() {
        let s = "docker.io/a/b/c:1.0";
        let reference = s.parse::<Reference>().unwrap();
        assert_eq!(reference.registry, "docker.io");
        assert_eq!(reference.repository, "a/b/c");
        match reference.selector {
            ReferenceSelector::Tag {
                ref tag,
                ref digest,
            } => {
                assert_eq!(tag, "1.0");
                assert!(digest.is_none());
            }
            _ => panic!("Expected Tag variant"),
        }
        assert_eq!(reference.to_string(), "docker.io/a/b/c:1.0");
    }

    #[test]
    fn test_empty_input() {
        let s = "";
        let err = s.parse::<Reference>().unwrap_err();
        let err_str = err.to_string();
        assert!(err_str.contains("input string is empty"));
    }

    #[test]
    fn test_empty_repository() {
        let s = "registry.example.com/:tag";
        let err = s.parse::<Reference>().unwrap_err();
        let err_str = err.to_string();
        assert!(err_str.contains("repository is empty"));
    }

    #[test]
    fn test_reference_registry_ip_port_single_segment() {
        let s = "192.168.1.1:5000/ubuntu:18.04";
        let reference = s.parse::<Reference>().unwrap();
        assert_eq!(reference.registry, "192.168.1.1:5000");
        assert_eq!(
            reference.repository,
            format!("{}/ubuntu", DEFAULT_OCI_REFERENCE_REPO_NAMESPACE)
        );
        match reference.selector {
            ReferenceSelector::Tag {
                ref tag,
                ref digest,
            } => {
                assert_eq!(tag, "18.04");
                assert!(digest.is_none());
            }
            _ => panic!("Expected Tag variant"),
        }
        let expected = format!(
            "192.168.1.1:5000/{}/ubuntu:18.04",
            DEFAULT_OCI_REFERENCE_REPO_NAMESPACE
        );
        assert_eq!(reference.to_string(), expected);
    }

    #[test]
    fn test_reference_registry_ip_port_multi_segment() {
        let s = "192.168.1.1:5000/org/repo:version";
        let reference = s.parse::<Reference>().unwrap();
        assert_eq!(reference.registry, "192.168.1.1:5000");
        assert_eq!(reference.repository, "org/repo");
        match reference.selector {
            ReferenceSelector::Tag {
                ref tag,
                ref digest,
            } => {
                assert_eq!(tag, "version");
                assert!(digest.is_none());
            }
            _ => panic!("Expected Tag variant"),
        }
        let expected = "192.168.1.1:5000/org/repo:version".to_string();
        assert_eq!(reference.to_string(), expected);
    }

    #[test]
    fn test_reference_invalid_registry() {
        // Registry contains an invalid character '!' and is forced as a registry by containing a dot
        let s = "inva!id-registry.com/library/alpine:3.12";
        let err = s.parse::<Reference>().unwrap_err();
        assert!(err.to_string().contains("invalid registry"));
    }

    #[test]
    fn test_reference_invalid_repository() {
        // Repository contains uppercase letters which are invalid
        let s = "docker.io/Library/alpine:3.12";
        let err = s.parse::<Reference>().unwrap_err();
        assert!(err.to_string().contains("invalid repository"));
    }

    #[test]
    fn test_reference_invalid_tag() {
        // Tag contains an invalid character '!'
        let s = "docker.io/library/alpine:t!ag";
        let err = s.parse::<Reference>().unwrap_err();
        let err_str = err.to_string();
        assert!(err_str.contains("invalid tag"));
    }

    #[test]
    fn test_reference_tag_length_exceeds_limit() {
        // Create a tag of length 129 (exceeds max length of 128 characters)
        let long_tag = "a".repeat(129);
        let s = format!("docker.io/library/alpine:{}", long_tag);
        let err = s.parse::<Reference>().unwrap_err();
        assert!(err.to_string().contains("invalid tag"));
    }
}
