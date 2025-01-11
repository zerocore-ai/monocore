//! NFSv4 server implementation

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use bytes::BytesMut;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{error, info};

use crate::error::Error;
use crate::nfs::{CompoundReply, CompoundRequest, CompoundResult, NfsOpcode};
use crate::rpc::{MessageType, RpcCall, RpcHeader};
use crate::state::StateManager;
use crate::xdr::{XdrDecode, XdrEncode};
use crate::Result;

/// Trait for implementing filesystem operations
#[async_trait]
pub trait FileSystem: Send + Sync {
    /// Get root filehandle
    async fn root_filehandle(&self) -> Result<Vec<u8>>;

    /// Look up a file by name
    async fn lookup(&self, parent: &[u8], name: &str) -> Result<Vec<u8>>;

    /// Open a file
    async fn open(&self, filehandle: &[u8], access: u32, share: u32) -> Result<()>;

    /// Read from a file
    async fn read(&self, filehandle: &[u8], offset: u64, count: u32) -> Result<Vec<u8>>;

    /// Write to a file
    async fn write(&self, filehandle: &[u8], offset: u64, data: &[u8]) -> Result<u32>;

    /// Close a file
    async fn close(&self, filehandle: &[u8]) -> Result<()>;
}

#[async_trait]
impl<F: FileSystem + Send + Sync> FileSystem for Arc<F> {
    async fn root_filehandle(&self) -> Result<Vec<u8>> {
        (**self).root_filehandle().await
    }

    async fn lookup(&self, parent: &[u8], name: &str) -> Result<Vec<u8>> {
        (**self).lookup(parent, name).await
    }

    async fn open(&self, filehandle: &[u8], access: u32, share: u32) -> Result<()> {
        (**self).open(filehandle, access, share).await
    }

    async fn read(&self, filehandle: &[u8], offset: u64, count: u32) -> Result<Vec<u8>> {
        (**self).read(filehandle, offset, count).await
    }

    async fn write(&self, filehandle: &[u8], offset: u64, data: &[u8]) -> Result<u32> {
        (**self).write(filehandle, offset, data).await
    }

    async fn close(&self, filehandle: &[u8]) -> Result<()> {
        (**self).close(filehandle).await
    }
}

/// NFS server configuration
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Server address to bind to
    pub bind_addr: SocketAddr,
    /// Lease duration for client state
    pub lease_duration: Duration,
}

/// NFS server
pub struct NfsServer<F> {
    /// Server configuration
    config: ServerConfig,
    /// Filesystem implementation
    fs: Arc<F>,
    /// State manager
    state: Arc<StateManager>,
}

impl<F: FileSystem + 'static> NfsServer<F> {
    /// Create a new NFS server
    pub fn new(config: ServerConfig, fs: F) -> Self {
        let lease_duration = config.lease_duration;
        NfsServer {
            config: config.clone(),
            fs: Arc::new(fs),
            state: Arc::new(StateManager::new(lease_duration)),
        }
    }

    /// Start the server
    pub async fn run(&self) -> Result<()> {
        let listener = TcpListener::bind(self.config.bind_addr).await?;
        info!("NFS server listening on {}", self.config.bind_addr);

        loop {
            let (socket, peer) = listener.accept().await?;
            info!("New connection from {}", peer);

            let fs = self.fs.clone();
            let state = self.state.clone();

            tokio::spawn(async move {
                if let Err(e) = handle_connection(socket, fs, state).await {
                    error!("Connection error: {}", e);
                }
            });
        }
    }
}

/// Handle a client connection
async fn handle_connection<F: FileSystem>(
    mut socket: TcpStream,
    fs: Arc<F>,
    state: Arc<StateManager>,
) -> Result<()> {
    let mut buf = BytesMut::with_capacity(4096);

    loop {
        // Read RPC record marker
        let mut marker = [0u8; 4];
        socket.read_exact(&mut marker).await?;
        let len = u32::from_be_bytes(marker) & 0x7fffffff;

        // Read RPC message
        buf.resize(len as usize, 0);
        socket.read_exact(&mut buf).await?;

        // Parse RPC header
        let mut reader = buf.clone().freeze();
        let header = RpcHeader::decode(&mut reader)?;

        match header.msg_type {
            MessageType::Call => {
                // Parse RPC call
                let call = RpcCall::decode(&mut reader)?;
                if call.program != 100003 || call.version != 4 {
                    return Err(Error::Rpc("Invalid program or version".into()));
                }

                // Handle NFS COMPOUND
                if call.procedure == 1 {
                    let request = CompoundRequest::decode(&mut reader)?;
                    let reply = handle_compound(request, &fs, &state).await?;

                    // Encode and send reply
                    let mut response = BytesMut::new();
                    RpcHeader {
                        xid: header.xid,
                        msg_type: MessageType::Reply,
                    }
                    .encode(&mut response)?;
                    reply.encode(&mut response)?;

                    // Write record marker and response
                    let len = response.len() as u32;
                    socket.write_all(&(len | 0x80000000).to_be_bytes()).await?;
                    socket.write_all(&response).await?;
                }
            }
            MessageType::Reply => {
                return Err(Error::Rpc("Unexpected RPC reply".into()));
            }
        }
    }
}

/// Handle an NFS COMPOUND request
async fn handle_compound<F: FileSystem>(
    request: CompoundRequest,
    fs: &F,
    _state: &StateManager,
) -> Result<CompoundReply> {
    let mut results = Vec::new();
    let mut current_fh = None;

    for op in request.operations {
        let result = match op.opcode {
            NfsOpcode::PutRootFh => {
                current_fh = Some(fs.root_filehandle().await?);
                CompoundResult {
                    status: 0,
                    data: Vec::new(),
                }
            }
            NfsOpcode::Lookup => {
                if let Some(ref parent) = current_fh {
                    let mut reader = op.data.as_slice();
                    let name = crate::xdr::helpers::decode_string(&mut reader)?;
                    current_fh = Some(fs.lookup(parent, &name).await?);
                    CompoundResult {
                        status: 0,
                        data: Vec::new(),
                    }
                } else {
                    CompoundResult {
                        status: 1, // NFS4ERR_NOFILEHANDLE
                        data: Vec::new(),
                    }
                }
            }
            // Add other operation handlers here
            _ => CompoundResult {
                status: 2, // NFS4ERR_NOTSUPP
                data: Vec::new(),
            },
        };
        results.push(result);
    }

    Ok(CompoundReply { status: 0, results })
}
