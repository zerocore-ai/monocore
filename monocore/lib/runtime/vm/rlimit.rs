use crate::MonocoreError;
use getset::Getters;
use serde::{Deserialize, Serialize};
use std::{convert::TryFrom, fmt, str::FromStr};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Represents the available Linux resource limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
#[allow(non_camel_case_types)]
pub enum LinuxRLimitResource {
    /// CPU time in seconds
    RLIMIT_CPU = 0,

    /// Maximum size of files created by the process
    RLIMIT_FSIZE = 1,

    /// Maximum size of the data segment
    RLIMIT_DATA = 2,

    /// Maximum size of the stack segment
    RLIMIT_STACK = 3,

    /// Maximum size of core dumps
    RLIMIT_CORE = 4,

    /// Maximum resident set size (not enforced on Linux)
    RLIMIT_RSS = 5,

    /// Maximum number of processes
    RLIMIT_NPROC = 6,

    /// Maximum number of open file descriptors
    RLIMIT_NOFILE = 7,

    /// Maximum locked memory size
    RLIMIT_MEMLOCK = 8,

    /// Maximum size of the address space
    RLIMIT_AS = 9,

    /// Maximum number of file locks
    RLIMIT_LOCKS = 10,

    /// Maximum number of signals that can be queued
    RLIMIT_SIGPENDING = 11,

    /// Maximum number of bytes in POSIX message queues
    RLIMIT_MSGQUEUE = 12,

    /// Maximum nice priority
    RLIMIT_NICE = 13,

    /// Maximum real-time priority
    RLIMIT_RTPRIO = 14,

    /// Maximum seconds to sleep in real time
    RLIMIT_RTTIME = 15,
}

/// Represents a resource limit for a Linux process.
///
/// This struct encapsulates a resource type and its corresponding soft and hard limits.
/// The soft limit is the value that the kernel enforces for the corresponding resource.
/// The hard limit acts as a ceiling for the soft limit.
///
/// ## Examples
///
/// ```
/// use monocore::runtime::{LinuxRlimit, LinuxRLimitResource};
///
/// // Create a new resource limit for CPU time
/// let cpu_limit = LinuxRlimit::new(LinuxRLimitResource::RLIMIT_CPU, 10, 20);
///
/// assert_eq!(cpu_limit.resource(), &LinuxRLimitResource::RLIMIT_CPU);
/// assert_eq!(cpu_limit.soft(), &10);
/// assert_eq!(cpu_limit.hard(), &20);
///
/// // Parse a resource limit from a string
/// let nofile_limit: LinuxRlimit = "RLIMIT_NOFILE=1000:2000".parse().unwrap();
///
/// assert_eq!(nofile_limit.resource(), &LinuxRLimitResource::RLIMIT_NOFILE);
/// assert_eq!(nofile_limit.soft(), &1000);
/// assert_eq!(nofile_limit.hard(), &2000);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Getters)]
#[getset(get = "pub with_prefix")]
pub struct LinuxRlimit {
    /// The resource to limit.
    resource: LinuxRLimitResource,

    /// The soft limit of the resource.
    ///
    /// This is the value that the kernel enforces for the corresponding resource.
    soft: u64,

