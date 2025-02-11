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
            tracing::info!("Initializing monocore project: path={path:?}");
            management::init_menv(path).await?;
            tracing::info!("Successfully initialized monocore project");
        }
        Some(MonocoreSubcommand::Pull {
            image,
            image_group,
            name,
        }) => {
            tracing::info!("Pulling image: name={name}, image={image}, image_group={image_group}");
            management::pull_image(name, image, image_group).await?;
            tracing::info!("Successfully pulled image");
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
