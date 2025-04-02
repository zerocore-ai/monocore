//! Configuration management for the Monocore runtime.
//!
//! This module provides structures and utilities for modifying Monocore
//! configuration.

use nondestructive::yaml;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use tokio::fs;
use typed_path::Utf8UnixPathBuf;

use crate::{
    config::{Monocore, PathSegment, DEFAULT_SHELL},
    utils::MONOCORE_CONFIG_FILENAME,
    MonocoreError, MonocoreResult,
};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

#[derive(Debug, Clone)]
/// The component to add to the Monocore configuration.
pub enum Component {
    /// A sandbox component.
    Sandbox {
        /// The image to use for the sandbox.
        image: String,

        /// The amount of RAM in MiB to use.
        ram: Option<u32>,

        /// The number of CPUs to use.
        cpus: Option<u32>,

        /// The volumes to mount.
        volumes: Vec<String>,

        /// The ports to expose.
        ports: Vec<String>,

        /// The environment variables to use.
        envs: Vec<String>,

        /// The environment file to use.
        env_file: Option<Utf8UnixPathBuf>,

        /// The dependencies to use for the sandbox.
        depends_on: Vec<String>,

        /// The working directory to use for the sandbox.
        workdir: Option<Utf8UnixPathBuf>,

        /// The shell to use for the sandbox.
        shell: Option<String>,

        /// The scripts to use for the sandbox.
        scripts: HashMap<String, String>,

        /// The imports to use for the sandbox.
        imports: HashMap<String, Utf8UnixPathBuf>,

        /// The exports to use for the sandbox.
        exports: HashMap<String, Utf8UnixPathBuf>,

        /// The network reach to use for the sandbox.
        reach: Option<String>,
    },
    /// A build component.
    Build {},
    /// A group component.
    Group {},
}

