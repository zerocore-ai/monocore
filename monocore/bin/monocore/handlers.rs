use clap::{error::ErrorKind, CommandFactory};
use monocore::{
    cli::{AnsiStyles, MonocoreArgs},
    config,
    management::{menv, orchestra, sandbox},
    oci::Reference,
    MonocoreError, MonocoreResult,
};
use std::path::PathBuf;
use typed_path::Utf8UnixPathBuf;

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

const SANDBOX_SCRIPT_SEPARATOR: char = '~';
const MONOCORE_ENV_DIR: &str = ".menv";
const LOG_SUBDIR: &str = "log";

//--------------------------------------------------------------------------------------------------
// Functions: Handlers
//--------------------------------------------------------------------------------------------------

pub async fn init_subcommand(
    path: Option<PathBuf>,
    path_with_flag: Option<PathBuf>,
) -> MonocoreResult<()> {
    let path = match (path, path_with_flag) {
        (Some(path), None) => Some(path),
        (None, Some(path)) => Some(path),
        (Some(_), Some(_)) => {
            MonocoreArgs::command()
                .override_usage(format!(
                    "{} {} {} {}",
                    "monocore".literal(),
                    "init".literal(),
                    "[OPTIONS]".placeholder(),
                    "[PATH]".placeholder()
                ))
                .error(
                    ErrorKind::ArgumentConflict,
                    format!(
                        "cannot specify path both as a positional argument and with `{}` or `{}` flag",
                        "--path".placeholder(),
                        "-p".placeholder()
                    ),
                )
                .exit();
        }
        (None, None) => None,
    };

    menv::initialize(path).await?;

    Ok(())
}

pub async fn run_subcommand(
    sandbox: bool,
    build: bool,
    name: String,
    args: Vec<String>,
    path: Option<PathBuf>,
    config: Option<String>,
    detach: bool,
    exec: Option<String>,
) -> MonocoreResult<()> {
    if build && sandbox {
        MonocoreArgs::command()
            .override_usage(format!(
                "{} {} {} {} {} {}{}",
                "monocore".literal(),
                "run".literal(),
                "[OPTIONS]".placeholder(),
                "[NAME]".placeholder(),
                "[--".literal(),
                "<ARGS>...".placeholder(),
                "]".literal()
            ))
            .error(
                ErrorKind::ArgumentConflict,
                format!(
                    "cannot specify both `{}` and `{}` flags",
                    "--sandbox".literal(),
                    "--build".literal()
                ),
            )
            .exit();
    }

    if build {
        MonocoreArgs::command()
            .override_usage(format!(
                "{} {} {} {} {} {}{}",
                "monocore".literal(),
                "run".literal(),
                "[OPTIONS]".placeholder(),
                "[NAME]".placeholder(),
                "[--".literal(),
                "<ARGS>...".placeholder(),
                "]".literal()
            ))
            .error(
                ErrorKind::ArgumentConflict,
                format!("`{}` not yet supported.", "--build".literal()),
            )
            .exit();
    }

    let (sandbox, script) = parse_name_and_script(&name);
    if matches!((script, &exec), (Some(_), Some(_))) {
        MonocoreArgs::command()
            .override_usage(format!(
                "{} {} {} {} {} {}{}",
                "monocore".literal(),
                "run".literal(),
                "[OPTIONS]".placeholder(),
                "[NAME[~SCRIPT]]".placeholder(),
                "[--".literal(),
                "<ARGS>...".placeholder(),
                "]".literal()
            ))
            .error(
                ErrorKind::ArgumentConflict,
                format!(
                    "cannot specify both a script and an `{}` option.",
                    "--exec".placeholder()
                ),
            )
            .exit();
    }

    sandbox::run(
        &sandbox,
        script,
        path,
        config.as_deref(),
        args,
        detach,
        exec.as_deref(),
    )
    .await?;

    Ok(())
}

