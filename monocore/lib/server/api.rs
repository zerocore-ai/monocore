use axum::{
    body::Body,
    extract::State,
    http::{header, Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};

use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, path::PathBuf, sync::Arc};

use crate::{
    config::{DEFAULT_CONFIG, DEFAULT_SERVER_NAMESPACE},
    management::{orchestra, server::API_KEY_PREFIX},
    server::data::{DownRequest, ErrorResponse, ErrorType, StatusResponse, UpRequest},
    utils::{self, MONOCORE_CONFIG_FILENAME},
    MonocoreError, MonocoreResult,
};

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

/// Default JWT header for HS256 algorithm in base64
const DEFAULT_JWT_HEADER: &str = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9";

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Server configuration for the Monocore API server
#[derive(Clone)]
pub struct SandboxServer {
    /// Directory for storing namespaces
    namespace_dir: PathBuf,

    /// Whether to enable the default namespace
    enable_default_namespace: bool,

    /// Address to listen on
    addr: SocketAddr,

    /// JWT authentication key
    key: Option<String>,
}

/// JWT Claims structure for API authentication
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    /// Expiration time (unix timestamp)
    pub exp: u64,

    /// Issued at (unix timestamp)
    pub iat: u64,
}

/// Type alias for the standard API response
type ApiResponse<T> = Result<Json<T>, Json<ErrorResponse>>;

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl SandboxServer {
    /// Create a new server instance
    pub fn new(
        namespace_dir: Option<PathBuf>,
        enable_default_namespace: bool,
        addr: SocketAddr,
        key: Option<String>,
    ) -> MonocoreResult<Self> {
        let server = Self {
            namespace_dir: namespace_dir
                .unwrap_or_else(|| utils::get_monocore_home_path().join(utils::NAMESPACES_SUBDIR)),
            enable_default_namespace,
            addr,
            key,
        };

        // Create default namespace directory and Sandboxfile if enabled
        if enable_default_namespace {
            let default_namespace_path = server.get_namespace_path(None)?;
            let default_config_path = default_namespace_path.join(MONOCORE_CONFIG_FILENAME);

            if !default_config_path.exists() {
                std::fs::write(default_config_path, DEFAULT_CONFIG)?;
                tracing::info!("Created default Sandboxfile in default namespace");
            }
        }

        Ok(server)
    }

    /// Start the server on the specified address
    pub async fn serve(&self) -> anyhow::Result<()> {
        // Create shared application state
        let state = Arc::new(self.clone());

        // Build the router with standard endpoints
        let mut app = Router::new()
            .route("/up", post(up))
            .route("/down", post(down))
            .with_state(state.clone());

        // Add JWT authentication if secure mode is enabled
        if self.key.is_some() {
            tracing::info!("Server running in secure mode with API key authentication");
            // Add authentication middleware to all routes
            app = app.layer(middleware::from_fn_with_state(state, auth_middleware));
        }

        tracing::info!("Server listening on {}", self.addr);

        axum::serve(
            tokio::net::TcpListener::bind(self.addr).await?,
            app.into_make_service(),
        )
        .await?;

        Ok(())
    }

    /// Get the path to a namespace directory, creating it if it doesn't exist
    fn get_namespace_path(&self, namespace: Option<String>) -> MonocoreResult<PathBuf> {
        let namespace = namespace.unwrap_or_else(|| DEFAULT_SERVER_NAMESPACE.to_string());

        // If default namespace is disabled and this is the default namespace, return error
        if !self.enable_default_namespace && namespace == DEFAULT_SERVER_NAMESPACE {
            return Err(MonocoreError::InvalidArgument(
                "Default namespace is disabled".to_string(),
            ));
        }

        let namespace_path = self.namespace_dir.join(namespace);
        if !namespace_path.exists() {
            std::fs::create_dir_all(&namespace_path)?;
        }
        Ok(namespace_path)
    }

    /// Get the server's JWT authentication key
    fn get_jwt_key(&self) -> MonocoreResult<String> {
        if let Some(key) = &self.key {
            return Ok(key.clone());
        }

        // Otherwise, we're likely a client process or the key wasn't set on startup
        Err(MonocoreError::SandboxServerError(
            "Server is not running in secure mode".to_string(),
        ))
    }
}

//--------------------------------------------------------------------------------------------------
// Functions: Middleware
//--------------------------------------------------------------------------------------------------

