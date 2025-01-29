use async_trait::async_trait;

use crate::MonoutilsResult;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A trait for monitoring process metrics
#[async_trait]
pub trait MetricsMonitor {
    /// Register a process for metrics monitoring
    async fn register(&self, pid: u32) -> MonoutilsResult<()>;

    /// Start metrics monitoring
    async fn start(&self) -> MonoutilsResult<()>;

    /// Stop metrics monitoring
    async fn stop(&self) -> MonoutilsResult<()>;
}
