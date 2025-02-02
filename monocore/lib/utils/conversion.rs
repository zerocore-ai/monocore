//! Utility functions for converting between different data types.

use std::{
    ffi::{c_char, CString},
    ops::{Bound, RangeBounds},
};

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Converts a range bound to a u64 start and end value.
///
/// ## Examples
///
/// ```
/// use monocore::utils::convert_bounds;
///
/// let (start, end) = convert_bounds(1..10);
/// assert_eq!(start, 1);
/// assert_eq!(end, 9);
///
/// let (start, end) = convert_bounds(..10);
/// assert_eq!(start, 0);
/// assert_eq!(end, 9);
///
/// let (start, end) = convert_bounds(1..);
/// assert_eq!(start, 1);
/// assert_eq!(end, u64::MAX);
///
/// let (start, end) = convert_bounds(..=10);
/// assert_eq!(start, 0);
/// assert_eq!(end, 10);
/// ```
pub fn convert_bounds(range: impl RangeBounds<u64>) -> (u64, u64) {
    let start = match range.start_bound() {
        Bound::Included(&start) => start,
        Bound::Excluded(&start) => start + 1,
        Bound::Unbounded => 0,
    };

    let end = match range.end_bound() {
        Bound::Included(&end) => end,
        Bound::Excluded(&end) => end - 1,
        Bound::Unbounded => u64::MAX,
    };

    (start, end)
}

/// Creates a null-terminated array of pointers from a slice of strings.
///
/// This function is useful for FFI calls that expect a null-terminated array of C-style strings.
///
/// ## Arguments
///
/// * `strings` - A slice of strings to convert
///
/// ## Returns
///
/// A vector of pointers to null-terminated C strings, with a null pointer appended at the end.
///
/// ## Safety
///
/// The returned vector must be kept alive as long as the pointers are in use.
pub fn to_null_terminated_c_array(strings: &[CString]) -> Vec<*const c_char> {
    let mut ptrs: Vec<*const c_char> = strings.iter().map(|s| s.as_ptr()).collect();
    ptrs.push(std::ptr::null());

    ptrs
}

/// Converts a file mode to a string representation similar to ls -l
///
/// # Examples
/// ```
/// use monocore::utils::format_mode;
/// assert_eq!(format_mode(0o755), "-rwxr-xr-x");
/// assert_eq!(format_mode(0o644), "-rw-r--r--");
/// assert_eq!(format_mode(0o40755), "drwxr-xr-x");
/// ```
pub fn format_mode(mode: u32) -> String {
    let file_type = match mode & 0o170000 {
        0o040000 => 'd', // directory
        0o120000 => 'l', // symbolic link
        0o010000 => 'p', // named pipe (FIFO)
        0o140000 => 's', // socket
        0o060000 => 'b', // block device
        0o020000 => 'c', // character device
        _ => '-',        // regular file
    };

    let user = format_triplet((mode >> 6) & 0o7);
    let group = format_triplet((mode >> 3) & 0o7);
    let other = format_triplet(mode & 0o7);

    format!("{}{}{}{}", file_type, user, group, other)
}

/// Helper function to convert a permission triplet (3 bits) to rwx format
fn format_triplet(mode: u32) -> String {
    let r = if mode & 0o4 != 0 { 'r' } else { '-' };
    let w = if mode & 0o2 != 0 { 'w' } else { '-' };
    let x = if mode & 0o1 != 0 { 'x' } else { '-' };
    format!("{}{}{}", r, w, x)
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_mode() {
        assert_eq!(format_mode(0o755), "-rwxr-xr-x");
        assert_eq!(format_mode(0o644), "-rw-r--r--");
        assert_eq!(format_mode(0o40755), "drwxr-xr-x");
        assert_eq!(format_mode(0o100644), "-rw-r--r--");
        assert_eq!(format_mode(0o120777), "lrwxrwxrwx");
        assert_eq!(format_mode(0o010644), "prw-r--r--");
    }
}
