use std::time::Duration;

use crate::config::DEFAULT_LOG_MAX_AGE;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Configuration for managing log file retention and cleanup in the orchestrator.
///
/// This configuration controls:
/// - How long log files are retained before being eligible for deletion
/// - Whether cleanup happens automatically during service lifecycle operations
#[derive(Debug, Clone)]
pub struct LogRetentionPolicy {
    /// Maximum age of log files before they are eligible for deletion.
    /// Files older than this duration will be removed during cleanup operations.
    pub(super) max_age: Duration,

    /// Whether to automatically clean up logs during service lifecycle operations (up/down).
    /// When true, old log files will be cleaned up during service start and stop operations.
    /// When false, cleanup must be triggered manually via `cleanup_old_logs()`.
    pub(super) auto_cleanup: bool,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl LogRetentionPolicy {
    /// Creates a new log retention policy with custom settings.
    pub fn new(max_age: Duration, auto_cleanup: bool) -> Self {
        Self {
            max_age,
            auto_cleanup,
        }
    }

    /// Creates a new policy that retains logs for the specified number of hours.
    pub fn with_max_age_hours(hours: u64) -> Self {
        Self {
            max_age: Duration::from_secs(hours * 60 * 60),
            auto_cleanup: true,
        }
    }

    /// Creates a new policy that retains logs for the specified number of days.
    pub fn with_max_age_days(days: u64) -> Self {
        Self {
            max_age: Duration::from_secs(days * 24 * 60 * 60),
            auto_cleanup: true,
        }
    }

    /// Creates a new policy that retains logs for the specified number of weeks.
    pub fn with_max_age_weeks(weeks: u64) -> Self {
        Self {
            max_age: Duration::from_secs(weeks * 7 * 24 * 60 * 60),
            auto_cleanup: true,
        }
    }

    /// Creates a new policy that retains logs for the specified number of months.
    /// Note: Uses a 30-day approximation for months.
    pub fn with_max_age_months(months: u64) -> Self {
        Self {
            max_age: Duration::from_secs(months * 30 * 24 * 60 * 60),
            auto_cleanup: true,
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl Default for LogRetentionPolicy {
    /// Creates a default configuration that:
    /// - Keeps logs for 7 days
    /// - Enables automatic cleanup during service operations
    fn default() -> Self {
        Self {
            max_age: DEFAULT_LOG_MAX_AGE,
            auto_cleanup: true,
        }
    }
}
