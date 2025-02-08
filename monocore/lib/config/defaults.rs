use std::{path::PathBuf, sync::LazyLock};

use crate::utils::MONOCORE_HOME_DIR;

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

/// The default number of vCPUs to use for the MicroVm.
pub const DEFAULT_NUM_VCPUS: u8 = 1;

/// The default amount of RAM in MiB to use for the MicroVm.
pub const DEFAULT_RAM_MIB: u32 = 1024;

/// The path where all monocore global data is stored.
pub static DEFAULT_MONOCORE_HOME: LazyLock<PathBuf> =
    LazyLock::new(|| dirs::home_dir().unwrap().join(MONOCORE_HOME_DIR));

/// The default OCI registry domain.
pub const DEFAULT_OCI_REGISTRY: &str = "sandboxes.io";

/// The default OCI reference tag.
pub const DEFAULT_OCI_REFERENCE_TAG: &str = "latest";

/// The default OCI reference repository namespace.
pub const DEFAULT_OCI_REFERENCE_REPO_NAMESPACE: &str = "library";

/// The default configuration file content
pub(crate) const DEFAULT_CONFIG: &str = r#"# Sandbox configurations
sandboxes: []
"#;
