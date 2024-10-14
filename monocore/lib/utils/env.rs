use std::env;

use crate::MonocoreResult;

use super::DEFAULT_MONOCORE_HOME;

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Checks if the environment variables are set and sets them if they are not.
pub fn check_or_set_env() -> MonocoreResult<()> {
    if env::var("MONOCORE_HOME").is_err() {
        tracing::warn!(
            "MONOCORE_HOME is not set, setting to default: {}",
            DEFAULT_MONOCORE_HOME.display().to_string()
        );
        unsafe {
            env::set_var("MONOCORE_HOME", DEFAULT_MONOCORE_HOME.display().to_string());
        }
    }

    Ok(())
}
