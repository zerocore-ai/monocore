//! Route definitions for the HTTP server.
//!
//! This module sets up the routing for the REST API endpoints.

use axum::{
    routing::{get, post},
    Router,
};

use super::{handlers, state::ServerState};

//-------------------------------------------------------------------------------------------------
// Functions
//-------------------------------------------------------------------------------------------------

/// Creates a new router with all API endpoints configured
///
/// ## Arguments
/// * `state` - The shared server state
///
/// # Returns
/// A configured Router instance
pub fn create_router(state: ServerState) -> Router {
    Router::new()
        .route("/up", post(handlers::up_handler))
        .route("/down", post(handlers::down_handler))
        .route("/status", get(handlers::status_handler))
        .route("/remove", post(handlers::remove_handler))
        .with_state(state)
}