pub async fn script_run_subcommand(
    sandbox: bool,
    build: bool,
    name: String,
    script: String,
    args: Vec<String>,
    path: Option<PathBuf>,
    config: Option<String>,
    detach: bool,
    exec: Option<String>,
) -> MonocoreResult<()> {
    if build && sandbox {
        MonocoreArgs::command()
            .override_usage(format!(
                "{} {} {} {} {} {}{}",
                "monocore".literal(),
                script.literal(),
                "[OPTIONS]".placeholder(),
                "[NAME]".placeholder(),
                "[--".literal(),
                "<ARGS>...".placeholder(),
                "]".literal()
            ))
            .error(
                ErrorKind::ArgumentConflict,
                format!(
                    "cannot specify both `{}` and `{}` flags",
                    "--sandbox".literal(),
                    "--build".literal()
                ),
            )
            .exit();
    }

    if build {
        MonocoreArgs::command()
            .override_usage(format!(
                "{} {} {} {} {} {}{}",
                "monocore".literal(),
                script.literal(),
                "[OPTIONS]".placeholder(),
                "[NAME]".placeholder(),
                "[--".literal(),
                "<ARGS>...".placeholder(),
                "]".literal()
            ))
            .error(
                ErrorKind::ArgumentConflict,
                format!("`{}` not yet supported.", "--build".literal()),
            )
            .exit();
    }

    sandbox::run(
        &name,
        Some(&script),
        path,
        config.as_deref(),
        args,
        detach,
        exec.as_deref(),
    )
    .await?;

    Ok(())
}

pub async fn tmp_subcommand(
    name: String,
    cpus: Option<u8>,
    ram: Option<u32>,
    volumes: Vec<String>,
    ports: Vec<String>,
    envs: Vec<String>,
    workdir: Option<Utf8UnixPathBuf>,
    exec: Option<String>,
    args: Vec<String>,
) -> MonocoreResult<()> {
    let (image, script) = parse_name_and_script(&name);
    let image = image.parse::<Reference>()?;

    if matches!((script, &exec), (Some(_), Some(_))) {
        MonocoreArgs::command()
            .override_usage(format!(
                "{} {} {} {} {} {}{}",
                "monocore".literal(),
                "tmp".literal(),
                "[OPTIONS]".placeholder(),
                "[NAME[~SCRIPT]]".placeholder(),
                "[--".literal(),
                "<ARGS>...".placeholder(),
                "]".literal()
            ))
            .error(
                ErrorKind::ArgumentConflict,
                format!(
                    "cannot specify both a script and an `{}` option.",
                    "--exec".placeholder()
                ),
            )
            .exit();
    }

    sandbox::run_temp(
        &image,
        script,
        cpus,
        ram,
        volumes,
        ports,
        envs,
        workdir,
        exec.as_deref(),
        args,
    )
    .await?;

    Ok(())
}

pub async fn up_subcommand(
    sandbox: bool,
    build: bool,
    group: bool,
    names: Vec<String>,
    path: Option<PathBuf>,
    config: Option<String>,
) -> MonocoreResult<()> {
    match (build, sandbox, group) {
        (true, true, _) => conflict_error("build", "sandbox", "up"),
        (true, _, true) => conflict_error("build", "group", "up"),
        (_, true, true) => conflict_error("sandbox", "group", "up"),
        _ => (),
    }

    if build || group {
        MonocoreArgs::command()
            .override_usage(format!(
                "{} {} {} {}",
                "monocore".literal(),
                "up".literal(),
                "[OPTIONS]".placeholder(),
                "[NAME]".placeholder(),
            ))
            .error(
                ErrorKind::ArgumentConflict,
                format!(
                    "`{}` or `{}` not yet supported.",
                    "--build".literal(),
                    "--group".literal()
                ),
            )
            .exit();
    }

    orchestra::up(names, path, config.as_deref()).await?;

    Ok(())
}

pub async fn down_subcommand(
    sandbox: bool,
    build: bool,
    group: bool,
    names: Vec<String>,
    path: Option<PathBuf>,
    config: Option<String>,
) -> MonocoreResult<()> {
    match (build, sandbox, group) {
        (true, true, _) => conflict_error("build", "sandbox", "down"),
        (true, _, true) => conflict_error("build", "group", "down"),
        (_, true, true) => conflict_error("sandbox", "group", "down"),
        _ => (),
    }

    if build || group {
        MonocoreArgs::command()
            .override_usage(format!(
                "{} {} {} {}",
                "monocore".literal(),
                "down".literal(),
                "[OPTIONS]".placeholder(),
                "[NAME]".placeholder(),
            ))
            .error(
                ErrorKind::ArgumentConflict,
                format!(
                    "`{}` or `{}` not yet supported.",
                    "--build".literal(),
                    "--group".literal()
                ),
            )
            .exit();
    }

    orchestra::down(names, path, config.as_deref()).await?;

    Ok(())
}

