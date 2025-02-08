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
    // Parse command line arguments
    let args = MonocoreArgs::parse();
    match args.subcommand {
        Some(MonocoreSubcommand::Init { path }) => {
            tracing::info!("Initializing monocore project...");
            management::init_menv(path).await?;
            tracing::info!("Successfully initialized monocore project");
        }
        Some(MonocoreSubcommand::Pull { image, image_group, name }) => {
            tracing::info!("Pulling image...");
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