    /// The hard limit of the resource.
    ///
    /// This acts as a ceiling for the soft limit.
    hard: u64,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl LinuxRLimitResource {
    /// Get the corresponding enum integer value
    pub fn as_int(&self) -> u32 {
        *self as u32
    }
}

impl LinuxRlimit {
    /// Creates a new `LinuxRlimit` instance with the specified resource, soft limit, and hard limit.
    ///
    /// # Arguments
    ///
    /// * `resource` - The resource type to limit.
    /// * `soft` - The soft limit value.
    /// * `hard` - The hard limit value.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monocore::runtime::{LinuxRlimit, LinuxRLimitResource};
    ///
    /// let cpu_limit = LinuxRlimit::new(LinuxRLimitResource::RLIMIT_CPU, 10, 20);
    /// assert_eq!(cpu_limit.resource(), &LinuxRLimitResource::RLIMIT_CPU);
    /// assert_eq!(cpu_limit.soft(), &10);
    /// assert_eq!(cpu_limit.hard(), &20);
    /// ```
    pub fn new(resource: LinuxRLimitResource, soft: u64, hard: u64) -> Self {
        Self {
            resource,
            soft,
            hard,
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl TryFrom<u32> for LinuxRLimitResource {
    type Error = MonocoreError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::RLIMIT_CPU),
            1 => Ok(Self::RLIMIT_FSIZE),
            2 => Ok(Self::RLIMIT_DATA),
            3 => Ok(Self::RLIMIT_STACK),
            4 => Ok(Self::RLIMIT_CORE),
            5 => Ok(Self::RLIMIT_RSS),
            6 => Ok(Self::RLIMIT_NPROC),
            7 => Ok(Self::RLIMIT_NOFILE),
            8 => Ok(Self::RLIMIT_MEMLOCK),
            9 => Ok(Self::RLIMIT_AS),
            10 => Ok(Self::RLIMIT_LOCKS),
            11 => Ok(Self::RLIMIT_SIGPENDING),
            12 => Ok(Self::RLIMIT_MSGQUEUE),
            13 => Ok(Self::RLIMIT_NICE),
            14 => Ok(Self::RLIMIT_RTPRIO),
            15 => Ok(Self::RLIMIT_RTTIME),
            _ => Err(MonocoreError::InvalidRLimitResource(value.to_string())),
        }
    }
}

impl FromStr for LinuxRLimitResource {
    type Err = MonocoreError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "RLIMIT_CPU" => Ok(Self::RLIMIT_CPU),
            "RLIMIT_FSIZE" => Ok(Self::RLIMIT_FSIZE),
            "RLIMIT_DATA" => Ok(Self::RLIMIT_DATA),
            "RLIMIT_STACK" => Ok(Self::RLIMIT_STACK),
            "RLIMIT_CORE" => Ok(Self::RLIMIT_CORE),
            "RLIMIT_RSS" => Ok(Self::RLIMIT_RSS),
            "RLIMIT_NPROC" => Ok(Self::RLIMIT_NPROC),
            "RLIMIT_NOFILE" => Ok(Self::RLIMIT_NOFILE),
            "RLIMIT_MEMLOCK" => Ok(Self::RLIMIT_MEMLOCK),
            "RLIMIT_AS" => Ok(Self::RLIMIT_AS),
            "RLIMIT_LOCKS" => Ok(Self::RLIMIT_LOCKS),
            "RLIMIT_SIGPENDING" => Ok(Self::RLIMIT_SIGPENDING),
            "RLIMIT_MSGQUEUE" => Ok(Self::RLIMIT_MSGQUEUE),
            "RLIMIT_NICE" => Ok(Self::RLIMIT_NICE),
            "RLIMIT_RTPRIO" => Ok(Self::RLIMIT_RTPRIO),
            "RLIMIT_RTTIME" => Ok(Self::RLIMIT_RTTIME),
            _ => Err(MonocoreError::InvalidRLimitResource(s.to_string())),
        }
    }
}

impl fmt::Display for LinuxRLimitResource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RLIMIT_CPU => write!(f, "RLIMIT_CPU"),
            Self::RLIMIT_FSIZE => write!(f, "RLIMIT_FSIZE"),
            Self::RLIMIT_DATA => write!(f, "RLIMIT_DATA"),
            Self::RLIMIT_STACK => write!(f, "RLIMIT_STACK"),
            Self::RLIMIT_CORE => write!(f, "RLIMIT_CORE"),
            Self::RLIMIT_RSS => write!(f, "RLIMIT_RSS"),
            Self::RLIMIT_NPROC => write!(f, "RLIMIT_NPROC"),
            Self::RLIMIT_NOFILE => write!(f, "RLIMIT_NOFILE"),
            Self::RLIMIT_MEMLOCK => write!(f, "RLIMIT_MEMLOCK"),
            Self::RLIMIT_AS => write!(f, "RLIMIT_AS"),
            Self::RLIMIT_LOCKS => write!(f, "RLIMIT_LOCKS"),
            Self::RLIMIT_SIGPENDING => write!(f, "RLIMIT_SIGPENDING"),
            Self::RLIMIT_MSGQUEUE => write!(f, "RLIMIT_MSGQUEUE"),
            Self::RLIMIT_NICE => write!(f, "RLIMIT_NICE"),
            Self::RLIMIT_RTPRIO => write!(f, "RLIMIT_RTPRIO"),
            Self::RLIMIT_RTTIME => write!(f, "RLIMIT_RTTIME"),
        }
    }
}

