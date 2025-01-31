use async_trait::async_trait;
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
        name: String,
        stdout: ChildStdout,
        stderr: ChildStderr,
    ) -> MonoutilsResult<()>;

    /// Stop monitoring
    async fn stop(&mut self) -> MonoutilsResult<()>;
}