/// Authentication middleware to validate JWT tokens
async fn auth_middleware(
    State(state): State<Arc<SandboxServer>>,
    req: Request<Body>,
    next: Next,
) -> Response {
    // Extract the token from the Authorization header
    let token = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|auth_header| auth_header.to_str().ok())
        .and_then(|auth_value| auth_value.strip_prefix("Bearer "))
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse::new(
                    401,
                    "Missing or invalid Bearer token".to_string(),
                    ErrorType::AuthenticationError,
                )),
            )
                .into_response()
        });

    // If token extraction failed, return the error response
    let token = match token {
        Ok(t) => t.to_string(), // Convert to owned String
        Err(response) => return response,
    };

    // Convert the custom API key to a standard JWT
    let jwt_token = match convert_api_key_to_jwt(&token) {
        Ok(jwt) => jwt,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse::new(
                    401,
                    "Invalid token format".to_string(),
                    ErrorType::AuthenticationError,
                )),
            )
                .into_response();
        }
    };

    // Get the JWT key - this should be in the state
    let key = match state.get_jwt_key() {
        Ok(key) => key,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    500,
                    "Server authentication configuration error".to_string(),
                    ErrorType::AuthenticationError,
                )),
            )
                .into_response();
        }
    };

    // Validate the JWT token
    match decode::<Claims>(
        &jwt_token,
        &DecodingKey::from_secret(key.as_bytes()),
        &Validation::default(),
    ) {
        Ok(_) => {
            // Token is valid, proceed with the request
            next.run(req).await
        }
        Err(err) => {
            let (status, message) = match err.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => {
                    (StatusCode::UNAUTHORIZED, "Token expired".to_string())
                }
                jsonwebtoken::errors::ErrorKind::InvalidToken => {
                    (StatusCode::UNAUTHORIZED, "Invalid token".to_string())
                }
                _ => (
                    StatusCode::UNAUTHORIZED,
                    format!("Token validation error: {}", err),
                ),
            };

            (
                status,
                Json(ErrorResponse::new(
                    status.as_u16() as u16,
                    message,
                    ErrorType::AuthenticationError,
                )),
            )
                .into_response()
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Functions: Handlers
//--------------------------------------------------------------------------------------------------

/// Handler for starting sandboxes
async fn up(
    State(state): State<Arc<SandboxServer>>,
    Json(request): Json<UpRequest>,
) -> ApiResponse<StatusResponse> {
    tracing::info!("Received up request: {:?}", request);
    let namespace_path = state.get_namespace_path(request.namespace).map_err(|e| {
        Json(
            ErrorResponse::new(
                400,
                "Invalid namespace".to_string(),
                ErrorType::NamespaceError,
            )
            .with_details(e.to_string()),
        )
    })?;

    orchestra::up(
        request.sandboxes.clone(),
        Some(&namespace_path),
        request.config_file.as_deref(),
    )
    .await
    .map_err(|e| {
        tracing::error!("Failed to start sandboxes: {}", e);
        Json(
            ErrorResponse::new(
                500,
                "Failed to start sandboxes".to_string(),
                ErrorType::SandboxError,
            )
            .with_details(e.to_string()),
        )
    })?;

    Ok(Json(StatusResponse::ok()))
}

/// Handler for stopping sandboxes
async fn down(
    State(state): State<Arc<SandboxServer>>,
    Json(request): Json<DownRequest>,
) -> ApiResponse<StatusResponse> {
    tracing::info!("Received down request: {:?}", request);
    let namespace_path = state.get_namespace_path(request.namespace).map_err(|e| {
        Json(
            ErrorResponse::new(
                400,
                "Invalid namespace".to_string(),
                ErrorType::NamespaceError,
            )
            .with_details(e.to_string()),
        )
    })?;

    orchestra::down(
        request.sandboxes.clone(),
        Some(&namespace_path),
        request.config_file.as_deref(),
    )
    .await
    .map_err(|e| {
        tracing::error!("Failed to stop sandboxes: {}", e);
        Json(
            ErrorResponse::new(
                500,
                "Failed to stop sandboxes".to_string(),
                ErrorType::SandboxError,
            )
            .with_details(e.to_string()),
        )
    })?;

    Ok(Json(StatusResponse::ok()))
}

//--------------------------------------------------------------------------------------------------
// Functions: Helpers
//--------------------------------------------------------------------------------------------------

/// Convert a custom API key back to a standard JWT token
/// Takes our custom API key format (API_KEY_PREFIX_<payload>.<signature>) and
/// returns a standard JWT token (<header>.<payload>.<signature>)
fn convert_api_key_to_jwt(api_key: &str) -> Result<String, StatusCode> {
    // Check if the API key uses our custom format
    if !api_key.starts_with(API_KEY_PREFIX) {
        // If it doesn't use our format, assume it's already a JWT
        return Ok(api_key.to_string());
    }

    // Strip the prefix from our custom format
    let without_prefix = api_key.strip_prefix(API_KEY_PREFIX).unwrap();

    // Split the remaining token into parts
    let parts: Vec<&str> = without_prefix.split('.').collect();
    if parts.len() != 2 {
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Reconstruct the JWT format with a default header
    Ok(format!("{}.{}.{}", DEFAULT_JWT_HEADER, parts[0], parts[1]))
}
