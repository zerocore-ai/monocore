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
    tracing_subscriber::fmt()
        .with_target(false)
        .with_file(false)
        .with_line_number(false)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_level(true)
        .init();

    // Parse command line arguments
    let args = MonocoreArgs::parse();
    match args.subcommand {
        Some(MonocoreSubcommand::Init { path }) => {
            tracing::info!("initializing monocore project: path={path:?}");
            management::init_menv(path).await?;
            tracing::info!("successfully initialized monocore project");
        }
        Some(MonocoreSubcommand::Pull {
            image,
            image_group,
            name,
        }) => {
            tracing::info!("pulling image: name={name}, image={image}, image_group={image_group}");
            management::pull_image(name, image, image_group).await?;
            tracing::info!("successfully pulled image");
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
