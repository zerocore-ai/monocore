use monocore::{
    group::{GroupConfig, GroupServer},
    MonocoreResult,
};

//--------------------------------------------------------------------------------------------------
// Main
//--------------------------------------------------------------------------------------------------

#[tokio::main]
async fn main() -> MonocoreResult<()> {
    tracing_subscriber::fmt::init();

    let server = GroupServer::new(GroupConfig::default());
    server.start().await?;

    Ok(())
}
