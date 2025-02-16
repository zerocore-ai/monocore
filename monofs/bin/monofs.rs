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
            tracing::info!("initializing monofs...");
            management::init_mfs(mount_dir).await?;
            tracing::info!("successfully initialized monofs");
        }
        Some(MonofsSubcommand::Detach { mount_dir, force }) => {
            tracing::info!("detaching monofs...");
            management::detach_mfs(mount_dir, force).await?;
            tracing::info!("successfully detached monofs");
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
