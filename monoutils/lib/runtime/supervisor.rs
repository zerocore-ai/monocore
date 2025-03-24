use nix::{
    fcntl::{fcntl, FcntlArg, OFlag},
    pty::openpty,
    unistd::Pid,
};
use std::{
    os::unix::io::{AsRawFd, FromRawFd, IntoRawFd},
    path::PathBuf,
    process::Stdio,
};
use tokio::{
    fs::{create_dir_all, File},
    io::unix::AsyncFd,
    process::Command,
    signal::unix::{signal, SignalKind},
};

use crate::{
    path::SUPERVISOR_LOG_FILENAME, term, ChildIo, MonoutilsResult, ProcessMonitor, RotatingLog,
};

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
    /// * `log_dir` - Path to the supervisor's log directory
    /// * `process_monitor` - The process monitor to use
    /// * `child_envs` - Environment variables for the child process
    pub fn new(
        child_exe: impl Into<PathBuf>,
        child_args: impl IntoIterator<Item = impl Into<String>>,
        child_envs: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
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
            child_pid: None,
            log_dir: log_dir.into(),
            process_monitor,
        }
    }

    /// Starts the supervisor and the child process.
    ///
    /// This method:
    /// 1. Creates the log directory if it doesn't exist
    /// 2. Starts the child process with appropriate IO (TTY or pipes)
    /// 3. Passes the IO to the process monitor
    pub async fn start(&mut self) -> MonoutilsResult<()> {
        // Create log directory if it doesn't exist
        create_dir_all(&self.log_dir).await?;

        // Setup supervisor's rotating log
        let _supervisor_log = RotatingLog::new(self.log_dir.join(SUPERVISOR_LOG_FILENAME)).await?;

        // Check if we're running in an interactive terminal
        let (mut child, child_io) = if term::is_interactive_terminal() {
            tracing::info!("running in an interactive terminal");
            // Create a new pseudo terminal and set master to non-blocking mode
            let pty = openpty(None, None)?;
            let master_fd = pty.master.as_raw_fd();
            {
                let flags = OFlag::from_bits_truncate(fcntl(master_fd, FcntlArg::F_GETFL)?);
                let new_flags = flags | OFlag::O_NONBLOCK;
                fcntl(master_fd, FcntlArg::F_SETFL(new_flags))?;
            }

            // Clone the slave for stdin, stdout, and stderr
            let slave_in = pty.slave.try_clone()?;
            let slave_out = pty.slave.try_clone()?;
            let slave_err = pty.slave;

            // Start child process with PTY
            let mut command = Command::new(&self.child_exe);
            command
                .args(&self.child_args)
                .envs(self.child_envs.iter().map(|(k, v)| (k, v)))
                .stdin(Stdio::from(slave_in))
                .stdout(Stdio::from(slave_out))
                .stderr(Stdio::from(slave_err));

            // Set up child's session and controlling terminal
            unsafe {
                command.pre_exec(|| {
                    nix::unistd::setsid()
                        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
                    if libc::ioctl(libc::STDIN_FILENO, libc::TIOCSCTTY as _, 1 as libc::c_long) < 0
                    {
                        return Err(std::io::Error::last_os_error());
                    }
                    Ok(())
                });
            }

            let child = command.spawn()?;

            // Set up master file handles for asynchronous I/O
            let master_fd_owned = pty.master;
            let master_write_fd = nix::unistd::dup(master_fd_owned.as_raw_fd())?;
            let master_read_file =
                unsafe { std::fs::File::from_raw_fd(master_fd_owned.into_raw_fd()) };
            let master_write_file = unsafe { std::fs::File::from_raw_fd(master_write_fd) };

            let master_read = AsyncFd::new(master_read_file)?;
            let master_write = File::from_std(master_write_file);

            // Create the TTY ChildIO
            let child_io = ChildIo::TTY {
                master_read,
                master_write,
            };

            (child, child_io)
        } else {
            tracing::info!("running in a non-interactive terminal");
            // Start child process with pipes
            let mut child = Command::new(&self.child_exe)
                .args(&self.child_args)
                .envs(self.child_envs.iter().map(|(k, v)| (k, v)))
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?;

            // Take ownership of child's stdin/stdout/stderr
            let stdin = child.stdin.take();
            let stdout = child.stdout.take();
            let stderr = child.stderr.take();

            // Create the Piped ChildIO enum
            let child_io = ChildIo::Piped {
                stdin,
                stdout,
                stderr,
            };

            (child, child_io)
        };

        let child_pid = child.id().expect("failed to get child process id");
        self.child_pid = Some(child_pid);

        // Start monitoring
        self.process_monitor.start(child_pid, child_io).await?;

        // Setup signal handlers
        let mut sigterm = signal(SignalKind::terminate())?;
        let mut sigint = signal(SignalKind::interrupt())?;

        // Wait for either child process to exit or signal to be received
        tokio::select! {
            status = child.wait() => {
                // Stop process monitoring
                self.process_monitor.stop().await?;

                tracing::info!("child process {} exited", child_pid);

                if status.is_ok() {
                    if let Ok(status) = status {
                        if status.success() {
                            tracing::info!(
                                "child process {} exited successfully",
                                child_pid
                            );
                        } else {
                            tracing::error!(
                                "child process {} exited with status: {:?}",
                                child_pid,
                                status
                            );
                        }
                    }
                } else {
                    tracing::error!(
                        "failed to wait for child process {}: {:?}",
                        child_pid,
                        status
                    );
                }
            }
            _ = sigterm.recv() => {
                // Stop process monitoring
                self.process_monitor.stop().await?;

                tracing::info!("received SIGTERM signal");

                if let Some(pid) = self.child_pid.take() {
                    if let Err(e) = nix::sys::signal::kill(Pid::from_raw(pid as i32), nix::sys::signal::Signal::SIGTERM) {
                        tracing::error!(
                            "failed to send SIGTERM to process {}: {}",
                            pid,
                            e
                        );
                    }
                }

                // Wait for child to exit after sending signal
                if let Err(e) = child.wait().await {
                    tracing::error!(
                        "error waiting for child after SIGTERM: {}",
                        e
                    );
                }
            }
            _ = sigint.recv() => {
                // Stop process monitoring
                self.process_monitor.stop().await?;

                tracing::info!("received SIGINT signal");

                if let Some(pid) = self.child_pid.take() {
                    if let Err(e) = nix::sys::signal::kill(Pid::from_raw(pid as i32), nix::sys::signal::Signal::SIGTERM) {
                        tracing::error!(
                            "failed to send SIGTERM to process {}: {}",
                            pid,
                            e
                        );
                    }
                }

                // Wait for child to exit after sending signal
                if let Err(e) = child.wait().await {
                    tracing::error!(
                        "error waiting for child after SIGINT: {}",
                        e
                    );
                }
            }
        }

        self.child_pid = None;

        Ok(())
    }
}
