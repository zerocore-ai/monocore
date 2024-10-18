use std::{
    fmt::{self, Display},
    str::FromStr,
};

use typed_path::{Utf8UnixComponent, Utf8UnixPath, Utf8UnixPathBuf};

use crate::FsError;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Represents a single segment of a UTF-8 encoded Unix path.
///
/// This struct provides a way to represent and manipulate individual components
/// of a Unix path, ensuring they are valid UTF-8 strings and non-empty.
///
/// ## Examples
///
/// ```
/// use std::str::FromStr;
/// use monocore::monofs::filesystem::dir::Utf8UnixPathSegment;
///
/// let segment = Utf8UnixPathSegment::from_str("example").unwrap();
///
/// assert_eq!(segment.as_str(), "example");
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Utf8UnixPathSegment(String);

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl Utf8UnixPathSegment {
    /// Returns the string representation of the segment.
    ///
    /// ## Examples
    ///
    /// ```
    /// use std::str::FromStr;
    /// use monocore::monofs::filesystem::dir::Utf8UnixPathSegment;
    ///
    /// let segment = Utf8UnixPathSegment::from_str("example").unwrap();
    ///
    /// assert_eq!(segment.as_str(), "example");
    /// ```
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the bytes representation of the segment.
    ///
    /// ## Examples
    ///
    /// ```
    /// use std::str::FromStr;
    /// use monocore::monofs::filesystem::dir::Utf8UnixPathSegment;
    ///
    /// let segment = Utf8UnixPathSegment::from_str("example").unwrap();
    ///
    /// assert_eq!(segment.as_bytes(), b"example");
    /// ```
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    /// Returns the length of the segment in bytes.
    ///
    /// ## Examples
    ///
    /// ```
    /// use std::str::FromStr;
    /// use monocore::monofs::filesystem::dir::Utf8UnixPathSegment;
    ///
    /// let segment = Utf8UnixPathSegment::from_str("example").unwrap();
    ///
    /// assert_eq!(segment.len(), 7);
    /// ```
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns `true` if the segment is empty.
    ///
    /// ## Examples
    ///
    /// ```
    /// use std::str::FromStr;
    /// use monocore::monofs::filesystem::dir::Utf8UnixPathSegment;
    ///
    /// let segment = Utf8UnixPathSegment::from_str("example").unwrap();
    ///
    /// assert_eq!(segment.is_empty(), false);
    /// ```
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl FromStr for Utf8UnixPathSegment {
    type Err = FsError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Utf8UnixPathSegment::try_from(s)
    }
}

impl Display for Utf8UnixPathSegment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<&str> for Utf8UnixPathSegment {
    type Error = FsError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if value.is_empty() {
            return Err(FsError::InvalidPathComponent(value.to_string()));
        }

        let component = Utf8UnixComponent::try_from(value)
            .map_err(|_| FsError::InvalidPathComponent(value.to_string()))?;

        match component {
            Utf8UnixComponent::Normal(component) => Ok(Utf8UnixPathSegment(component.to_string())),
            _ => Err(FsError::InvalidPathComponent(value.to_string())),
        }
    }
}

impl<'a> TryFrom<&Utf8UnixComponent<'a>> for Utf8UnixPathSegment {
    type Error = FsError;

    fn try_from(component: &Utf8UnixComponent<'a>) -> Result<Self, Self::Error> {
        match component {
            Utf8UnixComponent::Normal(component) => Ok(Utf8UnixPathSegment(component.to_string())),
            _ => Err(FsError::InvalidPathComponent(component.to_string())),
        }
    }
}

impl<'a> From<&'a Utf8UnixPathSegment> for Utf8UnixComponent<'a> {
    fn from(segment: &'a Utf8UnixPathSegment) -> Self {
        Utf8UnixComponent::Normal(&segment.0)
    }
}

impl From<Utf8UnixPathSegment> for Utf8UnixPathBuf {
    #[inline]
    fn from(segment: Utf8UnixPathSegment) -> Self {
        Utf8UnixPathBuf::from(segment.0)
    }
}

