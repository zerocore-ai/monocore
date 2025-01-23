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
    // Parse command line arguments
    let args = MonofsArgs::parse();
    match args.subcommand {
        Some(MonofsSubcommand::Init { system_path }) => {
            tracing::info!("Initializing monofs project...");
            management::init_fs(system_path).await?;
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
