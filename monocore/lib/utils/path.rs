use std::path::PathBuf;

use home::home_dir;

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

/// The sub directory where monocore artifacts, configs, etc are stored.
pub const MONOCORE_SUBDIR: &str = ".monocore";

/// The sub directory where monocore OCI image layers are cached.
pub const IMAGE_LAYERS_SUBDIR: &str = "image_layers";

/// The sub directory where monocore OCI image index configurations are cached.
pub const IMAGE_DEFS_SUBDIR: &str = "image_defs";

lazy_static::lazy_static! {
    /// The path where all monocore artifacts, configs, etc are stored.
    pub static ref MONOCORE_PATH: PathBuf = home_dir().unwrap().join(MONOCORE_SUBDIR);
}
