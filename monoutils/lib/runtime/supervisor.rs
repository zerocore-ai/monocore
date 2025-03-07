use nix::unistd::Pid;
use std::{path::PathBuf, process::Stdio};
use tokio::{
    fs::create_dir_all,
    process::Command,
    signal::unix::{signal, SignalKind},
};

use crate::{path::SUPERVISOR_LOG_FILENAME, MonoutilsResult, ProcessMonitor, RotatingLog};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A supervisor that manages a child process and its logging.
pub struct Supervisor<M>
where
    M: ProcessMonitor + Send,
{
    /// Path to the child executable
    child_exe: PathBuf,

    /// Arguments to pass to the child executable
    child_args: Vec<String>,

    /// Name of the child process
    child_name: String,

    /// The managed child process ID
    child_pid: Option<u32>,

    /// Environment variables for the child process
    child_envs: Vec<(String, String)>,

    /// Path to the supervisor's log directory
    log_dir: PathBuf,

    /// The metrics monitor
    process_monitor: M,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl<M> Supervisor<M>
where
    M: ProcessMonitor + Send,
{
    /// Creates a new supervisor instance.
    ///
    /// ## Arguments
    ///
    /// * `child_exe` - Path to the child executable
    /// * `child_args` - Arguments to pass to the child executable
    /// * `child_name` - Name of the child process
    /// * `log_dir` - Path to the supervisor's log directory
    /// * `process_monitor` - The process monitor to use
    /// * `child_envs` - Environment variables for the child process
    pub fn new(
        child_exe: impl Into<PathBuf>,
        child_args: impl IntoIterator<Item = impl Into<String>>,
        child_envs: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
        child_name: impl Into<String>,
        log_dir: impl Into<PathBuf>,
        process_monitor: M,
    ) -> Self {
        Self {
            child_exe: child_exe.into(),
            child_args: child_args.into_iter().map(Into::into).collect(),
            child_envs: child_envs
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
            child_name: child_name.into(),
            child_pid: None,
            log_dir: log_dir.into(),
            process_monitor,
        }
    }

    /// Starts the supervisor and the child process.
    ///
    /// This method:
    /// 1. Creates the log directory if it doesn't exist
    /// 2. Starts the child process
    /// 3. Passes stdout/stderr to the process monitor
    pub async fn start(&mut self) -> MonoutilsResult<()> {
        // Create log directory if it doesn't exist
        create_dir_all(&self.log_dir).await?;

        // Setup supervisor's rotating log
        let _supervisor_log = RotatingLog::new(self.log_dir.join(SUPERVISOR_LOG_FILENAME)).await?;

        // Start child process
        let mut child = Command::new(&self.child_exe)
            .args(&self.child_args)
            .envs(self.child_envs.iter().map(|(k, v)| (k, v)))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let child_pid = child.id().expect("Failed to get child process ID");
        self.child_pid = Some(child_pid);

        // Take ownership of child's stdout/stderr and start monitoring
        let stdout = child.stdout.take().expect("Failed to take child stdout");
        let stderr = child.stderr.take().expect("Failed to take child stderr");
        self.process_monitor
            .start(child_pid, self.child_name.clone(), stdout, stderr)
            .await?;

        // Setup signal handlers
        let mut sigterm = signal(SignalKind::terminate())?;
        let mut sigint = signal(SignalKind::interrupt())?;

        // Wait for either child process to exit or signal to be received
        tokio::select! {
            status = child.wait() => {
                tracing::info!("Child process {} exited", child_pid);

                // Stop process monitoring
                self.process_monitor.stop().await?;

                if status.is_ok() {
                    if let Ok(status) = status {
                        if status.success() {
                            tracing::info!(
                                "Child process {} exited successfully",
                                child_pid
                            );
                        } else {
                            tracing::error!(
                                "Child process {} exited with status: {:?}",
                                child_pid,
                                status
                            );
                        }
                    }
                } else {
                    tracing::error!(
                        "Failed to wait for child process {}: {:?}",
                        child_pid,
                        status
                    );
                }
            }
            _ = sigterm.recv() => {
                tracing::info!("Received SIGTERM signal");

                // Stop process monitoring
                self.process_monitor.stop().await?;

                if let Some(pid) = self.child_pid.take() {
                    if let Err(e) = nix::sys::signal::kill(Pid::from_raw(pid as i32), nix::sys::signal::Signal::SIGTERM) {
                        tracing::error!(
                            "Failed to send SIGTERM to process {}: {}",
                            pid,
                            e
                        );
                    }
                }

                // Wait for child to exit after sending signal
                if let Err(e) = child.wait().await {
                    tracing::error!(
                        "Error waiting for child after SIGTERM: {}",
                        e
                    );
                }
            }
            _ = sigint.recv() => {
                tracing::info!("Received SIGINT signal");

                // Stop process monitoring
                self.process_monitor.stop().await?;

                if let Some(pid) = self.child_pid.take() {
                    if let Err(e) = nix::sys::signal::kill(Pid::from_raw(pid as i32), nix::sys::signal::Signal::SIGTERM) {
                        tracing::error!(
                            "Failed to send SIGTERM to process {}: {}",
                            pid,
                            e
                        );
                    }
                }

                // Wait for child to exit after sending signal
                if let Err(e) = child.wait().await {
                    tracing::error!(
                        "Error waiting for child after SIGINT: {}",
                        e
                    );
                }
            }
        }

        self.child_pid = None;
        Ok(())
    }
}
