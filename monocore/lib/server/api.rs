use axum::{extract::State, routing::post, Json, Router};

use std::{net::SocketAddr, path::PathBuf, sync::Arc};

use crate::{
    config::{DEFAULT_CONFIG, DEFAULT_SERVER_NAMESPACE},
    management::orchestra,
    server::data::{DownRequest, ErrorResponse, ErrorType, StatusResponse, UpRequest},
    utils::{self, MONOCORE_CONFIG_FILENAME},
    MonocoreError, MonocoreResult,
};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Server configuration for the Monocore API server
#[derive(Clone)]
pub struct SandboxServer {
    namespace_dir: PathBuf,
    enable_default_namespace: bool,
    addr: SocketAddr,
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
    ) -> MonocoreResult<Self> {
        let server = Self {
            namespace_dir: namespace_dir
                .unwrap_or_else(|| utils::get_monocore_home_path().join(utils::NAMESPACES_SUBDIR)),
            enable_default_namespace,
            addr,
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

        let app = Router::new()
            .route("/up", post(Self::up))
            .route("/down", post(Self::down))
            .with_state(state);

        tracing::info!("Server listening on {}", self.addr);

        axum::serve(
            tokio::net::TcpListener::bind(self.addr).await?,
            app.into_make_service(),
        )
        .await?;

        Ok(())
    }

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
}
