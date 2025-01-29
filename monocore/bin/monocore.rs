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
            management::init_env(path).await?;
            tracing::info!("Successfully initialized monocore project");
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
