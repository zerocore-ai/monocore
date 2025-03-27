use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use clap::{error::ErrorKind, CommandFactory, Parser};
use monocore::{
    cli::{AnsiStyles, MonocoreArgs, MonocoreSubcommand, ServerSubcommand},
    config::{self, DEFAULT_SERVER_PORT},
    management::{image, menv, orchestra, sandbox},
    oci::Reference,
    server::SandboxServer,
    MonocoreError, MonocoreResult,
};
use std::path::PathBuf;
//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

const SANDBOX_SCRIPT_SEPARATOR: char = '~';
const START_SCRIPT: &str = "start";
const SHELL_SCRIPT: &str = "shell";
const MONOCORE_ENV_DIR: &str = ".menv";
const LOG_SUBDIR: &str = "log";

//--------------------------------------------------------------------------------------------------
// Functions: main
//--------------------------------------------------------------------------------------------------

#[tokio::main]
async fn main() -> MonocoreResult<()> {
    tracing_subscriber::fmt::init();

    // Parse command line arguments
    let args = MonocoreArgs::parse();
    match args.subcommand {
        Some(MonocoreSubcommand::Init {
            path,
            path_with_flag,
        }) => {
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
        }
        Some(MonocoreSubcommand::Pull {
            image,
            image_group,
            name,
            layer_path,
        }) => {
            image::pull(name, image, image_group, layer_path).await?;
        }
        Some(MonocoreSubcommand::Run {
            sandbox_script,
            sandbox_script_with_flag,
            args,
            path,
            config,
            detach,
            exec,
        }) => {
            let sandbox_script = match (sandbox_script, sandbox_script_with_flag) {
                (Some(name), None) => name,
                (None, Some(name)) => name,
                (Some(_), Some(_)) => {
                    MonocoreArgs::command()
                        .override_usage(format!(
                            "{} {} {} {} {} {}{}",
                            "monocore".literal(),
                            "run".literal(),
                            "[OPTIONS]".placeholder(),
                            "[SANDBOX[~SCRIPT]]".placeholder(),
                            "[--".literal(),
                            "<ARGS>...".placeholder(),
                            "]".literal()
                        ))
                        .error(
                            ErrorKind::ArgumentConflict,
                            format!(
                                "cannot specify sandbox both as a positional argument and with `{}` or `{}` flag",
                                "--sandbox".placeholder(),
                                "-s".placeholder()
                            ),
                        )
                        .exit();
                }
                (None, None) => {
                    MonocoreArgs::command()
                        .override_usage(format!(
                            "{} {} {} {} {} {}{}",
                            "monocore".literal(),
                            "run".literal(),
                            "[OPTIONS]".placeholder(),
                            "[SANDBOX~SCRIPT]".placeholder(),
                            "[--".literal(),
                            "<ARGS>...".placeholder(),
                            "]".literal()
                        ))
                        .error(
                            ErrorKind::MissingRequiredArgument,
                            format!("must specify a sandbox or build to run."),
                        )
                        .exit();
                }
            };

            let (sandbox, script) = parse_name_and_script(&sandbox_script);
            if matches!((script, &exec), (Some(_), Some(_))) {
                MonocoreArgs::command()
                    .override_usage(format!(
                        "{} {} {} {} {} {}{}",
                        "monocore".literal(),
                        "run".literal(),
                        "[OPTIONS]".placeholder(),
                        "[SANDBOX[~SCRIPT]]".placeholder(),
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
        }
        Some(MonocoreSubcommand::Start {
            sandbox,
            sandbox_with_flag,
            args,
            path,
            config,
            detach,
        }) => {
            let sandbox = match (sandbox, sandbox_with_flag) {
                (Some(name), None) => name,
                (None, Some(name)) => name,
                (Some(_), Some(_)) => {
                    MonocoreArgs::command()
                        .override_usage(format!(
                            "{} {} {} {} {} {}{}",
                            "monocore".literal(),
                            "start".literal(),
                            "[OPTIONS]".placeholder(),
                            "[SANDBOX[~SCRIPT]]".placeholder(),
                            "[--".literal(),
                            "<ARGS>...".placeholder(),
                            "]".literal()
                        ))
                        .error(
                            ErrorKind::ArgumentConflict,
                            format!(
                                "cannot specify sandbox both as a positional argument and with `{}` or `{}` flag",
                                "--sandbox".placeholder(),
                                "-s".placeholder()
                            ),
                        )
                        .exit();
                }
                (None, None) => {
                    MonocoreArgs::command()
                        .override_usage(format!(
                            "{} {} {} {} {} {}{}",
                            "monocore".literal(),
                            "start".literal(),
                            "[OPTIONS]".placeholder(),
                            "[SANDBOX[~SCRIPT]]".placeholder(),
                            "[--".literal(),
                            "<ARGS>...".placeholder(),
                            "]".literal()
                        ))
                        .error(
                            ErrorKind::MissingRequiredArgument,
                            format!("must specify a sandbox or build to start."),
                        )
                        .exit();
                }
            };

            sandbox::run(
                &sandbox,
                Some(START_SCRIPT),
                path,
                config.as_deref(),
                args,
                detach,
                None,
            )
            .await?;
        }
        Some(MonocoreSubcommand::Shell {
            sandbox,
            sandbox_with_flag,
            args,
            path,
            config,
            detach,
        }) => {
            let sandbox = match (sandbox, sandbox_with_flag) {
                (Some(name), None) => name,
                (None, Some(name)) => name,
                (Some(_), Some(_)) => {
                    MonocoreArgs::command()
                        .override_usage(format!(
                            "{} {} {} {} {} {}{}",
                            "monocore".literal(),
                            "shell".literal(),
                            "[OPTIONS]".placeholder(),
                            "[SANDBOX[~SCRIPT]]".placeholder(),
                            "[--".literal(),
                            "<ARGS>...".placeholder(),
                            "]".literal()
                        ))
                        .error(
                            ErrorKind::ArgumentConflict,
                            format!(
                                "cannot specify sandbox both as a positional argument and with `{}` or `{}` flag",
                                "--sandbox".placeholder(),
                                "-s".placeholder()
                            ),
                        )
                        .exit();
                }
                (None, None) => {
                    MonocoreArgs::command()
                        .override_usage(format!(
                            "{} {} {} {} {} {}{}",
                            "monocore".literal(),
                            "shell".literal(),
                            "[OPTIONS]".placeholder(),
                            "[SANDBOX]".placeholder(),
                            "[--".literal(),
                            "<ARGS>...".placeholder(),
                            "]".literal()
                        ))
                        .error(
                            ErrorKind::MissingRequiredArgument,
                            format!("must specify a sandbox or build to shell into."),
                        )
                        .exit();
                }
            };

            sandbox::run(
                &sandbox,
                Some(SHELL_SCRIPT),
                path,
                config.as_deref(),
                args,
                detach,
                None,
            )
            .await?;
        }
        Some(MonocoreSubcommand::Tmp {
            image_script,
            image_script_with_flag,
            cpus,
            ram,
            volumes,
            ports,
            envs,
            workdir,
            exec,
            args,
        }) => {
            let image_script = match (image_script, image_script_with_flag) {
                (Some(name), None) => name,
                (None, Some(name)) => name,
                (Some(_), Some(_)) => {
                    MonocoreArgs::command()
                        .override_usage(format!(
                            "{} {} {} {} {} {}{}",
                            "monocore".literal(),
                            "tmp".literal(),
                            "[OPTIONS]".placeholder(),
                            "[IMAGE[~SCRIPT]]".placeholder(),
                            "[--".literal(),
                            "<ARGS>...".placeholder(),
                            "]".literal()
                        ))
                        .error(
                            ErrorKind::ArgumentConflict,
                            format!(
                                "cannot specify image both as a positional argument and with `{}` or `{}` flag",
                                "--image".placeholder(),
                                "-i".placeholder()
                            ),
                        )
                        .exit();
                }
                (None, None) => {
                    MonocoreArgs::command()
                        .override_usage(format!(
                            "{} {} {} {} {} {}{}",
                            "monocore".literal(),
                            "tmp".literal(),
                            "[OPTIONS]".placeholder(),
                            "[IMAGE[~SCRIPT]]".placeholder(),
                            "[--".literal(),
                            "<ARGS>...".placeholder(),
                            "]".literal()
                        ))
                        .error(
                            ErrorKind::MissingRequiredArgument,
                            format!("must specify an image to run."),
                        )
                        .exit();
                }
            };
            let (image, script) = parse_name_and_script(&image_script);
            let image = image.parse::<Reference>()?;

            if matches!((script, &exec), (Some(_), Some(_))) {
                MonocoreArgs::command()
                    .error(
                        ErrorKind::ArgumentConflict,
                        "cannot specify both a script and an exec command.",
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
        }
        Some(MonocoreSubcommand::Apply { path, config }) => {
            orchestra::apply(path, config.as_deref()).await?;
        }
        Some(MonocoreSubcommand::Up {
            sandboxes,
            sandboxes_with_flag,
            path,
            config,
        }) => {
            let sandboxes = match (sandboxes, sandboxes_with_flag) {
                (Some(names), None) => names,
                (None, Some(names)) => names,
                (Some(_), Some(_)) => {
                    MonocoreArgs::command()
                        .override_usage(format!(
                            "{} {} {} {}",
                            "monocore".literal(),
                            "up".literal(),
                            "[OPTIONS]".placeholder(),
                            "[SANDBOXES]...".placeholder(),
                        ))
                        .error(
                            ErrorKind::ArgumentConflict,
                            format!(
                                "cannot specify sandboxes both as positional arguments and with `{}` or `{}` flag",
                                "--sandbox".placeholder(),
                                "-s".placeholder()
                            ),
                        )
                        .exit();
                }
                (None, None) => {
                    MonocoreArgs::command()
                        .override_usage(format!(
                            "{} {} {} {}",
                            "monocore".literal(),
                            "up".literal(),
                            "[OPTIONS]".placeholder(),
                            "[SANDBOXES]...".placeholder(),
                        ))
                        .error(
                            ErrorKind::MissingRequiredArgument,
                            format!("must specify at least one sandbox or group to start."),
                        )
                        .exit();
                }
            };
            orchestra::up(sandboxes, path, config.as_deref()).await?;
        }
        Some(MonocoreSubcommand::Down {
            sandboxes,
            sandboxes_with_flag,
            path,
            config,
        }) => {
            let sandboxes = match (sandboxes, sandboxes_with_flag) {
                (Some(names), None) => names,
                (None, Some(names)) => names,
                (Some(_), Some(_)) => {
                    MonocoreArgs::command()
                        .override_usage(format!(
                            "{} {} {} {}",
                            "monocore".literal(),
                            "up".literal(),
                            "[OPTIONS]".placeholder(),
                            "[SANDBOXES]...".placeholder(),
                        ))
                        .error(
                            ErrorKind::ArgumentConflict,
                            format!(
                                "cannot specify sandboxes both as positional arguments and with `{}` or `{}` flag",
                                "--sandbox".placeholder(),
                                "-s".placeholder()
                            ),
                        )
                        .exit();
                }
                (None, None) => {
                    MonocoreArgs::command()
                        .override_usage(format!(
                            "{} {} {} {}",
                            "monocore".literal(),
                            "up".literal(),
                            "[OPTIONS]".placeholder(),
                            "[SANDBOXES]...".placeholder(),
                        ))
                        .error(
                            ErrorKind::MissingRequiredArgument,
                            format!("must specify at least one sandbox or group to start."),
                        )
                        .exit();
                }
            };
            orchestra::down(sandboxes, path, config.as_deref()).await?;
        }
        Some(MonocoreSubcommand::Log {
            sandbox,
            sandbox_with_flag,
            path,
            config,
            follow,
            tail,
        }) => {
            let sandbox = match (sandbox, sandbox_with_flag) {
                (Some(name), None) => name,
                (None, Some(name)) => name,
                (Some(_), Some(_)) => {
                    MonocoreArgs::command()
                        .override_usage(format!(
                            "{} {} {} {}",
                            "monocore".literal(),
                            "log".literal(),
                            "[OPTIONS]".placeholder(),
                            "[SANDBOX]".placeholder(),
                        ))
                        .error(
                            ErrorKind::ArgumentConflict,
                            format!(
                                "cannot specify sandbox both as a positional argument and with `{}` or `{}` flag",
                                "--sandbox".placeholder(),
                                "-s".placeholder()
                            ),
                        )
                        .exit();
                }
                (None, None) => {
                    MonocoreArgs::command()
                        .override_usage(format!(
                            "{} {} {} {}",
                            "monocore".literal(),
                            "log".literal(),
                            "[OPTIONS]".placeholder(),
                            "[SANDBOX]".placeholder(),
                        ))
                        .error(
                            ErrorKind::MissingRequiredArgument,
                            format!("must specify a sandbox or build to log."),
                        )
                        .exit();
                }
            };

            handle_log_subcommand(&sandbox, path, config.as_deref(), follow, tail).await?;
        }
        Some(MonocoreSubcommand::Server { subcommand }) => {
            match subcommand {
                ServerSubcommand::Up {
                    port,
                    path,
                    disable_default,
                } => {
                    let server = SandboxServer::new(
                        path,
                        !disable_default,
                        SocketAddr::new(
                            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                            port.unwrap_or(DEFAULT_SERVER_PORT),
                        ),
                    )?;
                    server.serve().await?;
                }
                ServerSubcommand::Down => {
                    // We need to store the pid somewhere (~/.monocore/server.pid maybe?) and send a signal to it
                    unimplemented!()
                }
            }

            todo!()
        }
        Some(_) => (), // TODO: implement other subcommands
        None => {
            MonocoreArgs::command().print_help()?;
        }
    }

    Ok(())
}

//--------------------------------------------------------------------------------------------------
// Functions: *
//--------------------------------------------------------------------------------------------------

fn parse_name_and_script(name_and_script: &str) -> (&str, Option<&str>) {
    let (name, script) = match name_and_script.split_once(SANDBOX_SCRIPT_SEPARATOR) {
        Some((name, script)) => (name, Some(script)),
        None => (name_and_script, None),
    };

    (name, script)
}

async fn handle_log_subcommand(
    sandbox: &str,
    project_dir: Option<PathBuf>,
    config_file: Option<&str>,
    follow: bool,
    tail: Option<usize>,
) -> MonocoreResult<()> {
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
        config::load_config(project_dir, config_file).await?;

    // Construct log file path: <project_dir>/.menv/log/<config>-<sandbox>.log
    let log_path = canonical_project_dir
        .join(MONOCORE_ENV_DIR)
        .join(LOG_SUBDIR)
        .join(format!("{}-{}.log", config_file, sandbox));

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
