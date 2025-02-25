use async_trait::async_trait;
use tokio::{
    fs::File,
    io::unix::AsyncFd,
    process::{ChildStderr, ChildStdin, ChildStdout},
};

use crate::MonoutilsResult;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// The type of child IO to use.
pub enum ChildIo {
    /// A pseudo-TTY.
    TTY {
        /// The master read end of the pseudo-TTY.
        master_read: AsyncFd<std::fs::File>,

        /// The master write end of the pseudo-TTY.
        master_write: File,
    },

    /// Pipes for stdin, stdout, and stderr.
    Piped {
        /// The stdin pipe.
        stdin: Option<ChildStdin>,

        /// The stdout pipe.
        stdout: Option<ChildStdout>,

        /// The stderr pipe.
        stderr: Option<ChildStderr>,
    },
}

//--------------------------------------------------------------------------------------------------
// Traits
//--------------------------------------------------------------------------------------------------

/// A trait for monitoring processes
#[async_trait]
pub trait ProcessMonitor {
    /// Start monitoring a process
    async fn start(&mut self, pid: u32, name: String, child_io: ChildIo) -> MonoutilsResult<()>;

    /// Stop monitoring
    async fn stop(&mut self) -> MonoutilsResult<()>;
}
