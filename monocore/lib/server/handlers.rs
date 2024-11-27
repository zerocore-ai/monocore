//! HTTP request handlers for the REST API.
//!
//! This module implements the handlers for each API endpoint. The handlers
//! coordinate with the Orchestrator to perform the requested operations.

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};

use super::{
    state::ServerState,
    types::{
        DownRequest, DownResponse, ErrorResponse, RemoveRequest, RemoveResponse, ServiceMetrics,
        ServiceStatus, StatusResponse, UpRequest, UpResponse,
    },
};
use crate::MonocoreResult;

//-------------------------------------------------------------------------------------------------
// Functions: Handlers
//-------------------------------------------------------------------------------------------------

/// Handler for the POST /up endpoint
///
/// Starts services according to the provided configuration
pub async fn up_handler(
    State(state): State<ServerState>,
    Json(req): Json<UpRequest>,
) -> impl IntoResponse {
    match handle_up(state, req).await {
        Ok(started) => (
            StatusCode::OK,
            Json(UpResponse {
                started_services: started,
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

/// Handler for the POST /down endpoint
///
/// Stops running services, optionally filtered by group
pub async fn down_handler(
    State(state): State<ServerState>,
    Json(req): Json<DownRequest>,
) -> impl IntoResponse {
    match handle_down(state, req).await {
        Ok(stopped) => (
            StatusCode::OK,
            Json(DownResponse {
                stopped_services: stopped,
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

/// Handler for the GET /status endpoint
///
/// Returns status information for all running services
pub async fn status_handler(State(state): State<ServerState>) -> impl IntoResponse {
    match handle_status(state).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

/// Handler for the POST /remove endpoint
///
/// Removes service files for specified services or group
pub async fn remove_handler(
    State(state): State<ServerState>,
    Json(req): Json<RemoveRequest>,
) -> impl IntoResponse {
    match handle_remove(state, req).await {
        Ok(removed) => (
            StatusCode::OK,
            Json(RemoveResponse {
                removed_services: removed,
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

/// Implementation of the up operation
async fn handle_up(state: ServerState, req: UpRequest) -> MonocoreResult<Vec<String>> {
    let mut orchestrator = state.orchestrator().write().await;

    // Get list of running services before startup
    let running_before: Vec<String> = orchestrator
        .get_running_services()
        .keys()
        .cloned()
        .collect();

    // Start services
    orchestrator.up(req.config).await?;

    // Get list of running services after startup
    let running_after: Vec<String> = orchestrator
        .get_running_services()
        .keys()
        .cloned()
        .collect();

    // Return list of services that were newly started
    Ok(running_after
        .into_iter()
        .filter(|s| !running_before.contains(s))
        .collect())
}

/// Implementation of the down operation
async fn handle_down(state: ServerState, req: DownRequest) -> MonocoreResult<Vec<String>> {
    let mut orchestrator = state.orchestrator().write().await;

    // Get list of running services before shutdown
    let running_before: Vec<String> = orchestrator
        .get_running_services()
        .keys()
        .cloned()
        .collect();

    // Stop services
    orchestrator.down(req.group.as_deref()).await?;

    // Get list of running services after shutdown
    let running_after: Vec<String> = orchestrator
        .get_running_services()
        .keys()
        .cloned()
        .collect();

    // Return list of services that were actually stopped
    Ok(running_before
        .into_iter()
        .filter(|s| !running_after.contains(s))
        .collect())
}

/// Implementation of the status operation
async fn handle_status(state: ServerState) -> MonocoreResult<StatusResponse> {
    let orchestrator = state.orchestrator().read().await;
    let statuses = orchestrator.status().await?;

    Ok(StatusResponse {
        services: statuses
            .into_iter()
            .map(|s| ServiceStatus {
                name: s.get_name().to_string(),
                group: Some(s.get_state().get_group().get_name().to_string()),
                status: format!("{:?}", s.get_state().get_status()),
                pid: *s.get_pid(),
                metrics: ServiceMetrics {
                    cpu_usage: s.get_state().get_metrics().get_cpu_usage() as f64,
                    memory_usage: s.get_state().get_metrics().get_memory_usage(),
                },
            })
            .collect(),
    })
}

/// Implementation of the remove operation
async fn handle_remove(state: ServerState, req: RemoveRequest) -> MonocoreResult<Vec<String>> {
    let mut orchestrator = state.orchestrator().write().await;
    let services_before: Vec<String> = orchestrator
        .status()
        .await?
        .into_iter()
        .map(|s| s.name)
        .collect();

    match (req.services.is_empty(), req.group) {
        (false, None) => orchestrator.remove_services(&req.services).await?,
        (true, Some(group)) => orchestrator.remove_group(&group).await?,
        _ => {
            return Err(crate::MonocoreError::InvalidArgument(
                "Must specify either services or group".to_string(),
            ))
        }
    }

    let services_after: Vec<String> = orchestrator
        .status()
        .await?
        .into_iter()
        .map(|s| s.name)
        .collect();

    // Return list of services that were actually removed
    Ok(services_before
        .into_iter()
        .filter(|s| !services_after.contains(s))
        .collect())
}
