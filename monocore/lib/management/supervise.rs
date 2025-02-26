use std::{
    env,
    net::IpAddr,
    os::fd::{AsRawFd, FromRawFd, IntoRawFd},
    path::{Path, PathBuf},
    process::Stdio,
};

use monoutils::{term, ChildIo};
use nix::{
    fcntl::{fcntl, FcntlArg, OFlag},
    pty::openpty,
    unistd::Pid,
};
use tokio::{
    fs::File,
    io::unix::AsyncFd,
    process::{Child, Command},
    signal::unix::{signal, SignalKind},
};
use virtualfs::{DEFAULT_NFS_HOST, DEFAULT_NFS_PORT};

use crate::MonocoreResult;

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

pub async fn start_supervision(
    log_dir: impl Into<PathBuf>,
    child_name: impl AsRef<Path>,
    sandbox_db_path: impl AsRef<Path>,
    forward_output: bool,
    overlayfs_layer_paths: Vec<PathBuf>,
    nfs_host: Option<IpAddr>,
    nfs_port: Option<u16>,
    root_path: PathBuf,
    num_vcpus: u8,
    ram_mib: u32,
    workdir_path: String,
    exec_path: String,
    env: Vec<String>,
    mapped_dirs: Vec<String>,
    port_map: Vec<String>,
    log_level: Option<u8>,
    args: Vec<String>,
) -> MonocoreResult<()> {
    tracing::info!("setting up supervisor");
    let microvm_exe = env::current_exe()?;
    let supervisor_pid = std::process::id();

    let overlayfs_child = if !overlayfs_layer_paths.is_empty() {
        let nfs_host = nfs_host.unwrap_or_else(|| DEFAULT_NFS_HOST.parse().unwrap());
        let nfs_port = nfs_port.unwrap_or(DEFAULT_NFS_PORT);
        let overlayfs_args = compose_overlayfs_args(overlayfs_layer_paths, nfs_host, nfs_port);
        let overlayfs_envs = compose_child_envs();
        let overlayfs_child =
            bootstrap_overlayfs(overlayfs_exe, overlayfs_args, overlayfs_envs).await?;
        Some(overlayfs_child)
    } else {
        None
    };

    let microvm_args = compose_microvm_args(
        root_path,
        num_vcpus,
        ram_mib,
        workdir_path,
        exec_path,
        env,
        mapped_dirs,
        port_map,
        log_level,
        args,
    );
    let microvm_envs = compose_child_envs();
    let (microvm_child, microvm_io) =
        bootstrap_microvm(&microvm_exe, microvm_args, microvm_envs).await?;

    // Monitor HERE
    // monitor_children(microvm_child, overlayfs_child).await?;

    // wait_for_shutdown(microvm_child, overlayfs_child).await?;

    Ok(())
}

//--------------------------------------------------------------------------------------------------
// Functions: Helpers
//--------------------------------------------------------------------------------------------------

async fn bootstrap_overlayfs(
    overlayfs_exe: &Path,
    overlayfs_args: Vec<String>,
    overlayfs_envs: Vec<(String, String)>,
) -> MonocoreResult<(Child, ChildIo)> {
    todo!()
}

