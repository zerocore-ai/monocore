use std::{
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use monoutils::{
    ChildIo, MonoutilsError, MonoutilsResult, ProcessMonitor, RotatingLog, LOG_SUFFIX,
};
use sqlx::{Pool, Sqlite};
use tokio::io::AsyncReadExt;

use crate::{management, utils::MFSRUN_LOG_PREFIX, FsError, FsResult};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A process monitor for the NFS server
pub struct NfsServerMonitor {
    /// The database for tracking filesystem metrics and metadata.
    fs_db: Pool<Sqlite>,

    /// The name of the filesystem
    name: String,

    /// The supervisor PID
    supervisor_pid: u32,

    /// The mount directory
    mount_dir: PathBuf,

    /// The log directory
    log_dir: PathBuf,

    /// The log path
    log_path: Option<PathBuf>,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl NfsServerMonitor {
    /// Create a new NFS server monitor
    pub async fn new(
        supervisor_pid: u32,
        fs_db_path: impl AsRef<Path>,
        name: String,
        mount_dir: impl Into<PathBuf>,
        log_dir: impl Into<PathBuf>,
    ) -> FsResult<Self> {
        Ok(Self {
            fs_db: management::get_db_pool(fs_db_path.as_ref()).await?,
            name,
            supervisor_pid,
            mount_dir: mount_dir.into(),
            log_dir: log_dir.into(),
            log_path: None,
        })
    }

    /// Generates a unique log name using name, process ID, and current timestamp.
    ///
    /// The ID format is: "mfsrun-{name}-{timestamp}-{child_pid}.log"
    fn generate_log_name(&self, child_pid: u32) -> String {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        format!(
            "{}-{}-{}-{}.{}",
            MFSRUN_LOG_PREFIX, self.name, timestamp, child_pid, LOG_SUFFIX
        )
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

#[async_trait]
impl ProcessMonitor for NfsServerMonitor {
    async fn start(&mut self, pid: u32, child_io: ChildIo) -> MonoutilsResult<()> {
        let ChildIo::Piped { stdout, stderr, .. } = child_io else {
            return Err(MonoutilsError::custom(FsError::ChildIoMustBePiped));
        };

        // Setup child's log
        let log_name = self.generate_log_name(pid);
        let log_path = self.log_dir.join(&log_name);

        let nfs_server_log = RotatingLog::new(&log_path).await?;
        let mut stdout_writer = nfs_server_log.get_sync_writer();
        let mut stderr_writer = nfs_server_log.get_sync_writer();

        self.log_path = Some(log_path);

        // Insert filesystem entry into fs_db
        sqlx::query(
            r#"
            INSERT INTO filesystems (name, mount_dir, supervisor_pid, nfsserver_pid)
            VALUES (?, ?, ?, ?)
            "#,
        )
        .bind(&self.name)
        .bind(self.mount_dir.to_string_lossy().to_string())
        .bind(self.supervisor_pid)
        .bind(pid)
        .execute(&self.fs_db)
        .await
        .map_err(MonoutilsError::custom)?;

        // Spawn tasks to handle stdout/stderr
        if let Some(mut stdout) = stdout {
            tokio::spawn(async move {
                let mut buf = [0u8; 1024];

                while let Ok(n) = stdout.read(&mut buf).await {
                    if n == 0 {
                        break;
                    }
                    if let Err(e) = stdout_writer.write_all(&buf[..n]) {
                        tracing::error!(pid = pid, error = %e, "Failed to write to nfs server stdout log");
                    }
                    if let Err(e) = stdout_writer.flush() {
                        tracing::error!(pid = pid, error = %e, "Failed to flush nfs server stdout log");
                    }
                }
            });
        }

        if let Some(mut stderr) = stderr {
            tokio::spawn(async move {
                let mut buf = [0u8; 1024];

                while let Ok(n) = stderr.read(&mut buf).await {
                    if n == 0 {
                        break;
                    }
                    if let Err(e) = stderr_writer.write_all(&buf[..n]) {
                        tracing::error!(pid = pid, error = %e, "Failed to write to nfs server stderr log");
                    }
                    if let Err(e) = stderr_writer.flush() {
                        tracing::error!(pid = pid, error = %e, "Failed to flush nfs server stderr log");
                    }
                }
            });
        }

        Ok(())
    }

    async fn stop(&mut self) -> MonoutilsResult<()> {
        // Remove filesystem entry from fs_db
        sqlx::query(
            r#"
            DELETE FROM filesystems
            WHERE mount_dir = ? AND supervisor_pid = ?
            "#,
        )
        .bind(self.mount_dir.to_string_lossy().to_string())
        .bind(self.supervisor_pid)
        .execute(&self.fs_db)
        .await
        .map_err(MonoutilsError::custom)?;

        // Delete the log file if it exists
        if let Some(log_path) = &self.log_path {
            if let Err(e) = tokio::fs::remove_file(log_path).await {
                tracing::warn!(error = %e, "Failed to delete nfs server log file");
            }
        }

        // Reset the log path
        self.log_path = None;

        Ok(())
    }
}
