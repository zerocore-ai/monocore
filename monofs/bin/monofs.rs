use clap::{CommandFactory, Parser};
use monofs::{
    cli::{MonofsArgs, MonofsSubcommand},
    management,
};

//--------------------------------------------------------------------------------------------------
// Functions: main
//--------------------------------------------------------------------------------------------------

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Parse command line arguments
    let args = MonofsArgs::parse();
    match args.subcommand {
        Some(MonofsSubcommand::Init { mount_dir }) => {
            tracing::info!("Initializing monofs project...");
            management::init_mfs(mount_dir).await?;
            tracing::info!("Successfully initialized monofs project");
        }
        Some(MonofsSubcommand::Detach { mount_dir, force }) => {
            tracing::info!("Detaching monofs project...");
            management::detach_mfs(mount_dir, force).await?;
            tracing::info!("Successfully detached monofs project");
        }
        Some(_) => (), // TODO: implement other subcommands
        None => {
            MonofsArgs::command().print_help()?;
        }
    }

    Ok(())
}

//--------------------------------------------------------------------------------------------------
// Functions: *
//--------------------------------------------------------------------------------------------------
