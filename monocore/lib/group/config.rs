use std::net::IpAddr;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// The meta config for the group server.
pub struct GroupConfig {
    /// The ip address to bind to.
    ip: IpAddr,

    /// The port to listen on.
    port: u16,
    // /// The registry to pull images from.
    // registry: String,

    // /// The home directory to use.
    // home_dir: PathBuf,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl GroupConfig {
    /// Gets the ip address to bind to.
    pub fn get_ip(&self) -> IpAddr {
        self.ip
    }

    /// Gets the port to listen on.
    pub fn get_port(&self) -> u16 {
        self.port
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl Default for GroupConfig {
    fn default() -> Self {
        Self {
            ip: "127.0.0.1".parse().unwrap(),
            port: 6060,
        }
    }
}
