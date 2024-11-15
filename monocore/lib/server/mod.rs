//! Server module for monocore.
//!
//! This module implements a REST API server that provides HTTP endpoints for managing
//! monocore services. The server is stateless and uses an Orchestrator instance for
//! all state management.
//!
//! The server provides the following endpoints:
//! - POST /up - Start services with provided configuration
//! - POST /down - Stop running services
//! - GET /status - Get status of all services
//! - POST /remove - Remove service files

mod handlers;
mod routes;
mod state;
mod types;

//--------------------------------------------------------------------------------------------------
// Exports
//--------------------------------------------------------------------------------------------------

pub use handlers::*;
pub use routes::*;
pub use state::*;
pub use types::*;
