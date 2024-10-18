use std::{
    fmt::{self, Debug},
    iter::FromIterator,
    ops::{Deref, DerefMut},
};

use monoutils_store::IpldStore;

use crate::dir::{Dir, Utf8UnixPathSegment};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// `PathDirs` represents a path as a sequence of directory-name pairs. Each pair consists of a
/// `Dir` (representing the directory) and a `Utf8UnixPathSegment` (representing the name of the
/// next directory or file in the path).
///
/// For example, if the path is `/a/b/c`, `PathDirs` will contain:
/// 1. (Dir(/), PathSegment("a"))
/// 2. (Dir(/a), PathSegment("b"))
/// 3. (Dir(/a/b), PathSegment("c"))
#[derive(Clone)]
pub struct PathDirs<S>
where
    S: IpldStore,
{
    path: Vec<(Dir<S>, Utf8UnixPathSegment)>,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl<S> PathDirs<S>
where
    S: IpldStore,
{
    /// Creates a new empty `PathDirs`.
    pub fn new() -> Self {
        Self { path: vec![] }
    }

    /// Returns the number of segments in the path.
    pub fn len(&self) -> usize {
        self.path.len()
    }

    /// Returns whether the path is empty.
    pub fn is_empty(&self) -> bool {
        self.path.is_empty()
    }

    /// Returns an iterator over the path segments.
    pub fn iter(&self) -> impl Iterator<Item = &(Dir<S>, Utf8UnixPathSegment)> {
        self.path.iter()
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl<S> FromIterator<(Dir<S>, Utf8UnixPathSegment)> for PathDirs<S>
where
    S: IpldStore,
{
    fn from_iter<I: IntoIterator<Item = (Dir<S>, Utf8UnixPathSegment)>>(iter: I) -> Self {
        Self {
            path: iter.into_iter().collect(),
        }
    }
}

impl<S> IntoIterator for PathDirs<S>
where
    S: IpldStore,
{
    type Item = (Dir<S>, Utf8UnixPathSegment);
    type IntoIter = <Vec<(Dir<S>, Utf8UnixPathSegment)> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.path.into_iter()
    }
}

impl<S> Extend<(Dir<S>, Utf8UnixPathSegment)> for PathDirs<S>
where
    S: IpldStore,
{
    fn extend<T: IntoIterator<Item = (Dir<S>, Utf8UnixPathSegment)>>(&mut self, iter: T) {
        self.path.extend(iter);
    }
}

impl<S> Deref for PathDirs<S>
where
    S: IpldStore,
{
    type Target = Vec<(Dir<S>, Utf8UnixPathSegment)>;

    fn deref(&self) -> &Self::Target {
        &self.path
    }
}

impl<S> DerefMut for PathDirs<S>
where
    S: IpldStore,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.path
    }
}

impl<S> Default for PathDirs<S>
where
    S: IpldStore,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<S> Debug for PathDirs<S>
where
    S: IpldStore,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.path.iter()).finish()
    }
}

impl<S> PartialEq for PathDirs<S>
where
    S: IpldStore,
{
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
    }
}
