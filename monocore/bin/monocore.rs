use clap::{CommandFactory, Parser};
use monocore::{
    cli::{MonocoreArgs, MonocoreSubcommand},
    config::DEFAULT_SCRIPT,
    management::{image, menv, orchestra, sandbox},
    oci::Reference,
    MonocoreError, MonocoreResult,
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
                    return Err(MonocoreError::InvalidArgument(
                        "Please use only one method to specify the path.".to_string(),
                    ));
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
        }) => {
            let sandbox_script = determine_name_from_positional_or_flag_args(
                sandbox_script,
                sandbox_script_with_flag,
                "sandbox",
            )?;
            let (sandbox, script) = parse_name_and_script(&sandbox_script);

            sandbox::run(&sandbox, script, path, config.as_deref(), args, detach).await?;
        }
        Some(MonocoreSubcommand::Start {
            sandbox,
            sandbox_with_flag,
            args,
            path,
            config,
            detach,
        }) => {
            let sandbox =
                determine_name_from_positional_or_flag_args(sandbox, sandbox_with_flag, "sandbox")?;
            sandbox::run(
                &sandbox,
                START_SCRIPT,
                path,
                config.as_deref(),
                args,
                detach,
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
            let sandbox =
                determine_name_from_positional_or_flag_args(sandbox, sandbox_with_flag, "sandbox")?;
            sandbox::run(
                &sandbox,
                SHELL_SCRIPT,
                path,
                config.as_deref(),
                args,
                detach,
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
        }) => {
            let image_script = determine_name_from_positional_or_flag_args(
                image_script,
                image_script_with_flag,
                "image",
            )?;
            let (image, script) = parse_name_and_script(&image_script);
            let image = image.parse::<Reference>()?;
            sandbox::run_temp(&image, script, cpus, ram, volumes, ports, envs, workdir).await?;
        }
        Some(MonocoreSubcommand::Apply { path, config }) => {
            orchestra::apply(path, config.as_deref()).await?;
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

fn determine_name_from_positional_or_flag_args(
    positional: Option<String>,
    flag: Option<String>,
    name_type: &str,
) -> MonocoreResult<String> {
    match (positional, flag) {
        (Some(name), None) => Ok(name),
        (None, Some(name)) => Ok(name),
        (Some(_), Some(_)) => {
            return Err(MonocoreError::InvalidArgument(
                format!("Please use only one method to specify the {}", name_type).to_string(),
            ));
        }
        (None, None) => {
            return Err(MonocoreError::InvalidArgument(
                format!("Must specify a {}", name_type).to_string(),
            ))
        }
    }
}

fn parse_name_and_script(name_and_script: &str) -> (&str, &str) {
    let (name, script) = match name_and_script.split_once(SANDBOX_SCRIPT_SEPARATOR) {
        Some((name, script)) => (name, script),
        None => (name_and_script, DEFAULT_SCRIPT),
    };

    (name, script)
}
