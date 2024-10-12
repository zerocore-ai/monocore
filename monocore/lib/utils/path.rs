use std::path::PathBuf;

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
    pub static ref DEFAULT_MONOCORE_HOME: PathBuf = dirs::home_dir().unwrap().join(MONOCORE_SUBDIR);
}

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Returns the path where all monocore artifacts, configs, etc are stored.
#[inline]
pub fn monocore_home_path() -> PathBuf {
    let monocore_home = std::env::var("MONOCORE_HOME").unwrap();
    PathBuf::from(monocore_home)
}
