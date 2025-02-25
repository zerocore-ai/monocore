//! Module containing terminal utilities

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Determines if the process is running in an interactive terminal environment
pub fn is_interactive_terminal() -> bool {
    // Check if stdin and stdout are TTYs
    let stdin_is_tty = unsafe { libc::isatty(libc::STDIN_FILENO) == 1 };
    let stdout_is_tty = unsafe { libc::isatty(libc::STDOUT_FILENO) == 1 };

    // Base check: both stdin and stdout must be TTYs
    let is_tty = stdin_is_tty && stdout_is_tty;

    // Optional enhancement: check for TERM, but don't require it
    let has_term = std::env::var("TERM").is_ok();

    // Log the detection for debugging
    if is_tty && !has_term {
        tracing::debug!("detected TTY without TERM environment variable");
    }

    // Return true if we have TTYs, regardless of TERM
    is_tty
}
