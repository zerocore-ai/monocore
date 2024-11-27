use std::{
    ffi::{c_char, CString},
    ops::{Bound, RangeBounds},
};

use crate::error::MonocoreResult;

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
/// - Trims leading/trailing whitespace
/// - Collapses multiple consecutive separators
///
/// ## Examples:
/// ```
/// use monocore::utils::sanitize_name_for_path;
///
/// assert_eq!(sanitize_name_for_path("library/alpine"), "library_alpine");
/// assert_eq!(sanitize_name_for_path("user/repo/name"), "user_repo_name");
/// assert_eq!(sanitize_name_for_path("my:weird@repo"), "my_weird_repo");
/// assert_eq!(sanitize_name_for_path(".hidden/repo."), ".hidden_repo.");
/// ```
pub fn sanitize_name_for_path(repo_name: &str) -> String {
    // First replace forward slashes with double underscore
    let with_safe_slashes = repo_name.replace('/', "__");

    // Replace other invalid chars with single underscore
    let sanitized = with_safe_slashes
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();

    // Trim leading/trailing whitespace
    let trimmed: &str = sanitized.trim_matches(|c: char| c.is_whitespace());

    // Collapse multiple consecutive separators
    trimmed
        .split('_')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("_")
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

/// Parses an OCI image reference (e.g. "ubuntu:latest" or "registry.example.com/org/image:tag")
/// into its components and generates a filesystem-safe name.
///
/// Returns (repository, tag, fs_safe_name) where:
/// - repository is the part before the last ':' (excluding registry port colons)
/// - tag is the part after the last ':' (defaults to "latest" if no tag specified)
/// - fs_safe_name is the sanitized filesystem name
///
/// Automatically qualifies unqualified image references with "library/", similar to Docker.
/// For example, "ubuntu" becomes "library/ubuntu".
///
/// ## Examples
/// ```
/// use monocore::utils::parse_image_ref;
///
/// assert_eq!(parse_image_ref("ubuntu:latest").unwrap(), ("library/ubuntu".to_string(), "latest".to_string(), "library_ubuntu__latest".to_string()));
/// assert_eq!(parse_image_ref("ubuntu").unwrap(), ("library/ubuntu".to_string(), "latest".to_string(), "library_ubuntu__latest".to_string()));
/// assert_eq!(
///     parse_image_ref("registry.com/org/image:1.0").unwrap(),
///     ("registry.com/org/image".to_string(), "1.0".to_string(), "registry.com_org_image__1.0".to_string())
/// );
/// ```
pub fn parse_image_ref(image_ref: &str) -> MonocoreResult<(String, String, String)> {
    // Split into parts to handle registry with port case
    let parts: Vec<&str> = image_ref.split('/').collect();

    // Find the last colon that's not part of registry:port
    let (repo, tag) = if parts.len() > 1 && parts[0].contains(':') {
        // Handle registry:port/path:tag case
        let registry_port = parts[0];
        let remainder = &image_ref[registry_port.len() + 1..];

        remainder
            .rsplit_once(':')
            .map(|(_, t)| {
                let repo = &image_ref[..image_ref.len() - t.len() - 1];
                (repo, t)
            })
            .unwrap_or((image_ref, "latest"))
    } else {
        // Handle normal case (no registry port)
        image_ref.rsplit_once(':').unwrap_or((image_ref, "latest"))
    };

    // If repo doesn't contain any slashes and doesn't start with a domain or localhost,
    // prepend "library/"
    let qualified_repo = if !repo.contains('/')
        && !repo.contains('.')  // No dots (not a domain)
        && !repo.starts_with("localhost")
    {
        format!("library/{}", repo)
    } else {
        repo.to_string()
    };

    let fs_name = format!(
        "{}__{}",
        sanitize_name_for_path(&qualified_repo),
        sanitize_name_for_path(tag)
    );

    Ok((qualified_repo, tag.to_string(), fs_name))
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_name_for_path() {
        // Test basic repository names
        assert_eq!(sanitize_name_for_path("library/alpine"), "library_alpine");
        assert_eq!(sanitize_name_for_path("user/repo/name"), "user_repo_name");

        // Test special characters
        assert_eq!(sanitize_name_for_path("my:weird@repo"), "my_weird_repo");
        assert_eq!(sanitize_name_for_path("repo#with$chars"), "repo_with_chars");
        assert_eq!(sanitize_name_for_path(".hidden/repo."), ".hidden_repo.");

        // Test leading/trailing whitespace
        assert_eq!(sanitize_name_for_path(" spaces /repo "), "spaces_repo");

        // Test multiple consecutive separators
        assert_eq!(
            sanitize_name_for_path("multiple___underscores"),
            "multiple_underscores"
        );
        assert_eq!(sanitize_name_for_path("weird////slashes"), "weird_slashes");

        // Test mixed cases
        assert_eq!(
            sanitize_name_for_path("my.weird/repo@with/special:chars"),
            "my.weird_repo_with_special_chars"
        );
    }

    #[test]
    fn test_format_mode() {
        assert_eq!(format_mode(0o755), "-rwxr-xr-x");
        assert_eq!(format_mode(0o644), "-rw-r--r--");
        assert_eq!(format_mode(0o40755), "drwxr-xr-x");
        assert_eq!(format_mode(0o100644), "-rw-r--r--");
        assert_eq!(format_mode(0o120777), "lrwxrwxrwx");
        assert_eq!(format_mode(0o010644), "prw-r--r--");
    }

    #[test]
    fn test_parse_image_ref() -> MonocoreResult<()> {
        // Test basic image references with library/ qualification
        assert_eq!(
            parse_image_ref("ubuntu:latest")?,
            (
                "library/ubuntu".to_string(),
                "latest".to_string(),
                "library_ubuntu__latest".to_string()
            )
        );
        assert_eq!(
            parse_image_ref("ubuntu")?,
            (
                "library/ubuntu".to_string(),
                "latest".to_string(),
                "library_ubuntu__latest".to_string()
            )
        );
        assert_eq!(
            parse_image_ref("nginx:1.19")?,
            (
                "library/nginx".to_string(),
                "1.19".to_string(),
                "library_nginx__1.19".to_string()
            )
        );

        // Test with registry and organization (should not add library/)
        let (repo, tag, fs_name) = parse_image_ref("registry.example.com/org/image")?;
        assert_eq!(repo, "registry.example.com/org/image");
        assert_eq!(tag, "latest");
        assert_eq!(fs_name, "registry.example.com_org_image__latest");

        // Test with registry port (should not add library/)
        let (repo, tag, fs_name) = parse_image_ref("registry:5000/org/image")?;
        assert_eq!(repo, "registry:5000/org/image");
        assert_eq!(tag, "latest");
        assert_eq!(fs_name, "registry_5000_org_image__latest");

        let (repo, tag, fs_name) = parse_image_ref("localhost:5000/my-image:latest")?;
        assert_eq!(repo, "localhost:5000/my-image");
        assert_eq!(tag, "latest");
        assert_eq!(fs_name, "localhost_5000_my-image__latest");

        // Test with unsafe characters
        let (repo, tag, fs_name) = parse_image_ref("image@sha256:abc123")?;
        assert_eq!(repo, "library/image@sha256");
        assert_eq!(tag, "abc123");
        assert_eq!(fs_name, "library_image_sha256__abc123");

        Ok(())
    }

    #[test]
    fn test_parse_image_ref_errors() -> MonocoreResult<()> {
        // Remove this test since we no longer error on missing tags
        Ok(())
    }
}
