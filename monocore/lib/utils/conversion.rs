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
/// # Arguments
///
/// * `strings` - A slice of strings to convert
///
/// # Returns
///
/// A vector of pointers to null-terminated C strings, with a null pointer appended at the end.
///
/// # Safety
///
/// The returned vector must be kept alive as long as the pointers are in use.
pub fn to_null_terminated_c_array(strings: &[CString]) -> Vec<*const c_char> {
    let mut ptrs: Vec<*const c_char> = strings.iter().map(|s| s.as_ptr()).collect();
    ptrs.push(std::ptr::null());

    ptrs
}
