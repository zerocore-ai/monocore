//! Type definitions for the server module.
//!
//! This module contains request and response types used by the REST API endpoints.

use serde::{Deserialize, Serialize};

use crate::config::Monocore;

//-------------------------------------------------------------------------------------------------
// Types
//-------------------------------------------------------------------------------------------------

/// Request body for the /up endpoint
#[derive(Debug, Deserialize)]
pub struct UpRequest {
    /// The monocore configuration to apply
    pub config: Monocore,
    /// Optional group name to only start services in that group
    pub group: Option<String>,
}

/// Request body for the /down endpoint
#[derive(Debug, Deserialize)]
pub struct DownRequest {
    /// Optional group name to only stop services in that group
    pub group: Option<String>,
}

/// Request body for the /remove endpoint
#[derive(Debug, Deserialize)]
pub struct RemoveRequest {
    /// List of service names to remove
    pub services: Vec<String>,
    /// Optional group name to remove all services in that group
    pub group: Option<String>,
}

/// Response body for the /status endpoint
#[derive(Debug, Serialize)]
pub struct StatusResponse {
    /// List of service statuses
    pub services: Vec<ServiceStatus>,
}

/// Status information for a single service
#[derive(Debug, Serialize)]
pub struct ServiceStatus {
    /// Name of the service
    pub name: String,
    /// Group the service belongs to
    pub group: Option<String>,
    /// Current status of the service
    pub status: String,
    /// Process ID of the service if running
    pub pid: Option<u32>,
    /// Resource usage metrics
    pub metrics: ServiceMetrics,
}

/// Resource usage metrics for a service
#[derive(Debug, Serialize)]
pub struct ServiceMetrics {
    /// CPU usage as a percentage
    pub cpu_usage: f64,
    /// Memory usage in bytes
    pub memory_usage: u64,
}

/// Error response returned when an operation fails
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    /// Error message describing what went wrong
    pub error: String,
}

/// Response body for the /up endpoint
#[derive(Debug, Serialize)]
pub struct UpResponse {
    /// List of services that were started
    pub started_services: Vec<String>,
}

/// Response body for the /down endpoint
#[derive(Debug, Serialize)]
pub struct DownResponse {
    /// List of services that were stopped
    pub stopped_services: Vec<String>,
}

/// Response body for the /remove endpoint
#[derive(Debug, Serialize)]
pub struct RemoveResponse {
    /// List of services that were removed
    pub removed_services: Vec<String>,
}