impl FromStr for LinuxRlimit {
    type Err = MonocoreError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split('=').collect();
        if parts.len() != 2 {
            return Err(MonocoreError::InvalidRLimitFormat(s.to_string()));
        }

        let resource = if let Ok(resource_num) = parts[0].parse::<u32>() {
            LinuxRLimitResource::try_from(resource_num)?
        } else {
            parts[0].parse()?
        };

        let limits: Vec<&str> = parts[1].split(':').collect();
        if limits.len() != 2 {
            return Err(MonocoreError::InvalidRLimitFormat(s.to_string()));
        }

        let soft = limits[0]
            .parse()
            .map_err(|_| MonocoreError::InvalidRLimitValue(limits[0].to_string()))?;
        let hard = limits[1]
            .parse()
            .map_err(|_| MonocoreError::InvalidRLimitValue(limits[1].to_string()))?;

        Ok(Self::new(resource, soft, hard))
    }
}

impl fmt::Display for LinuxRlimit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={}:{}", self.resource.as_int(), self.soft, self.hard)
    }
}

impl Serialize for LinuxRlimit {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for LinuxRlimit {
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
    fn test_linux_rlimit_resource_from_u32() -> anyhow::Result<()> {
        assert_eq!(
            LinuxRLimitResource::try_from(0)?,
            LinuxRLimitResource::RLIMIT_CPU
        );
        assert_eq!(
            LinuxRLimitResource::try_from(7)?,
            LinuxRLimitResource::RLIMIT_NOFILE
        );
        assert_eq!(
            LinuxRLimitResource::try_from(15)?,
            LinuxRLimitResource::RLIMIT_RTTIME
        );
        assert!(LinuxRLimitResource::try_from(16).is_err());
        Ok(())
    }

    #[test]
    fn test_linux_rlimit_resource_as_int() {
        assert_eq!(LinuxRLimitResource::RLIMIT_CPU.as_int(), 0);
        assert_eq!(LinuxRLimitResource::RLIMIT_NOFILE.as_int(), 7);
        assert_eq!(LinuxRLimitResource::RLIMIT_RTTIME.as_int(), 15);
    }

    #[test]
    fn test_linux_rlimit_resource_from_str() -> anyhow::Result<()> {
        assert_eq!(
            "RLIMIT_CPU".parse::<LinuxRLimitResource>()?,
            LinuxRLimitResource::RLIMIT_CPU
        );
        assert_eq!(
            "RLIMIT_NOFILE".parse::<LinuxRLimitResource>()?,
            LinuxRLimitResource::RLIMIT_NOFILE
        );
        assert_eq!(
            "RLIMIT_RTTIME".parse::<LinuxRLimitResource>()?,
            LinuxRLimitResource::RLIMIT_RTTIME
        );
        assert!("RLIMIT_INVALID".parse::<LinuxRLimitResource>().is_err());
        Ok(())
    }

    #[test]
    fn test_linux_rlimit_resource_display() {
        assert_eq!(LinuxRLimitResource::RLIMIT_CPU.to_string(), "RLIMIT_CPU");
        assert_eq!(
            LinuxRLimitResource::RLIMIT_NOFILE.to_string(),
            "RLIMIT_NOFILE"
        );
        assert_eq!(
            LinuxRLimitResource::RLIMIT_RTTIME.to_string(),
            "RLIMIT_RTTIME"
        );
    }

