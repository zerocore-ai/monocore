use std::path::PathBuf;

use home::home_dir;

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

/// The sub directory where monocore artifacts, configs, etc are stored.
pub const MONOCORE_SUBDIR: &str = ".monocore";

/// The sub directory where monocore OCI image layers are cached.
pub const IMAGE_LAYERS_SUBDIR: &str = "layers";

/// The sub directory where monocore OCI image index configurations are cached.
pub const IMAGE_DESCRIPTION_SUBDIR: &str = "descriptions";

lazy_static::lazy_static! {
    /// The path where all monocore artifacts, configs, etc are stored.
    pub static ref MONOCORE_PATH: PathBuf = home_dir().unwrap().join(MONOCORE_SUBDIR);

    /// The path where all monocore OCI image layers are cached.
    pub static ref MONOCORE_IMAGE_LAYERS_PATH: PathBuf = MONOCORE_PATH.join(IMAGE_LAYERS_SUBDIR);
}
