use std::path::{Path, PathBuf};

use async_trait::async_trait;
use monoutils::{MetricsMonitor, MonoutilsResult};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A basic metrics monitor implementation for monofs
pub struct MfsRuntimeMetricsMonitor {
    /// The database path
    _database: PathBuf,
}

impl MfsRuntimeMetricsMonitor {
    /// Create a new MonofsMetrics instance
    pub fn new(database: impl AsRef<Path>) -> Self {
        Self {
            _database: database.as_ref().to_path_buf(),
        }
    }
}

#[async_trait]
impl MetricsMonitor for MfsRuntimeMetricsMonitor {
    async fn register(&self, pid: u32) -> MonoutilsResult<()> {
        tracing::info!("Registering process {} for metrics monitoring", pid);
        Ok(())
    }

    async fn start(&self) -> MonoutilsResult<()> {
        tracing::info!("Starting metrics monitoring");
        Ok(())
    }

    async fn stop(&self) -> MonoutilsResult<()> {
        tracing::info!("Stopping metrics monitoring");
        Ok(())
    }
}
