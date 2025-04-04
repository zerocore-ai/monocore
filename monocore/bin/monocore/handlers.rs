use clap::{error::ErrorKind, CommandFactory};
use monocore::{
    cli::{AnsiStyles, MonocoreArgs},
    management::{
        config::{self, Component, ComponentType},
        menv, orchestra, sandbox,
    },
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

pub async fn add_subcommand(
    sandbox: bool,
    build: bool,
    group: bool,
    names: Vec<String>,
    image: String,
    ram: Option<u32>,
    cpus: Option<u32>,
    volumes: Vec<String>,
    ports: Vec<String>,
    envs: Vec<String>,
    env_file: Option<Utf8UnixPathBuf>,
    depends_on: Vec<String>,
    workdir: Option<Utf8UnixPathBuf>,
    shell: Option<String>,
    scripts: Vec<(String, String)>,
    imports: Vec<(String, String)>,
    exports: Vec<(String, String)>,
    reach: Option<String>,
    path: Option<PathBuf>,
    config: Option<String>,
) -> MonocoreResult<()> {
    trio_conflict_error(build, sandbox, group, "add", "[NAMES]");
    unsupported_build_group_error(build, group, "add", "[NAMES]");

    let component = Component::Sandbox {
        image,
        ram,
        cpus,
        volumes,
        ports,
        envs,
        env_file,
        depends_on,
        workdir,
        shell,
        scripts: scripts.into_iter().map(|(k, v)| (k, v.into())).collect(),
        imports: imports.into_iter().map(|(k, v)| (k, v.into())).collect(),
        exports: exports.into_iter().map(|(k, v)| (k, v.into())).collect(),
        reach,
    };

    config::add(&names, &component, path.as_deref(), config.as_deref()).await
}

pub async fn remove_subcommand(
    sandbox: bool,
    build: bool,
    group: bool,
    names: Vec<String>,
    path: Option<PathBuf>,
    config: Option<String>,
) -> MonocoreResult<()> {
    trio_conflict_error(build, sandbox, group, "remove", "[NAMES]");
    unsupported_build_group_error(build, group, "remove", "[NAMES]");
    config::remove(
        ComponentType::Sandbox,
        &names,
        path.as_deref(),
        config.as_deref(),
    )
    .await
}

pub async fn list_subcommand(
    sandbox: bool,
    build: bool,
    group: bool,
    path: Option<PathBuf>,
    config: Option<String>,
) -> MonocoreResult<()> {
    trio_conflict_error(build, sandbox, group, "list", "[NAMES]");
    unsupported_build_group_error(build, group, "list", "[NAMES]");
    let names = config::list(ComponentType::Sandbox, path.as_deref(), config.as_deref()).await?;
    for name in names {
        println!("{}", name);
    }

    Ok(())
}

pub async fn init_subcommand(
    path: Option<PathBuf>,
    path_with_flag: Option<PathBuf>,
) -> MonocoreResult<()> {
    let path = match (path, path_with_flag) {
        (Some(path), None) => Some(path),
        (None, Some(path)) => Some(path),
        (Some(_), Some(_)) => {
            MonocoreArgs::command()
                .override_usage(usage("init", "[PATH]", None))
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
            .override_usage(usage("run", "[NAME]", Some("<ARGS>")))
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

    unsupported_build_group_error(build, sandbox, "run", "[NAME]");

    let (sandbox, script) = parse_name_and_script(&name);
    if matches!((script, &exec), (Some(_), Some(_))) {
        MonocoreArgs::command()
            .override_usage(usage("run", "[NAME[~SCRIPT]]", Some("<ARGS>")))
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
        path.as_deref(),
        config.as_deref(),
        args,
        detach,
        exec.as_deref(),
        true,
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
            .override_usage(usage(&script, "[NAME]", Some("<ARGS>")))
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

    unsupported_build_group_error(build, sandbox, &script, "[NAME]");

    sandbox::run(
        &name,
        Some(&script),
        path.as_deref(),
        config.as_deref(),
        args,
        detach,
        exec.as_deref(),
        true,
    )
    .await
}

pub async fn tmp_subcommand(
    name: String,
    cpus: Option<u8>,
    ram: Option<u32>,
    volumes: Vec<String>,
    ports: Vec<String>,
    envs: Vec<String>,
    reach: Option<String>,
    workdir: Option<Utf8UnixPathBuf>,
    exec: Option<String>,
    args: Vec<String>,
) -> MonocoreResult<()> {
    let (image, script) = parse_name_and_script(&name);
    let image = image.parse::<Reference>()?;

    if matches!((script, &exec), (Some(_), Some(_))) {
        MonocoreArgs::command()
            .override_usage(usage("tmp", "[NAME[~SCRIPT]]", Some("<ARGS>")))
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
        reach,
        workdir,
        exec.as_deref(),
        args,
        true,
    )
    .await
}

pub async fn up_subcommand(
    sandbox: bool,
    build: bool,
    group: bool,
    names: Vec<String>,
    path: Option<PathBuf>,
    config: Option<String>,
) -> MonocoreResult<()> {
    trio_conflict_error(build, sandbox, group, "up", "[NAMES]");
    unsupported_build_group_error(build, group, "up", "[NAMES]");

    orchestra::up(names, path.as_deref(), config.as_deref()).await
}

pub async fn down_subcommand(
    sandbox: bool,
    build: bool,
    group: bool,
    names: Vec<String>,
    path: Option<PathBuf>,
    config: Option<String>,
) -> MonocoreResult<()> {
    trio_conflict_error(build, sandbox, group, "down", "[NAMES]");
    unsupported_build_group_error(build, group, "down", "[NAMES]");

    orchestra::down(names, path.as_deref(), config.as_deref()).await
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
    trio_conflict_error(build, sandbox, group, "log", "[NAME]");
    unsupported_build_group_error(build, group, "log", "[NAME]");

    // Check if tail command exists when follow mode is requested
    if follow {
        let tail_exists = which::which("tail").is_ok();
        if !tail_exists {
            MonocoreArgs::command()
                .override_usage(usage("log", "[NAME]", None))
                .error(
                    ErrorKind::InvalidValue,
                    "'tail' command not found. Please install it to use the follow (-f) option.",
                )
                .exit();
        }
    }

    // Load the configuration to get canonical paths
    let (_, canonical_project_dir, config_file) =
        config::load_config(project_dir.as_deref(), config_file.as_deref()).await?;

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
// Functions: Common Errors
//--------------------------------------------------------------------------------------------------

fn trio_conflict_error(
    build: bool,
    sandbox: bool,
    group: bool,
    command: &str,
    positional_placeholder: &str,
) {
    match (build, sandbox, group) {
        (true, true, _) => conflict_error("build", "sandbox", command, positional_placeholder),
        (true, _, true) => conflict_error("build", "group", command, positional_placeholder),
        (_, true, true) => conflict_error("sandbox", "group", command, positional_placeholder),
        _ => (),
    }
}

fn conflict_error(arg1: &str, arg2: &str, command: &str, positional_placeholder: &str) {
    MonocoreArgs::command()
        .override_usage(usage(command, positional_placeholder, None))
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

fn unsupported_build_group_error(
    build: bool,
    group: bool,
    command: &str,
    positional_placeholder: &str,
) {
    if build || group {
        MonocoreArgs::command()
            .override_usage(usage(command, positional_placeholder, None))
            .error(
                ErrorKind::ArgumentConflict,
                format!(
                    "`{}/{}` or `{}/{}` not yet supported.",
                    "--build".literal(),
                    "-b".literal(),
                    "--group".literal(),
                    "-g".literal()
                ),
            )
            .exit();
    }
}

//--------------------------------------------------------------------------------------------------
// Functions: Helpers
//--------------------------------------------------------------------------------------------------

fn usage(command: &str, positional_placeholder: &str, varargs: Option<&str>) -> String {
    let mut usage = format!(
        "{} {} {} {}",
        "monocore".literal(),
        command.literal(),
        "[OPTIONS]".placeholder(),
        positional_placeholder.placeholder()
    );

    if let Some(varargs) = varargs {
        usage.push_str(&format!(
            " {} {} {}",
            "[--".literal(),
            format!("{}...", varargs).placeholder(),
            "]".literal()
        ));
    }

    usage
}

fn parse_name_and_script(name_and_script: &str) -> (&str, Option<&str>) {
    let (name, script) = match name_and_script.split_once(SANDBOX_SCRIPT_SEPARATOR) {
        Some((name, script)) => (name, Some(script)),
        None => (name_and_script, None),
    };

    (name, script)
}
