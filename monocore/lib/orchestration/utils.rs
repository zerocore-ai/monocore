use std::{
    collections::{BTreeSet, HashMap, HashSet},
    net::Ipv4Addr,
    path::PathBuf,
};
use tokio::{fs, process::Command};
use tracing::warn;

use crate::{
    config::{Monocore, Service},
    runtime::MicroVmState,
    MonocoreResult,
};

//-------------------------------------------------------------------------------------------------
// Types
//-------------------------------------------------------------------------------------------------

/// Represents the state loaded from state files
pub struct LoadedState {
    pub services: Vec<Service>,
    pub groups: Vec<crate::config::Group>,
    pub running_services: HashMap<String, u32>,
    pub assigned_ips: HashMap<String, Ipv4Addr>,
    pub used_ips: BTreeSet<u8>,
}

//-------------------------------------------------------------------------------------------------
// Functions
//-------------------------------------------------------------------------------------------------

/// Helper function to check if a process is running
pub async fn is_process_running(pid: u32) -> bool {
    Command::new("kill")
        .arg("-0") // Only check process existence
        .arg(pid.to_string())
        .output()
        .await
        .map_or(false, |output| output.status.success())
}

/// Reads state files and reconstructs services, groups and running services
pub async fn load_state_from_files(state_dir: &PathBuf) -> MonocoreResult<LoadedState> {
    let (services, groups, running_services) = load_services_and_groups(state_dir).await?;

    // Convert groups from HashSet to Vec
    let groups = groups.into_iter().collect();

    let (assigned_ips, used_ips) = load_ip_assignments(state_dir).await?;

    Ok(LoadedState {
        services,
        groups,
        running_services,
        assigned_ips,
        used_ips,
    })
}
/// Reads state files and loads services and groups that are still running
pub async fn load_services_and_groups(
    state_dir: &PathBuf,
) -> MonocoreResult<(
    Vec<Service>,
    HashSet<crate::config::Group>,
    HashMap<String, u32>,
)> {
    let mut services = Vec::new();
    let mut groups = HashSet::new();
    let mut running_services = HashMap::new();

    let mut dir = fs::read_dir(state_dir).await?;
    while let Some(entry) = dir.next_entry().await? {
        let path = entry.path();
        if path.is_file() && path.extension().map_or(false, |ext| ext == "json") {
            match fs::read_to_string(&path).await {
                Ok(contents) => match serde_json::from_str::<MicroVmState>(&contents) {
                    Ok(state) => {
                        // Only include if process is still running
                        if let Some(pid) = state.get_pid() {
                            if is_process_running(*pid).await {
                                services.push(state.get_service().clone());
                                groups.insert(state.get_group().clone());
                                running_services
                                    .insert(state.get_service().get_name().to_string(), *pid);
                            } else {
                                // Clean up stale state file
                                if let Err(e) = fs::remove_file(&path).await {
                                    warn!("Failed to remove stale state file {:?}: {}", path, e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to parse state file {:?}: {}", path, e);
                        // Clean up invalid state file
                        if let Err(e) = fs::remove_file(&path).await {
                            warn!("Failed to remove invalid state file {:?}: {}", path, e);
                        }
                    }
                },
                Err(e) => {
                    warn!("Failed to read state file {:?}: {}", path, e);
                }
            }
        }
    }

    Ok((services, groups, running_services))
}

/// Reads state files to reconstruct IP assignments
pub async fn load_ip_assignments(
    state_dir: &PathBuf,
) -> MonocoreResult<(HashMap<String, Ipv4Addr>, BTreeSet<u8>)> {
    let mut assigned_ips = HashMap::new();
    let mut used_ips = BTreeSet::new();

    let mut dir = fs::read_dir(state_dir).await?;
    while let Some(entry) = dir.next_entry().await? {
        let path = entry.path();
        if path.is_file() && path.extension().map_or(false, |ext| ext == "json") {
            match fs::read_to_string(&path).await {
                Ok(contents) => match serde_json::from_str::<MicroVmState>(&contents) {
                    Ok(state) => {
                        if let Some(group_ip) = state.get_group_ip() {
                            assigned_ips
                                .insert(state.get_group().get_name().to_string(), *group_ip);
                            used_ips.insert(group_ip.octets()[3]);
                        }
                    }
                    Err(e) => {
                        warn!("Failed to parse state file {:?}: {}", path, e);
                        if let Err(e) = fs::remove_file(&path).await {
                            warn!("Failed to remove invalid state file {:?}: {}", path, e);
                        }
                    }
                },
                Err(e) => {
                    warn!("Failed to read state file {:?}: {}", path, e);
                }
            }
        }
    }

    Ok((assigned_ips, used_ips))
}

/// Creates a Monocore configuration from loaded services and groups
pub fn create_config_from_state(state: LoadedState) -> MonocoreResult<(Monocore, LoadedState)> {
    let config = Monocore::builder()
        .services(state.services.clone())
        .groups(state.groups.clone())
        .build()?;
    // .build_unchecked();

    Ok((config, state))
}
