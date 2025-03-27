//! Server management for the Monocore runtime.

use std::{path::PathBuf, process::Stdio};

use tokio::{fs, process::Command};

use crate::{
    config::DEFAULT_MCRUN_EXE_PATH,
    utils::{self, MCRUN_EXE_ENV_VAR, SERVER_PID_FILE},
    MonocoreError, MonocoreResult,
};

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Start the sandbox server
pub async fn start(
    port: Option<u16>,
    path: Option<PathBuf>,
    disable_default: bool,
    detach: bool,
) -> MonocoreResult<()> {
    let mcrun_path =
        monoutils::path::resolve_env_path(MCRUN_EXE_ENV_VAR, &*DEFAULT_MCRUN_EXE_PATH)?;

    let mut command = Command::new(mcrun_path);
    command.arg("server");

    if let Some(port) = port {
        command.arg("--port").arg(port.to_string());
    }

    if let Some(path) = path {
        command.arg("--path").arg(path);
    }

    if disable_default {
        command.arg("--disable-default");
    }

    if detach {
        unsafe {
            command.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }

        // TODO: Redirect to log file
        // Redirect the i/o to /dev/null
        command.stdout(Stdio::null());
        command.stderr(Stdio::null());
        command.stdin(Stdio::null());
    }

    let mut child = command.spawn()?;

    let pid = child.id().unwrap_or(0);
    tracing::info!("started sandbox server process with PID: {}", pid);

    // Create PID file
    let monocore_home_path = utils::get_monocore_home_path();
    let pid_file_path = monocore_home_path.join(SERVER_PID_FILE);

    // Ensure monocore home directory exists
    fs::create_dir_all(&monocore_home_path).await?;

    // Write PID to file
    fs::write(&pid_file_path, pid.to_string())
        .await
        .map_err(|e| {
            MonocoreError::SandboxServerError(format!(
                "failed to write PID file {}: {}",
                pid_file_path.display(),
                e
            ))
        })?;

    if detach {
        return Ok(());
    }

    // Wait for the child process to complete
    let status = child.wait().await?;
    if !status.success() {
        tracing::error!(
            "child process — sandbox server — exited with status: {}",
            status
        );
        // Clean up PID file if process fails
        if pid_file_path.exists() {
            let _ = fs::remove_file(&pid_file_path).await;
        }
        return Err(MonocoreError::SandboxServerError(format!(
            "child process — sandbox server — failed with exit status: {}",
            status
        )));
    }

    // Clean up PID file on successful exit
    if pid_file_path.exists() {
        let _ = fs::remove_file(&pid_file_path).await;
    }

    Ok(())
}

/// Stop the sandbox server
pub async fn stop() -> MonocoreResult<()> {
    let monocore_home_path = utils::get_monocore_home_path();
    let pid_file_path = monocore_home_path.join(SERVER_PID_FILE);

    // Check if PID file exists
    if !pid_file_path.exists() {
        return Err(MonocoreError::SandboxServerError(
            "server is not running (PID file not found)".to_string(),
        ));
    }

    // Read PID from file
    let pid_str = fs::read_to_string(&pid_file_path).await?;
    let pid = pid_str.trim().parse::<i32>().map_err(|_| {
        MonocoreError::SandboxServerError("invalid PID found in server.pid file".to_string())
    })?;

    // Send SIGTERM to the process
    unsafe {
        if libc::kill(pid, libc::SIGTERM) != 0 {
            // If process doesn't exist, clean up PID file and return error
            if std::io::Error::last_os_error().raw_os_error().unwrap() == libc::ESRCH {
                fs::remove_file(&pid_file_path).await?;
                return Err(MonocoreError::SandboxServerError(
                    "server process not found (stale PID file removed)".to_string(),
                ));
            }
            return Err(MonocoreError::SandboxServerError(format!(
                "failed to stop server process (PID: {})",
                pid
            )));
        }
    }

    // Delete PID file
    fs::remove_file(&pid_file_path).await?;

    tracing::info!("stopped sandbox server process (PID: {})", pid);
    Ok(())
}
