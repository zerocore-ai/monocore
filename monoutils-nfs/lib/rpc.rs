//! RPC (Remote Procedure Call) implementation

use bytes::{Buf, BufMut, BytesMut};
use num_enum::{IntoPrimitive, TryFromPrimitive};

use crate::error::Error;
use crate::xdr::{XdrDecode, XdrEncode};
use crate::Result;

/// RPC message types
#[derive(Debug, Clone, Copy, PartialEq, Eq, IntoPrimitive, TryFromPrimitive)]
#[repr(u32)]
pub enum MessageType {
    /// RPC call message
    Call = 0,
    /// RPC reply message
    Reply = 1,
}

/// RPC authentication status
#[derive(Debug, Clone, Copy, PartialEq, Eq, IntoPrimitive, TryFromPrimitive)]
#[repr(u32)]
pub enum AuthStatus {
    /// Success
    Ok = 0,
    /// Bad credential (not authenticated)
    BadCred = 1,
    /// Bad verifier (not authenticated)
    BadVerf = 2,
    /// Invalid client credential (rejected)
    RejectedCred = 3,
    /// Invalid client verifier (rejected)
    RejectedVerf = 4,
    /// Too weak authentication (rejected)
    TooWeak = 5,
}

/// RPC message header
#[derive(Debug)]
pub struct RpcHeader {
    /// Transaction ID
    pub xid: u32,
    /// Message type (call or reply)
    pub msg_type: MessageType,
}

impl XdrEncode for RpcHeader {
    fn encode<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        self.xid.encode(buf)?;
        let msg_type: u32 = self.msg_type.into();
        msg_type.encode(buf)?;
        Ok(())
    }
}

impl XdrDecode for RpcHeader {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        let xid = u32::decode(buf)?;
        let msg_type = u32::decode(buf)?;
        let msg_type = MessageType::try_from(msg_type)
            .map_err(|e| Error::Rpc(format!("Invalid message type: {}", e)))?;
        Ok(RpcHeader { xid, msg_type })
    }
}

/// RPC call body
#[derive(Debug)]
pub struct RpcCall {
    /// RPC version (should be 2)
    pub rpc_version: u32,
    /// Program number (100003 for NFS)
    pub program: u32,
    /// Program version (4 for NFSv4)
    pub version: u32,
    /// Procedure number
    pub procedure: u32,
    /// Authentication credentials
    pub cred: AuthSys,
    /// Authentication verifier (usually empty)
    pub verf: AuthNone,
}

/// AUTH_SYS authentication (basic Unix-style)
#[derive(Debug)]
pub struct AuthSys {
    /// Timestamp
    pub stamp: u32,
    /// Machine name
    pub machinename: String,
    /// User ID
    pub uid: u32,
    /// Group ID
    pub gid: u32,
    /// Auxiliary group IDs
    pub gids: Vec<u32>,
}

/// AUTH_NONE authentication (no authentication)
#[derive(Debug)]
pub struct AuthNone;

impl XdrEncode for RpcCall {
    fn encode<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        self.rpc_version.encode(buf)?;
        self.program.encode(buf)?;
        self.version.encode(buf)?;
        self.procedure.encode(buf)?;

        // Encode AUTH_SYS cred
        buf.put_u32(1); // AUTH_SYS
        let mut cred_buf = BytesMut::new();
        self.cred.encode(&mut cred_buf)?;
        let cred_len = cred_buf.len() as u32;
        buf.put_u32(cred_len);
        buf.put_slice(&cred_buf);

        // Encode AUTH_NONE verf
        buf.put_u32(0); // AUTH_NONE
        buf.put_u32(0); // zero length
        Ok(())
    }
}

impl XdrDecode for RpcCall {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        let rpc_version = u32::decode(buf)?;
        let program = u32::decode(buf)?;
        let version = u32::decode(buf)?;
        let procedure = u32::decode(buf)?;

        // Decode AUTH_SYS cred
        let auth_flavor = u32::decode(buf)?;
        if auth_flavor != 1 {
            return Err(Error::Rpc("Only AUTH_SYS supported".into()));
        }
        let cred_len = u32::decode(buf)? as usize;
        let mut cred_buf = buf.copy_to_bytes(cred_len);
        let cred = AuthSys::decode(&mut cred_buf)?;

        // Decode AUTH_NONE verf
        let verf_flavor = u32::decode(buf)?;
        let verf_len = u32::decode(buf)?;
        if verf_flavor != 0 || verf_len != 0 {
            return Err(Error::Rpc("Only AUTH_NONE verifier supported".into()));
        }
        let verf = AuthNone;

        Ok(RpcCall {
            rpc_version,
            program,
            version,
            procedure,
            cred,
            verf,
        })
    }
}

impl XdrEncode for AuthSys {
    fn encode<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        self.stamp.encode(buf)?;
        crate::xdr::helpers::encode_string(&self.machinename, buf)?;
        self.uid.encode(buf)?;
        self.gid.encode(buf)?;

        // Encode auxiliary groups
        let gids_len = self.gids.len() as u32;
        buf.put_u32(gids_len);
        for gid in &self.gids {
            gid.encode(buf)?;
        }
        Ok(())
    }
}

impl XdrDecode for AuthSys {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        let stamp = u32::decode(buf)?;
        let machinename = crate::xdr::helpers::decode_string(buf)?;
        let uid = u32::decode(buf)?;
        let gid = u32::decode(buf)?;

        let gids_len = u32::decode(buf)? as usize;
        let mut gids = Vec::with_capacity(gids_len);
        for _ in 0..gids_len {
            gids.push(u32::decode(buf)?);
        }

        Ok(AuthSys {
            stamp,
            machinename,
            uid,
            gid,
            gids,
        })
    }
}
