//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

use std::{fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use typed_path::UnixPathBuf;

use crate::MonocoreError;

/// Represents a path on the host and the guest.
#[derive(Debug, PartialEq)]
pub enum PathPair {
    /// The guest path and host path are distinct.
    Distinct {
        /// The guest path.
        guest: UnixPathBuf,

        /// The host path.
        host: UnixPathBuf,
    },
    /// The guest path and host path are the same.
    Same(UnixPathBuf),
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl PathPair {
    /// Creates a new `PathPair` with the same host and guest path.
    pub fn with_same(path: UnixPathBuf) -> Self {
        Self::Same(path)
    }

    /// Creates a new `PathPair` with distinct guest and host paths.
    pub fn with_distinct(guest: UnixPathBuf, host: UnixPathBuf) -> Self {
        Self::Distinct { guest, host }
    }

    /// Returns the host path.
    pub fn host(&self) -> &UnixPathBuf {
        match self {
            Self::Distinct { host, .. } | Self::Same(host) => host,
        }
    }

    /// Returns the guest path.
    pub fn guest(&self) -> &UnixPathBuf {
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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Distinct { guest, host } => {
                write!(f, "{}:{}", guest.to_string_lossy(), host.to_string_lossy())
            }
            Self::Same(path) => write!(f, "{}", path.to_string_lossy()),
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
