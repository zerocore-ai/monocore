use std::{fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use typed_path::Utf8UnixPathBuf;

use crate::MonocoreError;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Represents a path on the host and the guest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathPair {
    /// The guest path and host path are distinct.
    Distinct {
        /// The guest path.
        guest: Utf8UnixPathBuf,

        /// The host path.
        host: Utf8UnixPathBuf,
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

    /// Creates a new `PathPair` with distinct guest and host paths.
    pub fn with_distinct(guest: Utf8UnixPathBuf, host: Utf8UnixPathBuf) -> Self {
        Self::Distinct { guest, host }
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
            let (guest, host) = s.split_once(':').unwrap();
            if guest.is_empty() || host.is_empty() {
                return Err(MonocoreError::InvalidPathPair(s.to_string()));
            }

            if guest == host {
                return Ok(Self::Same(guest.into()));
            } else {
                return Ok(Self::Distinct {
                    guest: guest.into(),
                    host: host.into(),
                });
            }
        }

        Ok(Self::Same(s.into()))
    }
}

impl fmt::Display for PathPair {
    /// Formats the path pair following the format "<guest>:<host>".
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Distinct { guest, host } => {
                write!(f, "{}:{}", guest, host)
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
