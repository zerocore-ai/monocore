use axum::{routing, Router};
use tokio::net::TcpListener;

use crate::MonocoreResult;

use super::GroupConfig;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A server for managing a group of services.
pub struct GroupServer {
    /// The meta config for the group server.
    config: GroupConfig,
    // /// The services in the group.
    // services: Arc<Mutex<HashMap<String, Service>>>,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl GroupServer {
    /// Creates a new group server.
    pub fn new(config: GroupConfig) -> Self {
        Self {
            config,
            // services: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Starts the group server.
    pub async fn start(&self) -> MonocoreResult<()> {
        let app = self.routes();
        let listener = TcpListener::bind((self.config.get_ip(), self.config.get_port())).await?;
        tracing::info!("group server listening on {}", listener.local_addr()?);
        axum::serve(listener, app).await?;

        Ok(())
    }

    fn routes(&self) -> Router {
        Router::new()
            .route("/up", routing::post(up_handler))
            .route("/down", routing::post(down_handler))
    }
}

//--------------------------------------------------------------------------------------------------
// Functions: Handlers
//--------------------------------------------------------------------------------------------------

async fn up_handler() -> &'static str {
    "Up"
}

async fn down_handler() -> &'static str {
    "Down"
}
