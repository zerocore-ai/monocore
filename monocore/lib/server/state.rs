//! Server state management.
//!
//! This module provides the ServerState type which manages shared state for the
//! HTTP server, primarily the Orchestrator instance.

use std::{path::PathBuf, sync::Arc};
use tokio::sync::RwLock;

use crate::orchestration::Orchestrator;

//-------------------------------------------------------------------------------------------------
// Types
//-------------------------------------------------------------------------------------------------

/// Shared server state containing the orchestrator
///
/// This type provides thread-safe access to the Orchestrator instance through
/// an Arc<RwLock>. It is shared across all HTTP request handlers.
#[derive(Clone)]
pub struct ServerState {
    /// The shared orchestrator instance
    orchestrator: Arc<RwLock<Orchestrator>>,
}

impl ServerState {
    /// Creates a new ServerState instance
    ///
    /// # Arguments
    /// * `home_dir` - Home directory for monocore state
    /// * `supervisor_path` - Path to the supervisor executable
    ///
    /// # Returns
    /// A new ServerState instance wrapped in a Result
    pub async fn new(home_dir: PathBuf, supervisor_path: PathBuf) -> crate::MonocoreResult<Self> {
        // Try to load existing orchestrator state, fall back to creating new if loading fails
        let orchestrator = match Orchestrator::load(&home_dir, &supervisor_path).await {
            Ok(orchestrator) => {
                tracing::info!("Loaded existing orchestrator state");
                orchestrator
            }
            Err(e) => {
                tracing::info!("Creating new orchestrator: {}", e);
                Orchestrator::new(&home_dir, &supervisor_path).await?
            }
        };

        Ok(Self {
            orchestrator: Arc::new(RwLock::new(orchestrator)),
        })
    }

    /// Gets a reference to the orchestrator lock
    pub fn orchestrator(&self) -> &Arc<RwLock<Orchestrator>> {
        &self.orchestrator
    }
}
