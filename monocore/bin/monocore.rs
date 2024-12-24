use std::{env, io::Write};

use clap::{CommandFactory, Parser};
use futures::StreamExt;
use monocore::{
    cli::{MonocoreArgs, MonocoreSubcommand},
    config::Monocore,
    utils::{self, OCI_SUBDIR},
    MonocoreError, MonocoreResult,
};
use serde::de::DeserializeOwned;
use tokio::{fs, io::AsyncWriteExt, process::Command, signal::unix::SignalKind};
use tracing::info;

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

/// The name of the supervisor executable
const SUPERVISOR_EXE: &str = "monokrun";

//--------------------------------------------------------------------------------------------------
// Function: main
//--------------------------------------------------------------------------------------------------

#[tokio::main]
async fn main() -> MonocoreResult<()> {
    // Parse command line arguments
    let args = MonocoreArgs::parse();

    match args.subcommand {
        Some(_) => (),
        None => (),
    }

    MonocoreArgs::command().print_help()?;

    Ok(())
}

//--------------------------------------------------------------------------------------------------
// Function: *
//--------------------------------------------------------------------------------------------------
