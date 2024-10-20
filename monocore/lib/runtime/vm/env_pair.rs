use crate::MonocoreError;
use getset::Getters;
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Represents an environment variable pair.
///
/// This struct encapsulates a variable name and its corresponding value.
/// It is used to manage environment variables for processes.
///
/// ## Examples
///
/// ```
/// use monocore::runtime::EnvPair;
/// use std::str::FromStr;
///
/// // Create a new environment variable pair
/// let env_pair = EnvPair::new("PATH", "/usr/local/bin:/usr/bin");
///
/// assert_eq!(env_pair.get_var(), "PATH");
/// assert_eq!(env_pair.get_value(), "/usr/local/bin:/usr/bin");
///
/// // Parse an environment variable pair from a string
/// let env_pair = EnvPair::from_str("USER=alice").unwrap();
///
/// assert_eq!(env_pair.get_var(), "USER");
/// assert_eq!(env_pair.get_value(), "alice");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Getters)]
#[getset(get = "pub with_prefix")]
pub struct EnvPair {
    /// The environment variable name.
    var: String,

    /// The value of the environment variable.
    value: String,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl EnvPair {
    /// Creates a new `EnvPair` with the given variable name and value.
    ///
    /// # Arguments
    ///
    /// * `var` - The name of the environment variable.
    /// * `value` - The value of the environment variable.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monocore::runtime::EnvPair;
    ///
    /// let env_pair = EnvPair::new("HOME", "/home/user");
    /// assert_eq!(env_pair.get_var(), "HOME");
    /// assert_eq!(env_pair.get_value(), "/home/user");
    /// ```
    pub fn new<S: Into<String>>(var: S, value: S) -> Self {
        Self {
            var: var.into(),
            value: value.into(),
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl FromStr for EnvPair {
    type Err = MonocoreError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (var, value) = s
            .split_once('=')
            .ok_or_else(|| MonocoreError::InvalidEnvPair(s.to_string()))?;

        if var.is_empty() {
            return Err(MonocoreError::InvalidEnvPair(s.to_string()));
        }

        Ok(Self::new(var, value))
    }
}

impl fmt::Display for EnvPair {
    /// Formats the environment variable pair following the format "<var>=<value>".
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={}", self.var, self.value)
    }
}

impl Serialize for EnvPair {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for EnvPair {
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
    fn test_env_pair_new() {
        let env_pair = EnvPair::new("VAR", "VALUE");
        assert_eq!(env_pair.var, String::from("VAR"));
        assert_eq!(env_pair.value, String::from("VALUE"));
    }

    #[test]
    fn test_env_pair_from_str() -> anyhow::Result<()> {
        let env_pair: EnvPair = "VAR=VALUE".parse()?;
        assert_eq!(env_pair.var, String::from("VAR"));
        assert_eq!(env_pair.value, String::from("VALUE"));

        let env_pair: EnvPair = "VAR=".parse()?;
        assert_eq!(env_pair.var, String::from("VAR"));
        assert_eq!(env_pair.value, String::from(""));

        assert!("VAR".parse::<EnvPair>().is_err());
        assert!("=VALUE".parse::<EnvPair>().is_err());

        Ok(())
    }

    #[test]
    fn test_env_pair_display() {
        let env_pair = EnvPair::new("VAR", "VALUE");
        assert_eq!(env_pair.to_string(), "VAR=VALUE");

        let env_pair = EnvPair::new("VAR", "");
        assert_eq!(env_pair.to_string(), "VAR=");
    }

    #[test]
    fn test_env_pair_serialize_deserialize() -> anyhow::Result<()> {
        let env_pair = EnvPair::new("VAR", "VALUE");
        let serialized = serde_json::to_string(&env_pair)?;
        assert_eq!(serialized, "\"VAR=VALUE\"");

        let deserialized: EnvPair = serde_json::from_str(&serialized)?;
        assert_eq!(deserialized, env_pair);

        let env_pair = EnvPair::new("VAR", "");
        let serialized = serde_json::to_string(&env_pair)?;
        assert_eq!(serialized, "\"VAR=\"");

        let deserialized: EnvPair = serde_json::from_str(&serialized)?;
        assert_eq!(deserialized, env_pair);

        Ok(())
    }

    #[test]
    fn test_env_pair_with_special_characters() -> anyhow::Result<()> {
        let env_pair: EnvPair = "VAR_WITH_UNDERSCORE=VALUE WITH SPACES".parse()?;
        assert_eq!(env_pair.get_var(), "VAR_WITH_UNDERSCORE");
        assert_eq!(env_pair.get_value(), "VALUE WITH SPACES");

        let env_pair: EnvPair = "VAR.WITH.DOTS=VALUE_WITH_UNDERSCORE".parse()?;
        assert_eq!(env_pair.get_var(), "VAR.WITH.DOTS");
        assert_eq!(env_pair.get_value(), "VALUE_WITH_UNDERSCORE");

        Ok(())
    }
}
