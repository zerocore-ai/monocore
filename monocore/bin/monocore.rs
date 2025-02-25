use clap::{CommandFactory, Parser};
use monocore::{
    cli::{MonocoreArgs, MonocoreSubcommand},
    management, MonocoreResult,
};

//--------------------------------------------------------------------------------------------------
// Functions: main
//--------------------------------------------------------------------------------------------------

#[tokio::main]
async fn main() -> MonocoreResult<()> {
    tracing_subscriber::fmt::init();

    // Parse command line arguments
    let args = MonocoreArgs::parse();
    match args.subcommand {
        Some(MonocoreSubcommand::Init { path }) => {
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
            sandbox,
            name,
            script,
            args,
            path,
            config,
        }) => {
            tracing::trace!("running sandbox: name={name}, sandbox={sandbox}, script={script:?}, args={args:?}, path={path:?}");
            management::run_sandbox(&name, script, args, path, config).await?;
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
