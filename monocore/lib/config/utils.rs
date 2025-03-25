//! Configuration utilities for Monocore.
//!
//! This module provides utility functions for working with Monocore configurations,
//! such as loading and validating configuration files.

use std::path::{Path, PathBuf};
use tokio::fs;

use crate::{
    config::{Monocore, PathSegment},
    utils::MONOCORE_CONFIG_FILENAME,
    MonocoreError, MonocoreResult,
};

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Loads a Monocore configuration from a file.
///
/// This function handles all the common steps for loading a Monocore configuration, including:
/// - Resolving the project directory and config file path
/// - Validating the config file path
/// - Checking if the config file exists
/// - Reading and parsing the config file
///
/// ## Arguments
///
/// * `project_dir` - Optional path to the project directory. If None, defaults to current directory
/// * `config_file` - Optional path to the Monocore config file. If None, uses default filename
///
/// ## Returns
///
/// Returns a tuple containing:
/// - The loaded Monocore configuration
/// - The canonical project directory path
/// - The config file name
///
/// Or a `MonocoreError` if:
/// - The config file path is invalid
/// - The config file does not exist
/// - The config file cannot be read
/// - The config file contains invalid YAML
pub async fn load_config(
    project_dir: Option<PathBuf>,
    config_file: Option<&str>,
) -> MonocoreResult<(Monocore, PathBuf, String)> {
    // Get the target path, defaulting to current directory if none specified
    let project_dir = project_dir.unwrap_or_else(|| PathBuf::from("."));
    let canonical_project_dir = fs::canonicalize(&project_dir).await?;

    // Validate the config file path
    let config_file = config_file.unwrap_or_else(|| MONOCORE_CONFIG_FILENAME);
    let _ = PathSegment::try_from(config_file)?;
    let full_config_path = canonical_project_dir.join(config_file);

    // Check if config file exists
    if !full_config_path.exists() {
        return Err(MonocoreError::MonocoreConfigNotFound(
            project_dir.display().to_string(),
        ));
    }

    // Read and parse the config file
    let config_contents = fs::read_to_string(&full_config_path).await?;
    let config: Monocore = serde_yaml::from_str(&config_contents)?;

    Ok((config, canonical_project_dir, config_file.to_string()))
}

/// Resolves the paths for a Monocore configuration.
///
/// This function is similar to `load_config` but without actually loading the file.
/// It just resolves the paths that would be used.
///
/// ## Arguments
///
/// * `project_dir` - Optional path to the project directory. If None, defaults to current directory
/// * `config_file` - Optional path to the Monocore config file. If None, uses default filename
///
/// ## Returns
///
/// Returns a tuple containing:
/// - The canonical project directory path
/// - The config file name
/// - The full config file path
pub async fn resolve_config_paths(
    project_dir: Option<&Path>,
    config_file: Option<&str>,
) -> MonocoreResult<(PathBuf, String, PathBuf)> {
    // Get the target path, defaulting to current directory if none specified
    let project_dir = project_dir.unwrap_or_else(|| Path::new("."));
    let canonical_project_dir = fs::canonicalize(project_dir).await?;

    // Validate the config file path
    let config_file = config_file.unwrap_or_else(|| MONOCORE_CONFIG_FILENAME);
    let _ = PathSegment::try_from(config_file)?;
    let full_config_path = canonical_project_dir.join(config_file);

    Ok((
        canonical_project_dir,
        config_file.to_string(),
        full_config_path,
    ))
}