/// The type of component to add to the Monocore configuration.
#[derive(Debug, Clone)]
pub enum ComponentType {
    /// A sandbox component.
    Sandbox,
    /// A build component.
    Build,
    /// A group component.
    Group,
}

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Adds one or more components to the Monocore configuration.
///
/// Modifies the Monocore configuration file by adding new components while preserving
/// the existing formatting and structure.
///
/// ## Arguments
///
/// * `names` - Names for the components to add
/// * `component` - The component specification to add
/// * `project_dir` - Optional project directory path (defaults to current directory)
/// * `config_file` - Optional config file path (defaults to standard filename)
///
/// ## Returns
///
/// * `Ok(())` on success, or error if the file cannot be found/read/written,
///   contains invalid YAML, or a component with the same name already exists
pub async fn add(
    names: &[String],
    component: &Component,
    project_dir: Option<&Path>,
    config_file: Option<&str>,
) -> MonocoreResult<()> {
    let (_, _, full_config_path) = resolve_config_paths(project_dir, config_file).await?;

    // Read the configuration file content
    let config_contents = fs::read_to_string(&full_config_path).await?;

    // Parse the YAML document using nondestructive
    let mut doc = yaml::from_slice(config_contents.as_bytes())
        .map_err(|e| MonocoreError::ConfigParseError(e.to_string()))?;

    for name in names {
        match component {
            Component::Sandbox {
                image,
                ram,
                cpus,
                volumes,
                ports,
                envs,
                env_file,
                depends_on,
                workdir,
                shell,
                scripts,
                imports,
                exports,
                reach,
            } => {
                let doc_mut = doc.as_mut();
                let mut root_mapping = doc_mut.make_mapping();

                // Ensure the "sandboxes" key exists in the root mapping
                let mut sandboxes_mapping =
                    if let Some(sandboxes_mut) = root_mapping.get_mut("sandboxes") {
                        // Get the existing sandboxes mapping
                        sandboxes_mut.make_mapping()
                    } else {
                        // Create a new sandboxes mapping if it doesn't exist
                        root_mapping
                            .insert("sandboxes", yaml::Separator::Auto)
                            .make_mapping()
                    };

                // Check if the sandbox already exists by trying to get it
                if sandboxes_mapping.get_mut(name).is_some() {
                    return Err(MonocoreError::ConfigValidation(format!(
                        "Sandbox with name '{}' already exists",
                        name
                    )));
                }

                // Create a new sandbox mapping
                let mut sandbox_mapping = sandboxes_mapping
                    .insert(name, yaml::Separator::Auto)
                    .make_mapping();

                // Add image field (required)
                sandbox_mapping.insert_str("image", image.to_string());

                // Add optional fields
                if let Some(ram_value) = ram {
                    sandbox_mapping.insert_u32("ram", *ram_value);
                }

                if let Some(cpus_value) = cpus {
                    sandbox_mapping.insert_u32("cpus", *cpus_value as u32);
                }

                // Add shell (default if not provided)
                if let Some(shell_value) = shell {
                    sandbox_mapping.insert_str("shell", shell_value);
                } else if sandbox_mapping.get_mut("shell").is_none() {
                    sandbox_mapping.insert_str("shell", DEFAULT_SHELL);
                }

                // Add volumes if any
                if !volumes.is_empty() {
                    let mut volumes_sequence = sandbox_mapping
                        .insert("volumes", yaml::Separator::Auto)
                        .make_sequence();

                    for volume in volumes {
                        volumes_sequence.push_string(volume);
                    }
                }

                // Add ports if any
                if !ports.is_empty() {
                    let mut ports_sequence = sandbox_mapping
                        .insert("ports", yaml::Separator::Auto)
                        .make_sequence();

                    for port in ports {
                        ports_sequence.push_string(port);
                    }
                }

                // Add env vars if any
                if !envs.is_empty() {
                    let mut envs_sequence = sandbox_mapping
                        .insert("envs", yaml::Separator::Auto)
                        .make_sequence();

                    for env in envs {
                        envs_sequence.push_string(env);
                    }
                }

                // Add env_file if provided
                if let Some(env_file_path) = env_file {
                    sandbox_mapping.insert_str("env_file", env_file_path.to_string());
                }

                // Add depends_on if any
                if !depends_on.is_empty() {
                    let mut depends_on_sequence = sandbox_mapping
                        .insert("depends_on", yaml::Separator::Auto)
                        .make_sequence();

                    for dep in depends_on {
                        depends_on_sequence.push_string(dep);
                    }
                }

                // Add workdir if provided
                if let Some(workdir_path) = workdir {
                    sandbox_mapping.insert_str("workdir", workdir_path.to_string());
                }

                // Add scripts if any
                if !scripts.is_empty() {
                    let mut scripts_mapping = sandbox_mapping
                        .insert("scripts", yaml::Separator::Auto)
                        .make_mapping();

                    for (script_name, script_content) in scripts {
                        scripts_mapping.insert_str(script_name, script_content);
                    }
                }

                // Add imports if any
                if !imports.is_empty() {
                    let mut imports_mapping = sandbox_mapping
                        .insert("imports", yaml::Separator::Auto)
                        .make_mapping();

                    for (import_name, import_path) in imports {
                        imports_mapping.insert_str(import_name, import_path.to_string());
                    }
                }

                // Add exports if any
                if !exports.is_empty() {
                    let mut exports_mapping = sandbox_mapping
                        .insert("exports", yaml::Separator::Auto)
                        .make_mapping();

                    for (export_name, export_path) in exports {
                        exports_mapping.insert_str(export_name, export_path.to_string());
                    }
                }

                // Add network reach if provided
                if let Some(reach_value) = reach {
                    let mut network_mapping = sandbox_mapping
                        .insert("network", yaml::Separator::Auto)
                        .make_mapping();

                    network_mapping.insert_str("reach", reach_value);
                }
            }
            Component::Build {} => {}
            Component::Group {} => {}
        }
    }

    // Write the modified YAML back to the file, preserving formatting
    let modified_content = doc.to_string();

    // TODO: Validate config before writing
    fs::write(full_config_path, modified_content).await?;

    Ok(())
}

