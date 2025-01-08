use tokio::fs;

use crate::{runtime::MicroVmState, utils::STATE_SUBDIR, MonocoreResult};

use super::{utils, Orchestrator, ServiceStatus};

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl Orchestrator {
    /// Retrieves the current status of all services, including their process IDs and state
    /// information. Also identifies and cleans up stale state files for processes that are
    /// no longer running.
    pub async fn status(&self) -> MonocoreResult<Vec<ServiceStatus>> {
        let mut statuses = Vec::new();
        let mut stale_files = Vec::new();
        let state_dir = self.home_dir.join(STATE_SUBDIR);

        // Ensure directory exists before reading
        if !fs::try_exists(&state_dir).await? {
            fs::create_dir_all(&state_dir).await?;
            return Ok(statuses);
        }

        // Read all state files from the state directory
        let mut dir = fs::read_dir(&state_dir).await?;
        while let Some(entry) = dir.next_entry().await? {
            let path = entry.path();
            if path.is_file() && path.extension().is_some_and(|ext| ext == "json") {
                match fs::read_to_string(&path).await {
                    Ok(contents) => match serde_json::from_str::<MicroVmState>(&contents) {
                        Ok(state) => {
                            // Check if the process is still running
                            if let Some(pid) = state.get_pid() {
                                if !utils::is_process_running(*pid).await {
                                    stale_files.push(path);
                                    continue;
                                }
                            }

                            statuses.push(ServiceStatus {
                                name: state.get_service().get_name().to_string(),
                                pid: *state.get_pid(),
                                state,
                            });
                        }
                        Err(e) => {
                            tracing::error!("Failed to parse state file {:?}: {}", path, e);
                            stale_files.push(path);
                        }
                    },
                    Err(e) => {
                        tracing::error!("Failed to read state file {:?}: {}", path, e);
                        stale_files.push(path);
                    }
                }
            }
        }

        // Clean up stale files
        for path in stale_files {
            if let Err(e) = fs::remove_file(&path).await {
                tracing::warn!("Failed to remove stale state file {:?}: {}", path, e);
            }
        }

        Ok(statuses)
    }
}
