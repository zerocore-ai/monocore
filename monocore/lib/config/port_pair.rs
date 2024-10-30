use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

use crate::MonocoreError;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A pair of ports to map between the host and the guest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortPair {
    /// The guest port and the host port are distinct.
    Distinct {
        /// The guest port.
        guest: u16,

        /// The host port.
        host: u16,
    },

    /// The guest port and the host port are the same.
    Same(u16),
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl PortPair {
    /// Creates a new `PortPair` with the same guest and host port.
    pub fn with_same(port: u16) -> Self {
        Self::Same(port)
    }

    /// Creates a new `PortPair` with distinct guest and host ports.
    pub fn with_distinct(guest: u16, host: u16) -> Self {
        Self::Distinct { guest, host }
    }

    /// Returns the host port.
    pub fn get_host(&self) -> u16 {
        match self {
            Self::Distinct { host, .. } | Self::Same(host) => *host,
        }
    }

    /// Returns the guest port.
    pub fn get_guest(&self) -> u16 {
        match self {
            Self::Distinct { guest, .. } | Self::Same(guest) => *guest,
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl FromStr for PortPair {
    type Err = MonocoreError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Err(MonocoreError::InvalidPortPair(s.to_string()));
        }

        if s.contains(':') {
            let (guest, host) = s.split_once(':').unwrap();
            if guest.is_empty() || host.is_empty() {
                return Err(MonocoreError::InvalidPortPair(s.to_string()));
            }

            if guest == host {
                return Ok(Self::Same(
                    guest
                        .parse()
                        .map_err(|_| MonocoreError::InvalidPortPair(s.to_string()))?,
                ));
            } else {
                return Ok(Self::Distinct {
                    guest: guest
                        .parse()
                        .map_err(|_| MonocoreError::InvalidPortPair(s.to_string()))?,
                    host: host
                        .parse()
                        .map_err(|_| MonocoreError::InvalidPortPair(s.to_string()))?,
                });
            }
        }

        Ok(Self::Same(s.parse().map_err(|_| {
            MonocoreError::InvalidPortPair(s.to_string())
        })?))
    }
}

impl fmt::Display for PortPair {
    /// Formats the path pair following the format "<guest>:<host>".
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Distinct { guest, host } => {
                write!(f, "{}:{}", guest, host)
            }
            Self::Same(port) => write!(f, "{}:{}", port, port),
        }
    }
}

impl Serialize for PortPair {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for PortPair {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::from_str(&s).map_err(serde::de::Error::custom)
    }
}