/// Removes a component from the Monocore configuration.
///
/// Modifies the Monocore configuration file by removing an existing component
/// while preserving the existing formatting and structure.
///
/// ## Arguments
///
/// * `component` - The component to remove from the configuration
///
/// ## Returns
///
/// * `Ok(())` on success, or error if the file cannot be found/read/written,
///   contains invalid YAML, or the component does not exist
///
/// Note: This function is currently a placeholder and needs to be implemented.
pub async fn remove(
    component_type: ComponentType,
    names: &[String],
    project_dir: Option<&Path>,
    config_file: Option<&str>,
) -> MonocoreResult<()> {
    let (_, _, full_config_path) = resolve_config_paths(project_dir, config_file).await?;

    // Read the configuration file content
    let config_contents = fs::read_to_string(&full_config_path).await?;

    let mut doc = yaml::from_slice(config_contents.as_bytes())
        .map_err(|e| MonocoreError::ConfigParseError(e.to_string()))?;

    match component_type {
        ComponentType::Sandbox => {
            let doc_mut = doc.as_mut();
            let mut root_mapping =
                doc_mut
                    .into_mapping_mut()
                    .ok_or(MonocoreError::ConfigParseError(
                        "config is not valid. expected an object".to_string(),
                    ))?;

            // Ensure the "sandboxes" key exists in the root mapping
            let mut sandboxes_mapping =
                if let Some(sandboxes_mut) = root_mapping.get_mut("sandboxes") {
                    // Get the existing sandboxes mapping
                    sandboxes_mut
                        .into_mapping_mut()
                        .ok_or(MonocoreError::ConfigParseError(
                            "sandboxes is not a valid mapping".to_string(),
                        ))?
                } else {
                    // Create a new sandboxes mapping if it doesn't exist
                    root_mapping
                        .insert("sandboxes", yaml::Separator::Auto)
                        .make_mapping()
                };

            for name in names {
                sandboxes_mapping.remove(name);
            }
        }
        _ => (),
    }

    // Write the modified YAML back to the file, preserving formatting
    let modified_content = doc.to_string();

    // TODO: Validate config before writing
    fs::write(full_config_path, modified_content).await?;

    Ok(())
}

/// Lists components in the Monocore configuration.
///
/// Retrieves and displays information about components defined in the Monocore configuration.
///
/// ## Arguments
///
/// * `component_type` - The type of component to list
/// * `project_dir` - Optional path to the project directory. If None, defaults to current directory
/// * `config_file` - Optional path to the Monocore config file. If None, uses default filename
///
/// ## Returns
///
/// * `Ok(())` on success, or error if the file cannot be found/read/written,
///   contains invalid YAML, or the component does not exist
pub async fn list(
    component_type: ComponentType,
    project_dir: Option<&Path>,
    config_file: Option<&str>,
) -> MonocoreResult<Vec<String>> {
    let (config, _, _) = load_config(project_dir, config_file).await?;

    match component_type {
        ComponentType::Sandbox => {
            return Ok(config.get_sandboxes().keys().cloned().collect());
        }
        _ => return Ok(vec![]),
    }
}

//--------------------------------------------------------------------------------------------------
// Functions: Helpers
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
    project_dir: Option<&Path>,
    config_file: Option<&str>,
) -> MonocoreResult<(Monocore, PathBuf, String)> {
    // Get the target path, defaulting to current directory if none specified
    let project_dir = project_dir.unwrap_or_else(|| Path::new("."));
    let canonical_project_dir = fs::canonicalize(project_dir).await?;

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

    // Check if config file exists
    if !full_config_path.exists() {
        return Err(MonocoreError::MonocoreConfigNotFound(
            project_dir.display().to_string(),
        ));
    }

    Ok((
        canonical_project_dir,
        config_file.to_string(),
        full_config_path,
    ))
}