    #[test]
    fn test_linux_rlimit_new() {
        let rlimit = LinuxRlimit::new(LinuxRLimitResource::RLIMIT_CPU, 10, 20);
        assert_eq!(rlimit.resource, LinuxRLimitResource::RLIMIT_CPU);
        assert_eq!(rlimit.soft, 10);
        assert_eq!(rlimit.hard, 20);

        let rlimit = LinuxRlimit::new(LinuxRLimitResource::RLIMIT_NOFILE, 1000, 2000);
        assert_eq!(rlimit.resource, LinuxRLimitResource::RLIMIT_NOFILE);
        assert_eq!(rlimit.soft, 1000);
        assert_eq!(rlimit.hard, 2000);
    }

    #[test]
    fn test_linux_rlimit_from_str_with_rlimit_syntax() -> anyhow::Result<()> {
        let rlimit: LinuxRlimit = "RLIMIT_CPU=10:20".parse()?;
        assert_eq!(rlimit.resource, LinuxRLimitResource::RLIMIT_CPU);
        assert_eq!(rlimit.soft, 10);
        assert_eq!(rlimit.hard, 20);

        let rlimit: LinuxRlimit = "RLIMIT_NOFILE=1000:2000".parse()?;
        assert_eq!(rlimit.resource, LinuxRLimitResource::RLIMIT_NOFILE);
        assert_eq!(rlimit.soft, 1000);
        assert_eq!(rlimit.hard, 2000);

        let rlimit: LinuxRlimit = "RLIMIT_AS=1048576:2097152".parse()?;
        assert_eq!(rlimit.resource, LinuxRLimitResource::RLIMIT_AS);
        assert_eq!(rlimit.soft, 1048576);
        assert_eq!(rlimit.hard, 2097152);

        assert!("RLIMIT_INVALID=10:20".parse::<LinuxRlimit>().is_err());
        assert!("RLIMIT_CPU=10".parse::<LinuxRlimit>().is_err());
        assert!("RLIMIT_CPU=10:".parse::<LinuxRlimit>().is_err());
        assert!("RLIMIT_CPU=:20".parse::<LinuxRlimit>().is_err());
        Ok(())
    }

    #[test]
    fn test_linux_rlimit_from_str_mixed_syntax() -> anyhow::Result<()> {
        let rlimit: LinuxRlimit = "0=10:20".parse()?;
        assert_eq!(rlimit.resource, LinuxRLimitResource::RLIMIT_CPU);
        assert_eq!(rlimit.soft, 10);
        assert_eq!(rlimit.hard, 20);

        let rlimit: LinuxRlimit = "RLIMIT_NOFILE=1000:2000".parse()?;
        assert_eq!(rlimit.resource, LinuxRLimitResource::RLIMIT_NOFILE);
        assert_eq!(rlimit.soft, 1000);
        assert_eq!(rlimit.hard, 2000);

        Ok(())
    }

    #[test]
    fn test_linux_rlimit_display() {
        let rlimit = LinuxRlimit::new(LinuxRLimitResource::RLIMIT_CPU, 10, 20);
        assert_eq!(rlimit.to_string(), "0=10:20");

        let rlimit = LinuxRlimit::new(LinuxRLimitResource::RLIMIT_NOFILE, 1000, 2000);
        assert_eq!(rlimit.to_string(), "7=1000:2000");
    }

    #[test]
    fn test_linux_rlimit_serialize_deserialize() -> anyhow::Result<()> {
        let rlimit = LinuxRlimit::new(LinuxRLimitResource::RLIMIT_CPU, 10, 20);
        let serialized = serde_json::to_string(&rlimit)?;
        assert_eq!(serialized, "\"0=10:20\"");

        let deserialized: LinuxRlimit = serde_json::from_str(&serialized)?;
        assert_eq!(deserialized, rlimit);

        let rlimit = LinuxRlimit::new(LinuxRLimitResource::RLIMIT_NOFILE, 1000, 2000);
        let serialized = serde_json::to_string(&rlimit)?;
        assert_eq!(serialized, "\"7=1000:2000\"");

        let deserialized: LinuxRlimit = serde_json::from_str(&serialized)?;
        assert_eq!(deserialized, rlimit);

        Ok(())
    }
}
