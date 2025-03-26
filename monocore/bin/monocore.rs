use clap::{error::ErrorKind, CommandFactory, Parser};
use monocore::{
    cli::{AnsiStyles, MonocoreArgs, MonocoreSubcommand},
    management::{image, menv, orchestra, sandbox},
    oci::Reference,
    MonocoreResult,
};

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

const SANDBOX_SCRIPT_SEPARATOR: char = '~';
const START_SCRIPT: &str = "start";
const SHELL_SCRIPT: &str = "shell";

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
