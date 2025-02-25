use std::{path::PathBuf, sync::LazyLock};

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

/// The default maximum depth of a symbolic link.
pub const DEFAULT_SYMLINK_DEPTH: u32 = 10;

/// The default host address to bind to.
pub const DEFAULT_HOST: &str = "127.0.0.1";

/// The default NFS port number to use.
pub const DEFAULT_NFS_PORT: u32 = 2049;

/// The default path for the mfsrun binary.
pub static DEFAULT_MFSRUN_EXE_PATH: LazyLock<PathBuf> = LazyLock::new(|| {
    let current_exe = std::env::current_exe().unwrap();
    current_exe.parent().unwrap().join("mfsrun")
});
