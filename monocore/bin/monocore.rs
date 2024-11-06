use clap::{CommandFactory, Parser};
use monocore::{
    cli::{MonocoreArgs, MonocoreSubcommand},
    MonocoreResult,
};

//--------------------------------------------------------------------------------------------------
// Function: main
//--------------------------------------------------------------------------------------------------

#[tokio::main]
async fn main() -> MonocoreResult<()> {
    tracing_subscriber::fmt::init();

    // Parse command line arguments
    match MonocoreArgs::parse().subcommand {
        Some(MonocoreSubcommand::Up { file, group }) => {
            println!(
                "up command coming soon: up file={:?}, group={:?}",
                file, group
            );
        }
        Some(MonocoreSubcommand::Down { file, group }) => {
            println!(
                "down command coming soon: down file={:?}, group={:?}",
                file, group
            );
        }
        Some(MonocoreSubcommand::Status {}) => {
            println!("status command coming soon");
        }
        Some(MonocoreSubcommand::Push {}) => {
            println!("push command coming soon");
        }
        Some(MonocoreSubcommand::Pull { image }) => {
            println!("pull command coming soon: image={:?}", image);
        }
        Some(MonocoreSubcommand::Run {}) => {
            println!("run command coming soon");
        }
        Some(MonocoreSubcommand::Install {}) => {
            println!("install command coming soon");
        }
        Some(MonocoreSubcommand::Init {}) => {
            println!("init command coming soon");
        }
        Some(MonocoreSubcommand::Agent {}) => {
            println!("agent command coming soon");
        }
        None => MonocoreArgs::command().print_help()?,
    }

    Ok(())
}
