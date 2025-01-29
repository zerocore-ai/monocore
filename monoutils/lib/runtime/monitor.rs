use async_trait::async_trait;
use std::path::Path;
use tokio::process::ChildStderr;
use tokio::process::ChildStdout;

use crate::MonoutilsResult;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A trait for monitoring processes
#[async_trait]
pub trait ProcessMonitor {
    /// Start monitoring a process
    async fn start(
        &self,
        pid: u32,
        stdout: ChildStdout,
        stderr: ChildStderr,
        log_path: impl AsRef<Path> + Send + 'static,
    ) -> MonoutilsResult<()>;

    /// Stop monitoring
    async fn stop(&self) -> MonoutilsResult<()>;
}
