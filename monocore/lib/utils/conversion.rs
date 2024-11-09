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

/// Sanitizes a repository name for use in file paths by replacing invalid characters
/// with safe alternatives while maintaining readability and uniqueness.
///
/// ## Rules:
/// - Replaces '/' with '__' (double underscore to avoid collisions)
/// - Replaces other invalid path chars with '_'
/// - Trims leading/trailing whitespace and dots
/// - Collapses multiple consecutive separators
///
/// ## Examples:
/// ```
/// use monocore::utils::sanitize_repo_name;
///
/// assert_eq!(sanitize_repo_name("library/alpine"), "library__alpine");
/// assert_eq!(sanitize_repo_name("user/repo/name"), "user__repo__name");
/// assert_eq!(sanitize_repo_name("my:weird@repo"), "my_weird_repo");
/// assert_eq!(sanitize_repo_name(".hidden/repo."), "hidden__repo");
/// ```
pub fn sanitize_repo_name(repo_name: &str) -> String {
    // First replace forward slashes with double underscore
    let with_safe_slashes = repo_name.replace('/', "__");

    // Replace other invalid chars with single underscore
    let sanitized = with_safe_slashes
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();

    // Trim leading/trailing dots and whitespace
    let trimmed: &str = sanitized.trim_matches(|c: char| c == '.' || c.is_whitespace());

    // Collapse multiple consecutive separators
    trimmed
        .split('_')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_repo_name() {
        // Test basic repository names
        assert_eq!(sanitize_repo_name("library/alpine"), "library_alpine");
        assert_eq!(sanitize_repo_name("user/repo/name"), "user_repo_name");

        // Test special characters
        assert_eq!(sanitize_repo_name("my:weird@repo"), "my_weird_repo");
        assert_eq!(sanitize_repo_name("repo#with$chars"), "repo_with_chars");

        // Test leading/trailing characters
        assert_eq!(sanitize_repo_name(".hidden/repo."), "hidden_repo");
        assert_eq!(sanitize_repo_name(" spaces /repo "), "spaces_repo");

        // Test multiple consecutive separators
        assert_eq!(
            sanitize_repo_name("multiple___underscores"),
            "multiple_underscores"
        );
        assert_eq!(sanitize_repo_name("weird////slashes"), "weird_slashes");

        // Test mixed cases
        assert_eq!(
            sanitize_repo_name("my.weird/repo@with/special:chars"),
            "my_weird_repo_with_special_chars"
        );
    }
}
