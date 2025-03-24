use clap::{CommandFactory, Parser};
use monocore::{
    cli::{MonocoreArgs, MonocoreSubcommand},
    config::DEFAULT_SCRIPT,
    management, MonocoreError, MonocoreResult,
};

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

const SANDBOX_SCRIPT_SEPARATOR: char = '~';

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

            tracing::trace!("initializing monocore project: path={path:?}");
            management::init_menv(path).await?;
        }
        Some(MonocoreSubcommand::Pull {
            image,
            image_group,
            name,
            layer_path,
        }) => {
            tracing::trace!("pulling image: name={name}, image={image}, image_group={image_group}, layer_path={layer_path:?}");
            management::pull_image(name, image, image_group, layer_path).await?;
        }
        Some(MonocoreSubcommand::Run {
            sandbox_script,
            sandbox_script_with_flag,
            args,
            path,
            config,
        }) => {
            let sandbox_script = match (sandbox_script, sandbox_script_with_flag) {
                (Some(name), None) => name,
                (None, Some(name)) => name,
                (Some(_), Some(_)) => {
                    return Err(MonocoreError::InvalidArgument(
                        "Please use only one method to specify the sandbox.".to_string(),
                    ));
                }
                (None, None) => {
                    return Err(MonocoreError::InvalidArgument(
                        "Must specify a sandbox name".to_string(),
                    ));
                }
            };

            // Split the sandbox script into sandbox name and script name
            let (sandbox, script) = match sandbox_script.split_once(SANDBOX_SCRIPT_SEPARATOR) {
                Some((sandbox, script)) => (sandbox.to_string(), script.to_string()),
                None => (sandbox_script, DEFAULT_SCRIPT.to_string()),
            };

            tracing::trace!("running sandbox: sandbox={sandbox}, script={script:?}, args={args:?}, path={path:?}");
            management::run_sandbox(&sandbox, script, args, path, config).await?;
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
