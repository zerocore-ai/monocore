use std::{
    fmt::{self, Display},
    str::FromStr,
    sync::LazyLock,
};

use oci_spec::image::Digest;
use regex::Regex;

use crate::MonocoreError;

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

/// Regular expression for validating image tags
/// Must start with [A-Za-z0-9] and can be followed by [A-Za-z0-9_.-] up to 127 chars total
static TAG_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-z0-9][A-Za-z0-9_.-]{0,126}$").unwrap());

/// Regular expression for validating digest algorithm and hex components
/// Both algorithm and hex must contain only [A-Za-z0-9_+.-]
static DIGEST_COMPONENT_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-z0-9_+.-]+$").unwrap());

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Represents a selector for an image reference, which can be either a tag or a digest.
///
/// Format rules:
/// - Tags can be specified either as bare strings (e.g. "latest") or with a ":" prefix (e.g. ":v1.0.0")
/// - Digests can be specified either with "@" prefix (e.g. "@sha256:abc123") or as algorithm:hex (e.g. "sha256:abc123")
/// - A string starting with ":" is always treated as a tag
/// - A string starting with "@" is always treated as a digest
/// - A string containing ":" but not starting with ":" is treated as a digest
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferenceSelector {
    /// A tag reference (e.g., "latest", ":v1.0.0")
    Tag(String),

    /// A digest reference with algorithm and hex components (e.g., "@sha256:abc...", "sha256:abc...")
    Digest {
        /// The hash algorithm (e.g., "sha256")
        algorithm: String,

        /// The hex digest
        hex: String,
    },
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl ReferenceSelector {
    /// Parse a tag string, validating its format
    fn parse_tag(s: &str) -> Result<Self, MonocoreError> {
        if !TAG_REGEX.is_match(s) {
            return Err(MonocoreError::InvalidReferenceSelectorFormat(format!(
                "invalid tag format: must match {}",
                TAG_REGEX.as_str()
            )));
        }
        Ok(ReferenceSelector::Tag(s.to_string()))
    }

    /// Parse a digest string into algorithm and hex components
    fn parse_digest(s: &str) -> Result<Self, MonocoreError> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 2 {
            return Err(MonocoreError::InvalidReferenceSelectorFormat(
                "digest must be in format 'algorithm:hex'".to_string(),
            ));
        }

        let algorithm = parts[0];
        let hex = parts[1];

        // Validate algorithm
        if !DIGEST_COMPONENT_REGEX.is_match(algorithm) {
            return Err(MonocoreError::InvalidReferenceSelectorDigest(format!(
                "invalid algorithm format: must match {}",
                DIGEST_COMPONENT_REGEX.as_str()
            )));
        }

        // Validate hex
        if !DIGEST_COMPONENT_REGEX.is_match(hex) {
            return Err(MonocoreError::InvalidReferenceSelectorDigest(format!(
                "invalid hex format: must match {}",
                DIGEST_COMPONENT_REGEX.as_str()
            )));
        }

        Ok(ReferenceSelector::Digest {
            algorithm: algorithm.to_string(),
            hex: hex.to_string(),
        })
    }

    /// Convert to OCI Digest type when needed
    pub fn to_oci_digest(&self) -> Option<Digest> {
        match self {
            ReferenceSelector::Tag(_) => None,
            ReferenceSelector::Digest { algorithm, hex } => {
                Digest::from_str(&format!("{}:{}", algorithm, hex)).ok()
            }
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl FromStr for ReferenceSelector {
    type Err = MonocoreError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Err(MonocoreError::InvalidReferenceSelectorFormat(
                "empty selector".to_string(),
            ));
        }

        // Handle explicit digest format with @ prefix
        if s.starts_with('@') {
            return Self::parse_digest(s.trim_start_matches('@'));
        }

        // Handle explicit tag format with : prefix
        if s.starts_with(':') {
            return Self::parse_tag(s.trim_start_matches(':'));
        }

        // If it contains a colon in the middle, it must be a digest
        if s.contains(':') {
            return Self::parse_digest(s);
        }

        // Otherwise treat as tag
        Self::parse_tag(s)
    }
}

impl Display for ReferenceSelector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReferenceSelector::Tag(tag) => write!(f, "{}", tag),
            ReferenceSelector::Digest { algorithm, hex } => write!(f, "{}:{}", algorithm, hex),
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selector_valid_tag_formats() {
        // Simple tags
        assert!(ReferenceSelector::from_str("latest").is_ok());
        assert!(ReferenceSelector::from_str("v1").is_ok());
        assert!(ReferenceSelector::from_str("1.0").is_ok());

        // Complex valid tags
        assert!(ReferenceSelector::from_str("v1.0.0-alpha.1").is_ok());
        assert!(ReferenceSelector::from_str("release_2.1.0").is_ok());
        assert!(ReferenceSelector::from_str("3.1.0-debian-11-r47").is_ok());

        // Tags with explicit prefix
        assert!(ReferenceSelector::from_str(":latest").is_ok());
        assert!(ReferenceSelector::from_str(":v1.0.0").is_ok());
    }

    #[test]
    fn test_selector_invalid_tag_formats() {
        // Empty tags
        assert!(ReferenceSelector::from_str("").is_err());
        assert!(ReferenceSelector::from_str(":").is_err());

        // Invalid starting characters
        assert!(ReferenceSelector::from_str("_invalid").is_err());
        assert!(ReferenceSelector::from_str(".invalid").is_err());
        assert!(ReferenceSelector::from_str("-invalid").is_err());

        // Invalid characters
        assert!(ReferenceSelector::from_str("tag:with:colons").is_err());
        assert!(ReferenceSelector::from_str("tag@with@at").is_err());
        assert!(ReferenceSelector::from_str("tag with spaces").is_err());
        assert!(ReferenceSelector::from_str("tag/with/slashes").is_err());

        // Length validation
        let too_long_tag = "a".repeat(128);
        assert!(ReferenceSelector::from_str(&too_long_tag).is_err());
    }

    #[test]
    fn test_selector_valid_digest_formats() {
        // Standard SHA256
        assert!(ReferenceSelector::from_str(
            "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        )
        .is_ok());

        // With @ prefix
        assert!(ReferenceSelector::from_str(
            "@sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        )
        .is_ok());

        // Different algorithms
        assert!(ReferenceSelector::from_str(
            "sha512:d9022c66d40b3eb3e5c29ab6a3f4833248544d30a2aa3c60b82be636c7270192"
        )
        .is_ok());
        assert!(
            ReferenceSelector::from_str("blake2b:d9022c66d40b3eb3e5c29ab6a3f4833248544d30").is_ok()
        );
    }

    #[test]
    fn test_selector_invalid_digest_formats() {
        // Missing hex component
        assert!(ReferenceSelector::from_str("sha256:").is_err());
        assert!(ReferenceSelector::from_str("@:").is_err());
        assert!(ReferenceSelector::from_str("@sha256:").is_err());

        // Missing algorithm component
        assert!(ReferenceSelector::from_str(":abc123").is_ok()); // This is actually a valid tag!
        assert!(ReferenceSelector::from_str("@:abc123").is_err());
        assert!(ReferenceSelector::from_str(":").is_err());
        assert!(ReferenceSelector::from_str("@").is_err());

        // Invalid characters in algorithm
        assert!(ReferenceSelector::from_str("sha256$:abc123").is_err());
        assert!(ReferenceSelector::from_str("sha 256:abc123").is_err());
        assert!(ReferenceSelector::from_str("@sha256$:abc123").is_err());

        // Invalid characters in hex
        assert!(ReferenceSelector::from_str("sha256:xyz!@#").is_err());
        assert!(ReferenceSelector::from_str("sha256:abc 123").is_err());
        assert!(ReferenceSelector::from_str("@sha256:xyz!@#").is_err());

        // Too many components
        assert!(ReferenceSelector::from_str("sha256:abc:123").is_err());
        assert!(ReferenceSelector::from_str("@sha256:abc:123").is_err());
    }

    #[test]
    fn test_selector_display_format() {
        // Tag display
        let tag = ReferenceSelector::Tag("latest".to_string());
        assert_eq!(tag.to_string(), "latest");

        // Digest display
        let digest = ReferenceSelector::Digest {
            algorithm: "sha256".to_string(),
            hex: "abc123".to_string(),
        };
        assert_eq!(digest.to_string(), "sha256:abc123");
    }

    #[test]
    fn test_selector_to_oci_digest() {
        // Tag should return None
        let tag = ReferenceSelector::Tag("latest".to_string());
        assert!(tag.to_oci_digest().is_none());

        // Valid digest should convert successfully
        let digest = ReferenceSelector::Digest {
            algorithm: "sha256".to_string(),
            hex: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string(),
        };
        assert!(digest.to_oci_digest().is_some());

        // Invalid digest format should return None
        let invalid_digest = ReferenceSelector::Digest {
            algorithm: "sha256".to_string(),
            hex: "invalid".to_string(),
        };
        assert!(invalid_digest.to_oci_digest().is_none());
    }

    #[test]
    fn test_selector_parse_display_roundtrip() {
        let test_selector_cases = [
            "latest",
            "v1.0.0",
            "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        ];

        for &case in test_selector_cases.iter() {
            let selector = ReferenceSelector::from_str(case).unwrap();
            assert_eq!(selector.to_string(), case);
        }
    }

    #[test]
    fn test_selector_error_messages() {
        // Empty selector
        match ReferenceSelector::from_str("") {
            Err(MonocoreError::InvalidReferenceSelectorFormat(msg)) => {
                assert_eq!(msg, "empty selector");
            }
            _ => panic!("Expected InvalidReferenceSelectorFormat error"),
        }

        // Invalid tag format
        match ReferenceSelector::from_str("_invalid") {
            Err(MonocoreError::InvalidReferenceSelectorFormat(msg)) => {
                assert!(msg.contains("invalid tag format"));
            }
            _ => panic!("Expected InvalidReferenceSelectorFormat error"),
        }

        // Invalid digest format
        match ReferenceSelector::from_str("sha256:abc:def") {
            Err(MonocoreError::InvalidReferenceSelectorFormat(msg)) => {
                assert!(msg.contains("digest must be in format"));
            }
            _ => panic!("Expected InvalidReferenceSelectorFormat error"),
        }
    }
}
