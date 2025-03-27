use serde::{Deserialize, Serialize};

//--------------------------------------------------------------------------------------------------
// Types: Requests
//--------------------------------------------------------------------------------------------------

/// Request body for starting a sandbox
#[derive(Debug, Deserialize)]
pub struct UpRequest {
    /// Optional namespace name, defaults to "default" if not specified
    pub namespace: Option<String>,

    /// Optional config file name, defaults to Sandboxfile if not specified
    pub config_file: Option<String>,

    /// List of sandbox names to start
    pub sandboxes: Vec<String>,
}

/// Request body for stopping a sandbox
#[derive(Debug, Deserialize)]
pub struct DownRequest {
    /// Optional namespace name, defaults to "default" if not specified
    pub namespace: Option<String>,

    /// Optional config file name, defaults to Sandboxfile if not specified
    pub config_file: Option<String>,

    /// List of sandbox names to stop
    pub sandboxes: Vec<String>,
}

//--------------------------------------------------------------------------------------------------
// Types: Responses
//--------------------------------------------------------------------------------------------------

/// Response type for status requests
#[derive(Debug, Serialize)]
pub struct StatusResponse {
    /// Message indicating the status of the sandbox operation
    pub message: String,
}

//--------------------------------------------------------------------------------------------------
// Types: Error Response
//--------------------------------------------------------------------------------------------------

/// Standard error response format
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    /// HTTP status code
    pub code: u16,

    /// Error message
    pub message: String,

    /// Error type for categorizing errors
    pub error_type: ErrorType,

    /// Optional additional details about the error
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

/// Types of errors that can occur
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorType {
    /// Invalid request parameters or body
    ValidationError,

    /// Resource not found
    NotFound,

    /// Namespace related errors
    NamespaceError,

    /// Sandbox operation errors
    SandboxError,

    /// Internal server errors
    InternalError,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl StatusResponse {
    /// Create a new status response
    pub fn success(action: &str, sandboxes: Vec<String>) -> Self {
        let sandbox_list = sandboxes.join(", ");
        let message = format!("Successfully {} sandbox(es): {}", action, sandbox_list);
        Self { message }
    }

    /// Create a new status response for a successful operation
    pub fn ok() -> Self {
        Self {
            message: "OK".to_string(),
        }
    }
}

impl ErrorResponse {
    /// Create a new error response
    pub fn new(code: u16, message: String, error_type: ErrorType) -> Self {
        Self {
            code,
            message,
            error_type,
            details: None,
        }
    }

    /// Add details to the error response, ignoring details for 500-level errors
    pub fn with_details(mut self, details: String) -> Self {
        // Only include details for non-500 errors
        if self.code < 500 {
            self.details = Some(details);
        }
        self
    }
}