impl AsRef<[u8]> for Utf8UnixPathSegment {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl AsRef<str> for Utf8UnixPathSegment {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<Utf8UnixPath> for Utf8UnixPathSegment {
    #[inline]
    fn as_ref(&self) -> &Utf8UnixPath {
        Utf8UnixPath::new(self)
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_as_str() {
        let segment = Utf8UnixPathSegment::from_str("example").unwrap();
        assert_eq!(segment.as_str(), "example");
    }

    #[test]
    fn test_as_bytes() {
        let segment = Utf8UnixPathSegment::from_str("example").unwrap();
        assert_eq!(segment.as_bytes(), b"example");
    }

    #[test]
    fn test_len() {
        let segment = Utf8UnixPathSegment::from_str("example").unwrap();
        assert_eq!(segment.len(), 7);
    }

    #[test]
    fn test_display() {
        let segment = Utf8UnixPathSegment::from_str("example").unwrap();
        assert_eq!(format!("{}", segment), "example");
    }

    #[test]
    fn test_try_from_str() {
        assert!(Utf8UnixPathSegment::try_from("example").is_ok());
        assert!(Utf8UnixPathSegment::from_str("example").is_ok());
        assert!("example".parse::<Utf8UnixPathSegment>().is_ok());

        // Negative cases
        assert!(Utf8UnixPathSegment::from_str("").is_err());
        assert!(Utf8UnixPathSegment::from_str(".").is_err());
        assert!(Utf8UnixPathSegment::from_str("..").is_err());
        assert!(Utf8UnixPathSegment::from_str("/").is_err());
        assert!(".".parse::<Utf8UnixPathSegment>().is_err());
        assert!("..".parse::<Utf8UnixPathSegment>().is_err());
        assert!("/".parse::<Utf8UnixPathSegment>().is_err());
        assert!("".parse::<Utf8UnixPathSegment>().is_err());
        assert!("///".parse::<Utf8UnixPathSegment>().is_err());
        assert!("...".parse::<Utf8UnixPathSegment>().is_err());
        assert!("\0".parse::<Utf8UnixPathSegment>().is_err());
        assert!("a/b".parse::<Utf8UnixPathSegment>().is_err());
        assert!(Utf8UnixPathSegment::try_from(".").is_err());
        assert!(Utf8UnixPathSegment::try_from("..").is_err());
        assert!(Utf8UnixPathSegment::try_from("/").is_err());
        assert!(Utf8UnixPathSegment::try_from("").is_err());
        assert!(Utf8UnixPathSegment::try_from("///").is_err());
        assert!(Utf8UnixPathSegment::try_from("...").is_err());
        assert!(Utf8UnixPathSegment::try_from("\0").is_err());
        assert!(Utf8UnixPathSegment::try_from("a/b").is_err());
    }

    #[test]
    fn test_utf8_characters() {
        assert!(Utf8UnixPathSegment::try_from("файл").is_ok());
        assert!(Utf8UnixPathSegment::try_from("文件").is_ok());
        assert!(Utf8UnixPathSegment::try_from("🚀").is_ok());

        // Negative cases
        assert!(Utf8UnixPathSegment::try_from("файл/имя").is_err());
        assert!(Utf8UnixPathSegment::try_from("文件/名称").is_err());
    }

    #[test]
    fn test_from_utf8_unix_path_segment_to_utf8_unix_component() {
        let segment = Utf8UnixPathSegment::from_str("example").unwrap();
        assert_eq!(
            Utf8UnixComponent::from(&segment),
            Utf8UnixComponent::Normal("example")
        );
    }

    #[test]
    fn test_from_utf8_unix_path_segment_to_utf8_unix_path_buf() {
        let segment = Utf8UnixPathSegment::from_str("example").unwrap();
        assert_eq!(
            Utf8UnixPathBuf::from(segment),
            Utf8UnixPathBuf::from("example")
        );
    }

    #[test]
    fn test_normal_with_special_characters() {
        assert!(Utf8UnixPathSegment::try_from("file.txt").is_ok());
        assert!(Utf8UnixPathSegment::try_from("file-name").is_ok());
        assert!(Utf8UnixPathSegment::try_from("file_name").is_ok());
        assert!(Utf8UnixPathSegment::try_from("file name").is_ok());

        // Negative cases
        assert!(Utf8UnixPathSegment::try_from("file/name").is_err());
        assert!(Utf8UnixPathSegment::try_from("file\\name").is_err());
        assert!(Utf8UnixPathSegment::try_from("file:name").is_err());
        assert!(Utf8UnixPathSegment::try_from("file*name").is_err());
        assert!(Utf8UnixPathSegment::try_from("file?name").is_err());
    }
}
