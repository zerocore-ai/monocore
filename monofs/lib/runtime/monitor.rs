use std::io::Write;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use monoutils::{MonoutilsError, MonoutilsResult, ProcessMonitor, RotatingLog};
use sqlx::{Pool, Sqlite};
use tokio::io::AsyncReadExt;
use tokio::process::{ChildStderr, ChildStdout};

use crate::{management, FsResult};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A process monitor for the NFS server
pub struct NfsServerMonitor {
    /// The database for tracking metrics and metadata.
    database: Pool<Sqlite>,

    /// The supervisor PID
    supervisor_pid: u32,

    /// The mount directory
    mount_dir: PathBuf,

    /// The NFS server log path
    nfsserver_log_path: Option<PathBuf>,
}

impl NfsServerMonitor {
    /// Create a new NFS server monitor
    pub async fn new(
        database: impl AsRef<Path>,
        supervisor_pid: u32,
        mount_dir: PathBuf,
    ) -> FsResult<Self> {
        Ok(Self {
            database: management::get_fs_db_pool(database.as_ref()).await?,
            supervisor_pid,
            mount_dir,
            nfsserver_log_path: None,
        })
    }
}

#[async_trait]
impl ProcessMonitor for NfsServerMonitor {
    async fn start(
        &mut self,
        pid: u32,
        mut stdout: ChildStdout,
        mut stderr: ChildStderr,
        log_path: PathBuf,
    ) -> MonoutilsResult<()> {
        let nfs_server_log = RotatingLog::new(&log_path).await?;
        let mut stdout_writer = nfs_server_log.get_sync_writer();
        let mut stderr_writer = nfs_server_log.get_sync_writer();
        let nfs_server_pid = pid;

        self.nfsserver_log_path = Some(log_path);

        // Insert filesystem entry into database
        sqlx::query(
            r#"
            INSERT INTO filesystems (mount_dir, supervisor_pid, nfsserver_pid)
            VALUES (?, ?, ?)
            "#,
        )
        .bind(self.mount_dir.to_string_lossy().to_string())
        .bind(self.supervisor_pid)
        .bind(nfs_server_pid)
        .execute(&self.database)
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
                    tracing::error!(nfs_server_pid = nfs_server_pid, error = %e, "Failed to write to nfs server stdout log");
                }
                if let Err(e) = stdout_writer.flush() {
                    tracing::error!(nfs_server_pid = nfs_server_pid, error = %e, "Failed to flush nfs server stdout log");
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
                    tracing::error!(nfs_server_pid = nfs_server_pid, error = %e, "Failed to write to nfs server stderr log");
                }
                if let Err(e) = stderr_writer.flush() {
                    tracing::error!(nfs_server_pid = nfs_server_pid, error = %e, "Failed to flush nfs server stderr log");
                }
            }
        });

        Ok(())
    }

    async fn stop(&mut self) -> MonoutilsResult<()> {
        // Remove filesystem entry from database
        sqlx::query(
            r#"
            DELETE FROM filesystems
            WHERE mount_dir = ? AND supervisor_pid = ?
            "#,
        )
        .bind(self.mount_dir.to_string_lossy().to_string())
        .bind(self.supervisor_pid)
        .execute(&self.database)
        .await
        .map_err(MonoutilsError::custom)?;

        // Delete the log file if it exists
        if let Some(log_path) = &self.nfsserver_log_path {
            if let Err(e) = tokio::fs::remove_file(log_path).await {
                tracing::warn!(error = %e, "Failed to delete nfs server log file");
            }
        }

        // Reset the log path
        self.nfsserver_log_path = None;

        Ok(())
    }
}
