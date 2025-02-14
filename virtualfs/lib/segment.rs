use std::{
    ffi::{OsStr, OsString},
    fmt::{self, Display},
    path::{Component, Path, PathBuf},
    str::FromStr,
};

use crate::VfsError;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Represents a single segment of a path.
///
/// This struct provides a way to represent and manipulate individual components
/// of a path, ensuring they are valid path segments.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PathSegment(OsString);

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl PathSegment {
    /// Returns the OS string representation of the segment.
    pub fn as_os_str(&self) -> &OsStr {
        &self.0
    }

    /// Returns the bytes representation of the segment.
    pub fn as_bytes(&self) -> &[u8] {
        self.as_os_str().as_encoded_bytes()
    }

    /// Returns the length of the segment in bytes.
    pub fn len(&self) -> usize {
        self.as_bytes().len()
    }

    /// Returns `true` if the segment is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl FromStr for PathSegment {
    type Err = VfsError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        PathSegment::try_from(s)
    }
}

impl Display for PathSegment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.to_string_lossy())
    }
}

impl TryFrom<&str> for PathSegment {
    type Error = VfsError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if value.is_empty() {
            return Err(VfsError::EmptyPathSegment);
        }

        #[cfg(unix)]
        {
            if value.contains('/') {
                return Err(VfsError::InvalidPathComponent(value.to_string()));
            }
        }

        #[cfg(windows)]
        {
            if value.contains('/') || value.contains('\\') {
                return Err(VfsError::InvalidPathComponent(value.to_string()));
            }
        }

        // At this point the string does not contain any separator characters
        let mut components = Path::new(value).components();
        let component = components
            .next()
            .ok_or_else(|| VfsError::InvalidPathComponent(value.to_string()))?;

        // Ensure there are no additional components
        if components.next().is_some() {
            return Err(VfsError::InvalidPathComponent(value.to_string()));
        }

        match component {
            Component::Normal(comp) => Ok(PathSegment(comp.to_os_string())),
            _ => Err(VfsError::InvalidPathComponent(value.to_string())),
        }
    }
}

impl<'a> TryFrom<Component<'a>> for PathSegment {
    type Error = VfsError;

    fn try_from(component: Component<'a>) -> Result<Self, Self::Error> {
        PathSegment::try_from(&component)
    }
}

impl<'a> TryFrom<&Component<'a>> for PathSegment {
    type Error = VfsError;

    fn try_from(component: &Component<'a>) -> Result<Self, Self::Error> {
        match component {
            Component::Normal(component) => Ok(PathSegment(component.to_os_string())),
            _ => Err(VfsError::InvalidPathComponent(
                component.as_os_str().to_string_lossy().into_owned(),
            )),
        }
    }
}

impl<'a> From<&'a PathSegment> for Component<'a> {
    fn from(segment: &'a PathSegment) -> Self {
        Component::Normal(segment.as_os_str())
    }
}

impl From<PathSegment> for PathBuf {
    #[inline]
    fn from(segment: PathSegment) -> Self {
        PathBuf::from(segment.0)
    }
}

impl AsRef<[u8]> for PathSegment {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl AsRef<OsStr> for PathSegment {
    #[inline]
    fn as_ref(&self) -> &OsStr {
        self.as_os_str()
    }
}

impl AsRef<Path> for PathSegment {
    #[inline]
    fn as_ref(&self) -> &Path {
        Path::new(self.as_os_str())
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_segment_as_os_str() {
        let segment = PathSegment::from_str("example").unwrap();
        assert_eq!(segment.as_os_str(), OsStr::new("example"));
    }

    #[test]
    fn test_segment_as_bytes() {
        let segment = PathSegment::from_str("example").unwrap();
        assert_eq!(segment.as_bytes(), b"example");
    }

    #[test]
    fn test_segment_len() {
        let segment = PathSegment::from_str("example").unwrap();
        assert_eq!(segment.len(), 7);
    }

    #[test]
    fn test_segment_display() {
        let segment = PathSegment::from_str("example").unwrap();
        assert_eq!(format!("{}", segment), "example");
    }

    #[test]
    fn test_segment_try_from_str() {
        assert!(PathSegment::try_from("example").is_ok());
        assert!(PathSegment::from_str("example").is_ok());
        assert!("example".parse::<PathSegment>().is_ok());

        // Negative cases
        assert!(PathSegment::from_str("").is_err());
        assert!(PathSegment::from_str(".").is_err());
        assert!(PathSegment::from_str("..").is_err());
        assert!(".".parse::<PathSegment>().is_err());
        assert!("..".parse::<PathSegment>().is_err());
        assert!("".parse::<PathSegment>().is_err());
        assert!(PathSegment::try_from(".").is_err());
        assert!(PathSegment::try_from("..").is_err());
        assert!(PathSegment::try_from("/").is_err());
        assert!(PathSegment::try_from("").is_err());
    }

    #[test]
    fn test_segment_from_path_segment_to_component() {
        let segment = PathSegment::from_str("example").unwrap();
        assert_eq!(
            Component::from(&segment),
            Component::Normal(OsStr::new("example"))
        );
    }

    #[test]
    fn test_segment_from_path_segment_to_path_buf() {
        let segment = PathSegment::from_str("example").unwrap();
        assert_eq!(PathBuf::from(segment), PathBuf::from("example"));
    }

    #[test]
    fn test_segment_normal_with_special_characters() {
        assert!(PathSegment::try_from("file.txt").is_ok());
        assert!(PathSegment::try_from("file-name").is_ok());
        assert!(PathSegment::try_from("file_name").is_ok());
        assert!(PathSegment::try_from("file name").is_ok());
        assert!(PathSegment::try_from("file:name").is_ok());
        assert!(PathSegment::try_from("file*name").is_ok());
        assert!(PathSegment::try_from("file?name").is_ok());
    }

    #[test]
    #[cfg(unix)]
    fn test_segment_with_unix_separator() {
        // On Unix systems, forward slash is the main separator
        assert!(PathSegment::try_from("file/name").is_err());
        assert!(PathSegment::try_from("/").is_err());
        assert!(PathSegment::try_from("///").is_err());
        assert!(PathSegment::try_from("name/").is_err());
        assert!(PathSegment::try_from("/name").is_err());
    }

    #[test]
    #[cfg(windows)]
    fn test_segment_with_windows_separators() {
        // On Windows, both forward slash and backslash are separators
        assert!(PathSegment::try_from("file\\name").is_err());
        assert!(PathSegment::try_from("file/name").is_err());
        assert!(PathSegment::try_from("\\").is_err());
        assert!(PathSegment::try_from("/").is_err());
        assert!(PathSegment::try_from("\\\\\\").is_err());
        assert!(PathSegment::try_from("///").is_err());
        assert!(PathSegment::try_from("name\\").is_err());
        assert!(PathSegment::try_from("name/").is_err());
        assert!(PathSegment::try_from("\\name").is_err());
        assert!(PathSegment::try_from("/name").is_err());
    }
}
