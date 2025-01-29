use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;
use tokio::process::Command;
use std::process::Stdio;
use std::{
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::{fs::create_dir_all, io::AsyncReadExt};

use crate::path::{LOG_SUFFIX, SUPERVISOR_LOG_FILENAME};
use crate::{MetricsMonitor, MonoutilsResult, RotatingLog};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A supervisor that manages a child process and its logging.
pub struct Supervisor<M>
where
    M: MetricsMonitor,
{
    /// Path to the child executable
    child_exe: PathBuf,

    /// Arguments to pass to the child executable
    child_args: Vec<String>,

    /// Name of the child process
    child_name: String,

    /// Prefix for the child's log file
    child_log_prefix: String,

    /// Path to the supervisor's log directory
    log_dir: PathBuf,

    /// The metrics monitor
    metrics_monitor: M,

    /// The managed child process ID
    child_pid: Option<u32>,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl<M> Supervisor<M>
where
    M: MetricsMonitor,
{
    /// Creates a new supervisor instance.
    ///
    /// ## Arguments
    ///
    /// * `child_exe` - Path to the child executable
    /// * `child_args` - Arguments to pass to the child executable
    /// * `child_name` - Name of the child process
    /// * `log_dir` - Path to the supervisor's log directory
    pub fn new(
        child_exe: impl AsRef<Path>,
        child_args: impl IntoIterator<Item = impl Into<String>>,
        child_name: impl Into<String>,
        child_log_prefix: impl Into<String>,
        log_dir: impl AsRef<Path>,
        metrics_monitor: M,
    ) -> Self {
        Self {
            child_exe: child_exe.as_ref().to_path_buf(),
            child_args: child_args.into_iter().map(Into::into).collect(),
            child_name: child_name.into(),
            child_log_prefix: child_log_prefix.into(),
            log_dir: log_dir.as_ref().to_path_buf(),
            metrics_monitor,
            child_pid: None,
        }
    }

    /// Generates a unique child ID using name, process ID, and current timestamp.
    ///
    /// The ID format is: "{name}-{pid}-{timestamp}"
    fn generate_child_id(&self, child_pid: u32) -> String {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        format!("{}-{}-{}", self.child_name, child_pid, timestamp)
    }

    /// Starts the supervisor and the child process.
    ///
    /// This method:
    /// 1. Sets up the supervisor's rotating log
    /// 2. Starts the child process
    /// 3. Sets up the child's rotating log for stdout/stderr
    pub async fn start(&mut self) -> MonoutilsResult<()> {
        // Create log directory if it doesn't exist
        create_dir_all(&self.log_dir).await?;

        // Setup supervisor's rotating log
        let _supervisor_log = RotatingLog::new(self.log_dir.join(SUPERVISOR_LOG_FILENAME)).await?;

        // Start child process
        let mut child = Command::new(&self.child_exe)
            .args(&self.child_args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let child_pid = child.id().expect("Failed to get child process ID");
        self.child_pid = Some(child_pid);

        // Register child process with metrics monitor
        self.metrics_monitor.register(child_pid).await?;

        // Generate unique child ID
        let child_id = self.generate_child_id(child_pid);

        // Setup child's rotating log
        let child_log_name = format!("{}-{}.{}", self.child_log_prefix, child_id, LOG_SUFFIX);
        let child_log = RotatingLog::new(self.log_dir.join(child_log_name)).await?;

        // Get sync writers for child stdout/stderr
        let mut stdout_writer = child_log.get_sync_writer();
        let mut stderr_writer = child_log.get_sync_writer();

        // Take ownership of child's stdout/stderr
        let mut stdout = child.stdout.take().expect("Failed to take child stdout");
        let mut stderr = child.stderr.take().expect("Failed to take child stderr");

        // Start metrics monitoring
        self.metrics_monitor.start().await?;

        // Spawn tasks to handle stdout/stderr
        tokio::spawn(async move {
            let mut buf = [0u8; 1024];

            while let Ok(n) = stdout.read(&mut buf).await {
                if n == 0 {
                    break;
                }
                if let Err(e) = stdout_writer.write_all(&buf[..n]) {
                    tracing::error!(child_pid = child_pid, error = %e, "Failed to write to child stdout log");
                }
                if let Err(e) = stdout_writer.flush() {
                    tracing::error!(child_pid = child_pid, error = %e, "Failed to flush child stdout log");
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
                    tracing::error!(child_pid = child_pid, error = %e, "Failed to write to child stderr log");
                }
                if let Err(e) = stderr_writer.flush() {
                    tracing::error!(child_pid = child_pid, error = %e, "Failed to flush child stderr log");
                }
            }
        });

        // Wait for child process to exit
        match child.wait().await {
            Ok(status) => {
                // Stop metrics monitoring
                self.metrics_monitor.stop().await?;

                if status.success() {
                    println!("Child process {} exited successfully", child_pid);
                } else {
                    eprintln!(
                        "Child process {} exited with status: {:?}",
                        child_pid, status
                    );
                }

                self.child_pid = None;
            }
            Err(e) => eprintln!("Failed to wait for child process {}: {}", child_pid, e),
        }

        Ok(())
    }

    /// Stops the supervisor and child process.
    pub async fn stop(&mut self) -> MonoutilsResult<()> {
        // Stop child process if it exists
        if let Some(pid) = self.child_pid.take() {
            // Send SIGTERM signal
            if let Err(e) = signal::kill(Pid::from_raw(pid as i32), Signal::SIGTERM) {
                eprintln!("Failed to send SIGTERM to process {}: {}", pid, e);
            }
        }

        Ok(())
    }
}
