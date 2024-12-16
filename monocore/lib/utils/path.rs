use std::path::PathBuf;

use crate::config::DEFAULT_MONOCORE_HOME;

use super::MONOCORE_HOME_ENV_VAR;

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

/// The sub directory where monocore artifacts, configs, etc are stored.
pub const MONOCORE_SUBDIR: &str = ".monocore";

/// The OCI sub directory where the OCI image layers, index, configurations, etc are stored.
pub const OCI_SUBDIR: &str = "oci";

/// The sub directory where monocore OCI image layers are cached.
pub const OCI_LAYER_SUBDIR: &str = "layer";

/// The sub directory where monocore OCI image index, configurations, etc. are cached.
pub const OCI_REPO_SUBDIR: &str = "repo";

/// The filename for the OCI image index JSON file
pub const OCI_INDEX_FILENAME: &str = "index.json";

/// The filename for the OCI image manifest JSON file
pub const OCI_MANIFEST_FILENAME: &str = "manifest.json";

/// The filename for the OCI image config JSON file
pub const OCI_CONFIG_FILENAME: &str = "config.json";

/// The filename for the supervisors log file
pub const SUPERVISORS_LOG_FILENAME: &str = "supervisors.log";

/// The rootfs sub directory where the rootfs and other related files associated with the microvm are stored.
pub const ROOTFS_SUBDIR: &str = "rootfs";

/// The reference sub directory where the reference rootfs is stored.
pub const REFERENCE_SUBDIR: &str = "reference";

/// The services sub directory where the services (rootfs) are stored.
pub const SERVICE_SUBDIR: &str = "service";

/// The merged sub directory where the merged rootfs is stored.
pub const MERGED_SUBDIR: &str = "merged";

/// The sub directory where runtime state is stored.
pub const STATE_SUBDIR: &str = "run";

/// The sub directory where runtime logs are stored.
pub const LOG_SUBDIR: &str = "log";

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Returns the path where all monocore artifacts, configs, etc are stored.
pub fn monocore_home_path() -> PathBuf {
    if let Ok(monocore_home) = std::env::var(MONOCORE_HOME_ENV_VAR) {
        PathBuf::from(monocore_home)
    } else {
        DEFAULT_MONOCORE_HOME.to_owned()
    }
}

/// Checks if two paths conflict (one is a parent/child of the other or they are the same)
pub fn paths_overlap(path1: &str, path2: &str) -> bool {
    let path1 = if path1.ends_with('/') {
        path1.to_string()
    } else {
        format!("{}/", path1)
    };
    let path2 = if path2.ends_with('/') {
        path2.to_string()
    } else {
        format!("{}/", path2)
    };

    path1.starts_with(&path2) || path2.starts_with(&path1)
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paths_overlap() {
        // Test cases that should conflict
        assert!(paths_overlap("/data", "/data"));
        assert!(paths_overlap("/data", "/data/app"));
        assert!(paths_overlap("/data/app", "/data"));
        assert!(paths_overlap("/data/app/logs", "/data/app"));

        // Test cases that should not conflict
        assert!(!paths_overlap("/data", "/database"));
        assert!(!paths_overlap("/var/log", "/var/lib"));
        assert!(!paths_overlap("/data/app1", "/data/app2"));
        assert!(!paths_overlap("/data/app/logs", "/data/web/logs"));
    }
}