async fn bootstrap_microvm(
    child_exe: &Path,
    child_args: Vec<String>,
    child_envs: Vec<(String, String)>,
) -> MonocoreResult<(Child, ChildIo)> {
    // Check if we're running in an interactive terminal
    if term::is_interactive_terminal() {
        tracing::info!("running microvm in an interactive terminal");
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
        let mut command = Command::new(&child_exe);
        command
            .args(&child_args)
            .envs(child_envs)
            .stdin(Stdio::from(slave_in))
            .stdout(Stdio::from(slave_out))
            .stderr(Stdio::from(slave_err));

        // Set up child's session and controlling terminal
        unsafe {
            command.pre_exec(|| {
                nix::unistd::setsid()
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
                if libc::ioctl(libc::STDIN_FILENO, libc::TIOCSCTTY as _, 1 as libc::c_long) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }

        let child = command.spawn()?;

        // Set up master file handles for asynchronous I/O
        let master_fd_owned = pty.master;
        let master_write_fd = nix::unistd::dup(master_fd_owned.as_raw_fd())?;
        let master_read_file = unsafe { std::fs::File::from_raw_fd(master_fd_owned.into_raw_fd()) };
        let master_write_file = unsafe { std::fs::File::from_raw_fd(master_write_fd) };

        let master_read = AsyncFd::new(master_read_file)?;
        let master_write = File::from_std(master_write_file);

        // Create the TTY ChildIO
        let child_io = ChildIo::TTY {
            master_read,
            master_write,
        };

        Ok((child, child_io))
    } else {
        tracing::info!("running microvm in a non-interactive terminal");
        // Start child process with pipes
        let mut child = Command::new(&child_exe)
            .args(&child_args)
            .envs(child_envs)
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

        Ok((child, child_io))
    }
}

async fn monitor_overlayfs() -> MonocoreResult<()> {
    Ok(())
}

async fn monitor_microvm() -> MonocoreResult<()> {
    Ok(())
}

async fn wait_for_shutdown(
    mut microvm_child: Child,
    mut overlayfs_child: Option<Child>,
) -> MonocoreResult<()> {
    // Setup signal handlers
    let mut sigterm = signal(SignalKind::terminate())?;
    let mut sigint = signal(SignalKind::interrupt())?;

    // PIDs
    let microvm_child_pid = microvm_child.id().expect("failed to get child process id");
    let overlayfs_child_pid = overlayfs_child
        .as_ref()
        .map(|c| c.id().expect("failed to get child process id"));

    // Wait for either child process to exit or signal to be received
    tokio::select! {
        status = microvm_child.wait() => {
            // // Stop process monitoring
            // self.process_monitor.stop().await?;

            tracing::info!("child process {} exited", microvm_child_pid);

            if status.is_ok() {
                if let Ok(status) = status {
                    if status.success() {
                        tracing::info!(
                            "microvm child process {} exited successfully",
                            microvm_child_pid
                        );
                    } else {
                        tracing::error!(
                            "microvm child process {} exited with status: {:?}",
                            microvm_child_pid,
                            status
                        );
                    }
                }
            } else {
                tracing::error!(
                    "failed to wait for microvm child process {}: {:?}",
                    microvm_child_pid,
                    status
                );
            }
        }
        _ = sigterm.recv() => {
            // Stop process monitoring
            // self.process_monitor.stop().await?;

            tracing::info!("received SIGTERM signal");

            if let Err(e) = nix::sys::signal::kill(Pid::from_raw(microvm_child_pid as i32), nix::sys::signal::Signal::SIGTERM) {
                tracing::error!(
                    "failed to send SIGTERM to process {}: {}",
                    microvm_child_pid,
                    e
                );
            }

            // Wait for child to exit after sending signal
            if let Err(e) = microvm_child.wait().await {
                tracing::error!(
                    "error waiting for child after SIGTERM: {}",
                    e
                );
            }
        }
        _ = sigint.recv() => {
            // Stop process monitoring
            // self.process_monitor.stop().await?;

            tracing::info!("received SIGINT signal");

            if let Err(e) = nix::sys::signal::kill(Pid::from_raw(microvm_child_pid as i32), nix::sys::signal::Signal::SIGTERM) {
                tracing::error!(
                    "failed to send SIGTERM to process {}: {}",
                    microvm_child_pid,
                    e
                );
            }

            // Wait for child to exit after sending signal
            if let Err(e) = microvm_child.wait().await {
                tracing::error!(
                    "error waiting for child after SIGINT: {}",
                    e
                );
            }
        }
    }

    Ok(())
}

fn compose_microvm_args(
    root_path: impl AsRef<Path>,
    num_vcpus: u8,
    ram_mib: u32,
    workdir_path: String,
    exec_path: impl AsRef<Path>,
    env: Vec<String>,
    mapped_dirs: Vec<String>,
    port_map: Vec<String>,
    log_level: Option<u8>,
    args: Vec<String>,
) -> Vec<String> {
    let mut child_args = vec![
        "microvm".to_string(),
        format!("--root-path={}", root_path.as_ref().display()),
        format!("--num-vcpus={}", num_vcpus),
        format!("--ram-mib={}", ram_mib),
        format!("--workdir-path={}", workdir_path),
        format!("--exec-path={}", exec_path.as_ref().display()),
    ];

    // Set env if provided
    if !env.is_empty() {
        child_args.push(format!("--env={}", env.join(",")));
    }

    // Set mapped dirs if provided
    if !mapped_dirs.is_empty() {
        child_args.push(format!("--mapped-dirs={}", mapped_dirs.join(",")));
    }

    // Set port map if provided
    if !port_map.is_empty() {
        child_args.push(format!("--port-map={}", port_map.join(",")));
    }

    // Set log level if provided
    if let Some(log_level) = log_level {
        child_args.push(format!("--log-level={}", log_level));
    }

    // Set args if provided
    if !args.is_empty() {
        child_args.push("--".to_string());
        for arg in args {
            child_args.push(arg);
        }
    }

    child_args
}

fn compose_overlayfs_args(
    overlayfs_layer_paths: Vec<PathBuf>,
    nfs_host: IpAddr,
    nfs_port: u16,
) -> Vec<String> {
    vec![
        "overlayfs".to_string(),
        format!(
            "--overlayfs-layer-paths={}",
            overlayfs_layer_paths
                .iter()
                .map(|p| p.to_string_lossy())
                .collect::<Vec<_>>()
                .join(",")
        ),
        format!("--nfs-host={}", nfs_host),
        format!("--nfs-port={}", nfs_port),
    ]
}

fn compose_child_envs() -> Vec<(String, String)> {
    let mut child_envs = Vec::<(String, String)>::new();

    // Only pass RUST_LOG if it's set in the environment
    if let Ok(rust_log) = std::env::var("RUST_LOG") {
        tracing::debug!("using existing RUST_LOG: {:?}", rust_log);
        child_envs.push(("RUST_LOG".to_string(), rust_log));
    }

    child_envs
}
