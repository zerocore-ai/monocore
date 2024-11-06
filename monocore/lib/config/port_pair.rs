use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

use crate::MonocoreError;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Represents a port mapping between host and guest systems, following Docker's port mapping convention.
///
/// ## Format
/// The port pair can be specified in two formats:
/// - `host:guest` - Maps the host port to a different guest port (e.g., "8080:80")
/// - `port` or `port:port` - Maps the same port number on both host and guest (e.g., "8080" or "8080:8080")
///
/// ## Examples
///
/// Creating port pairs:
/// ```
/// use monocore::config::PortPair;
///
/// // Same port on host and guest (8080:8080)
/// let same_port = PortPair::with_same(8080);
///
/// // Different ports (host 8080 maps to guest 80)
/// let distinct_ports = PortPair::with_distinct(8080, 80);
///
/// // Parse from string
/// let from_str = "8080:80".parse::<PortPair>().unwrap();
/// assert_eq!(from_str, distinct_ports);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortPair {
    /// The guest port and the host port are distinct.
    Distinct {
        /// The host port.
        host: u16,

        /// The guest port.
        guest: u16,
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
    pub fn with_distinct(host: u16, guest: u16) -> Self {
        Self::Distinct { host, guest }
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
            let (host, guest) = s.split_once(':').unwrap();
            if guest.is_empty() || host.is_empty() {
                return Err(MonocoreError::InvalidPortPair(s.to_string()));
            }

            if guest == host {
                return Ok(Self::Same(
                    host.parse()
                        .map_err(|_| MonocoreError::InvalidPortPair(s.to_string()))?,
                ));
            } else {
                return Ok(Self::Distinct {
                    host: host
                        .parse()
                        .map_err(|_| MonocoreError::InvalidPortPair(s.to_string()))?,
                    guest: guest
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
    /// Formats the port pair following the format "host:guest".
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Distinct { host, guest } => {
                write!(f, "{}:{}", host, guest)
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

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_port_pair_from_str() {
        // Test same ports
        assert_eq!("8080".parse::<PortPair>().unwrap(), PortPair::Same(8080));
        assert_eq!(
            "8080:8080".parse::<PortPair>().unwrap(),
            PortPair::Same(8080)
        );

        // Test distinct ports (host:guest format)
        assert_eq!(
            "8080:80".parse::<PortPair>().unwrap(),
            PortPair::Distinct {
                host: 8080,
                guest: 80
            }
        );

        // Test invalid formats
        assert!("".parse::<PortPair>().is_err());
        assert!(":80".parse::<PortPair>().is_err());
        assert!("80:".parse::<PortPair>().is_err());
        assert!("invalid".parse::<PortPair>().is_err());
        assert!("invalid:80".parse::<PortPair>().is_err());
        assert!("80:invalid".parse::<PortPair>().is_err());
    }

    #[test]
    fn test_port_pair_display() {
        // Test same ports
        assert_eq!(PortPair::Same(8080).to_string(), "8080:8080");

        // Test distinct ports (host:guest format)
        assert_eq!(
            PortPair::Distinct {
                host: 8080,
                guest: 80
            }
            .to_string(),
            "8080:80"
        );
    }

    #[test]
    fn test_port_pair_getters() {
        // Test same ports
        let same = PortPair::Same(8080);
        assert_eq!(same.get_host(), 8080);
        assert_eq!(same.get_guest(), 8080);

        // Test distinct ports
        let distinct = PortPair::Distinct {
            host: 8080,
            guest: 80,
        };
        assert_eq!(distinct.get_host(), 8080);
        assert_eq!(distinct.get_guest(), 80);
    }

    #[test]
    fn test_port_pair_constructors() {
        assert_eq!(PortPair::with_same(8080), PortPair::Same(8080));
        assert_eq!(
            PortPair::with_distinct(8080, 80),
            PortPair::Distinct {
                host: 8080,
                guest: 80
            }
        );
    }
}
