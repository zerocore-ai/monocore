use std::{fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use typed_path::Utf8UnixPathBuf;

use crate::MonocoreError;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Represents a path mapping between host and guest systems, following Docker's volume mapping convention.
///
/// ## Format
/// The path pair can be specified in two formats:
/// - `host:guest` - Maps a host path to a different guest path (e.g., "/host/path:/container/path")
/// - `path` or `path:path` - Maps the same path on both host and guest (e.g., "/data" or "/data:/data")
///
/// ## Examples
///
/// Creating path pairs:
/// ```
/// use monocore::config::PathPair;
/// use typed_path::Utf8UnixPathBuf;
///
/// // Same path on host and guest (/data:/data)
/// let same_path = PathPair::with_same("/data".into());
///
/// // Different paths (host /host/data maps to guest /container/data)
/// let distinct_paths = PathPair::with_distinct(
///     "/host/data".into(),
///     "/container/data".into()
/// );
///
/// // Parse from string
/// let from_str = "/host/data:/container/data".parse::<PathPair>().unwrap();
/// assert_eq!(from_str, distinct_paths);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathPair {
    /// The guest path and host path are distinct.
    Distinct {
        /// The host path.
        host: Utf8UnixPathBuf,

        /// The guest path.
        guest: Utf8UnixPathBuf,
    },
    /// The guest path and host path are the same.
    Same(Utf8UnixPathBuf),
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl PathPair {
    /// Creates a new `PathPair` with the same host and guest path.
    pub fn with_same(path: Utf8UnixPathBuf) -> Self {
        Self::Same(path)
    }

    /// Creates a new `PathPair` with distinct host and guest paths.
    pub fn with_distinct(host: Utf8UnixPathBuf, guest: Utf8UnixPathBuf) -> Self {
        Self::Distinct { host, guest }
    }

    /// Returns the host path.
    pub fn get_host(&self) -> &Utf8UnixPathBuf {
        match self {
            Self::Distinct { host, .. } | Self::Same(host) => host,
        }
    }

    /// Returns the guest path.
    pub fn get_guest(&self) -> &Utf8UnixPathBuf {
        match self {
            Self::Distinct { guest, .. } | Self::Same(guest) => guest,
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl FromStr for PathPair {
    type Err = MonocoreError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Err(MonocoreError::InvalidPathPair(s.to_string()));
        }

        if s.contains(':') {
            let (host, guest) = s.split_once(':').unwrap();
            if guest.is_empty() || host.is_empty() {
                return Err(MonocoreError::InvalidPathPair(s.to_string()));
            }

            if guest == host {
                return Ok(Self::Same(host.into()));
            } else {
                return Ok(Self::Distinct {
                    host: host.into(),
                    guest: guest.into(),
                });
            }
        }

        Ok(Self::Same(s.into()))
    }
}

impl fmt::Display for PathPair {
    /// Formats the path pair following the format "host:guest".
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Distinct { host, guest } => {
                write!(f, "{}:{}", host, guest)
            }
            Self::Same(path) => write!(f, "{}:{}", path, path),
        }
    }
}

impl Serialize for PathPair {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for PathPair {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::from_str(&s).map_err(serde::de::Error::custom)
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_pair_from_str() {
        // Test same paths
        assert_eq!(
            "/data".parse::<PathPair>().unwrap(),
            PathPair::Same("/data".into())
        );
        assert_eq!(
            "/data:/data".parse::<PathPair>().unwrap(),
            PathPair::Same("/data".into())
        );

        // Test distinct paths (host:guest format)
        assert_eq!(
            "/host/data:/container/data".parse::<PathPair>().unwrap(),
            PathPair::Distinct {
                host: "/host/data".into(),
                guest: "/container/data".into()
            }
        );

        // Test invalid formats
        assert!("".parse::<PathPair>().is_err());
        assert!(":".parse::<PathPair>().is_err());
        assert!(":/data".parse::<PathPair>().is_err());
        assert!("/data:".parse::<PathPair>().is_err());
    }

    #[test]
    fn test_path_pair_display() {
        // Test same paths
        assert_eq!(PathPair::Same("/data".into()).to_string(), "/data:/data");

        // Test distinct paths (host:guest format)
        assert_eq!(
            PathPair::Distinct {
                host: "/host/data".into(),
                guest: "/container/data".into()
            }
            .to_string(),
            "/host/data:/container/data"
        );
    }

    #[test]
    fn test_path_pair_getters() {
        // Test same paths
        let same = PathPair::Same("/data".into());
        assert_eq!(same.get_host().as_str(), "/data");
        assert_eq!(same.get_guest().as_str(), "/data");

        // Test distinct paths
        let distinct = PathPair::Distinct {
            host: "/host/data".into(),
            guest: "/container/data".into(),
        };
        assert_eq!(distinct.get_host().as_str(), "/host/data");
        assert_eq!(distinct.get_guest().as_str(), "/container/data");
    }

    #[test]
    fn test_path_pair_constructors() {
        assert_eq!(
            PathPair::with_same("/data".into()),
            PathPair::Same("/data".into())
        );
        assert_eq!(
            PathPair::with_distinct("/host/data".into(), "/container/data".into()),
            PathPair::Distinct {
                host: "/host/data".into(),
                guest: "/container/data".into()
            }
        );
    }
}
