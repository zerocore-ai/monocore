use std::path::Path;
use std::{io::Write, path::PathBuf};

use async_trait::async_trait;
use monoutils::{MonoutilsError, MonoutilsResult, ProcessMonitor, RotatingLog};
use sqlx::{Pool, Sqlite};
use tokio::io::AsyncReadExt;
use tokio::process::{ChildStderr, ChildStdout};

use crate::{management, MonocoreResult};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A process monitor for MicroVMs
pub struct MicroVmMonitor {
    /// The database for tracking sandbox metrics and metadata
    sandbox_db: Pool<Sqlite>,

    /// The supervisor PID
    supervisor_pid: u32,

    /// The MicroVM log path
    microvm_log_path: Option<PathBuf>,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl MicroVmMonitor {
    /// Create a new MicroVM monitor
    pub async fn new(database: impl AsRef<Path>, supervisor_pid: u32) -> MonocoreResult<Self> {
        Ok(Self {
            sandbox_db: management::get_sandbox_db_pool(database.as_ref()).await?,
            supervisor_pid,
            microvm_log_path: None,
        })
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

#[async_trait]
impl ProcessMonitor for MicroVmMonitor {
    async fn start(
        &mut self,
        pid: u32,
        mut stdout: ChildStdout,
        mut stderr: ChildStderr,
        log_path: PathBuf,
    ) -> MonoutilsResult<()> {
        let microvm_log = RotatingLog::new(&log_path).await?;
        let mut stdout_writer = microvm_log.get_sync_writer();
        let mut stderr_writer = microvm_log.get_sync_writer();
        let microvm_pid = pid;

        self.microvm_log_path = Some(log_path);

        // Insert sandbox entry into database
        sqlx::query(
            r#"
            INSERT INTO sandboxes (supervisor_pid, microvm_pid)
            VALUES (?, ?)
            "#,
        )
        .bind(self.supervisor_pid)
        .bind(microvm_pid)
        .execute(&self.sandbox_db)
        .await
        .map_err(MonoutilsError::custom)?;

        // Spawn tasks to handle stdout/stderr
        tokio::spawn(async move {
            let mut buf = [0u8; 1024];

            while let Ok(n) = stdout.read(&mut buf).await {
                if n == 0 {
                    break;
                }
                if let Err(e) = stdout_writer.write_all(&buf[..n]) {
                    tracing::error!(microvm_pid = microvm_pid, error = %e, "Failed to write to microvm stdout log");
                }
                if let Err(e) = stdout_writer.flush() {
                    tracing::error!(microvm_pid = microvm_pid, error = %e, "Failed to flush microvm stdout log");
                }
            }
        });

        tokio::spawn(async move {
            let mut buf = [0u8; 1024];

            while let Ok(n) = stderr.read(&mut buf).await {
                if n == 0 {
                    break;
                }
                if let Err(e) = stderr_writer.write_all(&buf[..n]) {
                    tracing::error!(microvm_pid = microvm_pid, error = %e, "Failed to write to microvm stderr log");
                }
                if let Err(e) = stderr_writer.flush() {
                    tracing::error!(microvm_pid = microvm_pid, error = %e, "Failed to flush microvm stderr log");
                }
            }
        });

        Ok(())
    }

    async fn stop(&mut self) -> MonoutilsResult<()> {
        // Remove sandbox entry from database
        sqlx::query(
            r#"
            DELETE FROM sandboxes
            WHERE supervisor_pid = ?
            "#,
        )
        .bind(self.supervisor_pid)
        .execute(&self.sandbox_db)
        .await
        .map_err(MonoutilsError::custom)?;

        // Delete the log file if it exists
        if let Some(log_path) = &self.microvm_log_path {
            if let Err(e) = tokio::fs::remove_file(log_path).await {
                tracing::warn!(error = %e, "Failed to delete microvm log file");
            }
        }

        Ok(())
    }
}