pub async fn log_subcommand(
    sandbox: bool,
    build: bool,
    group: bool,
    name: String,
    project_dir: Option<PathBuf>,
    config_file: Option<String>,
    follow: bool,
    tail: Option<usize>,
) -> MonocoreResult<()> {
    match (build, sandbox, group) {
        (true, true, _) => conflict_error("build", "sandbox", "log"),
        (true, _, true) => conflict_error("build", "group", "log"),
        (_, true, true) => conflict_error("sandbox", "group", "log"),
        _ => (),
    }

    if build || group {
        MonocoreArgs::command()
            .override_usage(format!(
                "{} {} {} {}",
                "monocore".literal(),
                "log".literal(),
                "[OPTIONS]".placeholder(),
                "[NAME]".placeholder(),
            ))
            .error(
                ErrorKind::ArgumentConflict,
                format!(
                    "`{}` or `{}` not yet supported.",
                    "--build".literal(),
                    "--group".literal()
                ),
            )
            .exit();
    }

    // Check if tail command exists when follow mode is requested
    if follow {
        let tail_exists = which::which("tail").is_ok();
        if !tail_exists {
            MonocoreArgs::command()
                .error(
                    ErrorKind::InvalidValue,
                    "'tail' command not found. Please install it to use the follow (-f) option.",
                )
                .exit();
        }
    }

    // Load the configuration to get canonical paths
    let (_, canonical_project_dir, config_file) =
        config::load_config(project_dir, config_file.as_deref()).await?;

    // Construct log file path: <project_dir>/.menv/log/<config>-<sandbox>.log
    let log_path = canonical_project_dir
        .join(MONOCORE_ENV_DIR)
        .join(LOG_SUBDIR)
        .join(format!("{}-{}.log", config_file, name));

    // Check if log file exists
    if !log_path.exists() {
        return Err(MonocoreError::LogNotFound(format!(
            "Log file not found at {}",
            log_path.display()
        )));
    }

    if follow {
        // For follow mode, use tokio::process::Command to run `tail -f`
        let mut child = tokio::process::Command::new("tail")
            .arg("-f")
            .arg(&log_path)
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .spawn()?;

        // Wait for the tail process
        let status = child.wait().await?;
        if !status.success() {
            return Err(MonocoreError::ProcessWaitError(format!(
                "tail process exited with status: {}",
                status
            )));
        }
    } else {
        // Read the file contents
        let contents = tokio::fs::read_to_string(&log_path).await?;

        // Split into lines
        let lines: Vec<&str> = contents.lines().collect();

        // If tail is specified, only show the last N lines
        let lines_to_print = if let Some(n) = tail {
            if n >= lines.len() {
                &lines[..]
            } else {
                &lines[lines.len() - n..]
            }
        } else {
            &lines[..]
        };

        // Print the lines
        for line in lines_to_print {
            println!("{}", line);
        }
    }

    Ok(())
}

//--------------------------------------------------------------------------------------------------
// Functions: Helpers
//--------------------------------------------------------------------------------------------------

fn conflict_error(arg1: &str, arg2: &str, command: &str) {
    MonocoreArgs::command()
        .override_usage(format!(
            "{} {} {} {}",
            "monocore".literal(),
            command.literal(),
            "[OPTIONS]".placeholder(),
            "[NAME]".placeholder(),
        ))
        .error(
            ErrorKind::ArgumentConflict,
            format!(
                "cannot specify both `{}` and `{}` flags",
                format!("--{}", arg1).literal(),
                format!("--{}", arg2).literal()
            ),
        )
        .exit();
}

fn parse_name_and_script(name_and_script: &str) -> (&str, Option<&str>) {
    let (name, script) = match name_and_script.split_once(SANDBOX_SCRIPT_SEPARATOR) {
        Some((name, script)) => (name, Some(script)),
        None => (name_and_script, None),
    };

    (name, script)
}
