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
            management::init_fs(mount_dir).await?;
            tracing::info!("Successfully initialized monofs project");
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
