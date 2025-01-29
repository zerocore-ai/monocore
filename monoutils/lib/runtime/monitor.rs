use async_trait::async_trait;
use std::path::PathBuf;
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
        &mut self,
        pid: u32,
        stdout: ChildStdout,
        stderr: ChildStderr,
        log_path: PathBuf,
    ) -> MonoutilsResult<()>;

    /// Stop monitoring
    async fn stop(&mut self) -> MonoutilsResult<()>;
}
