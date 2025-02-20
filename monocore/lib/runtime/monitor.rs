use std::{
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use monoutils::{MonoutilsError, MonoutilsResult, ProcessMonitor, RotatingLog, LOG_SUFFIX};
use sqlx::{Pool, Sqlite};
use tokio::{
    io::AsyncReadExt,
    process::{ChildStderr, ChildStdout},
};

use crate::{management, utils::MCRUN_LOG_PREFIX, MonocoreResult};

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
    log_path: Option<PathBuf>,

    /// The log directory
    log_dir: PathBuf,

    /// The root filesystem path
    root_path: PathBuf,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl MicroVmMonitor {
    /// Create a new MicroVM monitor
    pub async fn new(
        supervisor_pid: u32,
        sandbox_db_path: impl AsRef<Path>,
        log_dir: impl Into<PathBuf>,
        root_path: impl Into<PathBuf>,
    ) -> MonocoreResult<Self> {
        Ok(Self {
            supervisor_pid,
            sandbox_db: management::get_db_pool(sandbox_db_path.as_ref()).await?,
            log_path: None,
            log_dir: log_dir.into(),
            root_path: root_path.into(),
        })
    }

    /// Generates a unique log name using name, process ID, and current timestamp.
    ///
    /// The ID format is: "mcrun-{name}-{timestamp}-{child_pid}.log"
    fn generate_log_name(&self, child_pid: u32, name: impl AsRef<str>) -> String {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        format!(
            "{}-{}-{}-{}.{}",
            MCRUN_LOG_PREFIX,
            name.as_ref(),
            timestamp,
            child_pid,
            LOG_SUFFIX
        )
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
        name: String,
        mut stdout: ChildStdout,
        mut stderr: ChildStderr,
    ) -> MonoutilsResult<()> {
        let log_name = self.generate_log_name(pid, &name);
        let log_path = self.log_dir.join(&log_name);

        let microvm_log = RotatingLog::new(&log_path).await?;
        let mut stdout_writer = microvm_log.get_sync_writer();
        let mut stderr_writer = microvm_log.get_sync_writer();
        let microvm_pid = pid;

        self.log_path = Some(log_path);

        // Insert sandbox entry into database
        sqlx::query(
            r#"
            INSERT INTO sandboxes (name, supervisor_pid, microvm_pid, status, root_path)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(name)
        .bind(self.supervisor_pid)
        .bind(microvm_pid)
        .bind("STARTING")
        .bind(self.root_path.to_string_lossy().into_owned())
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

        // TODO:  We might need a better strategy for cleaning up the log files
        // // Delete the log file if it exists
        // if let Some(log_path) = &self.log_path {
        //     if let Err(e) = tokio::fs::remove_file(log_path).await {
        //         tracing::warn!(error = %e, "Failed to delete microvm log file");
        //     }
        // }

        // Reset the log path
        self.log_path = None;

        Ok(())
    }
}
