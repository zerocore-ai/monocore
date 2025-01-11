//! NFSv4 protocol implementation

use bytes::{Buf, BufMut};
use num_enum::{IntoPrimitive, TryFromPrimitive};

use crate::error::Error;
use crate::xdr::{XdrDecode, XdrEncode};
use crate::Result;

/// NFS version 4 operation codes
#[derive(Debug, Clone, Copy, PartialEq, Eq, IntoPrimitive, TryFromPrimitive)]
#[repr(u32)]
pub enum NfsOpcode {
    /// Access rights
    Access = 3,
    /// Close file
    Close = 4,
    /// Commit writes
    Commit = 5,
    /// Create file
    Create = 6,
    /// Delegations
    DelegPurge = 7,
    /// Return delegation
    DelegReturn = 8,
    /// Get file attributes
    GetAttr = 9,
    /// Get filesystem attributes
    GetFh = 10,
    /// Lock file
    Lock = 11,
    /// Lock with owner
    LockT = 12,
    /// Unlock file
    LockU = 13,
    /// Look up file name
    Lookup = 14,
    /// Look up parent directory
    LookupP = 15,
    /// Get filesystem locations
    NVerify = 16,
    /// Open file
    Open = 17,
    /// Open downgrade
    OpenAttr = 18,
    /// Open confirm
    OpenConfirm = 19,
    /// Open downgrade
    OpenDgrd = 20,
    /// Put public file handle
    PutFh = 21,
    /// Put root file handle
    PutPubFh = 22,
    /// Put root file handle
    PutRootFh = 23,
    /// Read from file
    Read = 24,
    /// Read directory
    ReadDir = 25,
    /// Read symbolic link
    ReadLink = 26,
    /// Remove file system object
    Remove = 27,
    /// Rename file
    Rename = 28,
    /// Renew lease
    Renew = 29,
    /// Restore file handle
    RestoreFh = 30,
    /// Save file handle
    SaveFh = 31,
    /// Security
    Secinfo = 32,
    /// Set file attributes
    SetAttr = 33,
    /// Verify file attributes
    Verify = 34,
    /// Write to file
    Write = 35,
    /// Release lock owner
    RelLockOwner = 36,
}

/// NFS file handle
#[derive(Debug, Clone)]
pub struct FileHandle(pub Vec<u8>);

impl XdrEncode for FileHandle {
    fn encode<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        let len = self.0.len() as u32;
        buf.put_u32(len);
        buf.put_slice(&self.0);
        Ok(())
    }
}

impl XdrDecode for FileHandle {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        let len = u32::decode(buf)? as usize;
        if len > 128 {
            return Err(Error::Nfs("File handle too long".into()));
        }
        let mut data = vec![0; len];
        buf.copy_to_slice(&mut data);
        Ok(FileHandle(data))
    }
}

/// NFS compound operation
#[derive(Debug)]
pub struct CompoundOp {
    /// Operation code
    pub opcode: NfsOpcode,
    /// Operation arguments (encoded in XDR)
    pub data: Vec<u8>,
}

impl XdrEncode for CompoundOp {
    fn encode<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        let opcode: u32 = self.opcode.into();
        opcode.encode(buf)?;
        let len = self.data.len() as u32;
        buf.put_u32(len);
        buf.put_slice(&self.data);
        Ok(())
    }
}

impl XdrDecode for CompoundOp {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        let opcode = u32::decode(buf)?;
        let opcode = NfsOpcode::try_from(opcode)
            .map_err(|e| Error::Nfs(format!("Invalid opcode: {}", e)))?;
        let len = u32::decode(buf)? as usize;
        let mut data = vec![0; len];
        buf.copy_to_slice(&mut data);
        Ok(CompoundOp { opcode, data })
    }
}

/// NFS compound request
#[derive(Debug)]
pub struct CompoundRequest {
    /// Minor version (0 for NFSv4.0)
    pub minor_version: u32,
    /// Array of operations
    pub operations: Vec<CompoundOp>,
}

impl XdrEncode for CompoundRequest {
    fn encode<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        self.minor_version.encode(buf)?;
        let ops_len = self.operations.len() as u32;
        buf.put_u32(ops_len);
        for op in &self.operations {
            op.encode(buf)?;
        }
        Ok(())
    }
}

impl XdrDecode for CompoundRequest {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        let minor_version = u32::decode(buf)?;
        let ops_len = u32::decode(buf)? as usize;
        let mut operations = Vec::with_capacity(ops_len);
        for _ in 0..ops_len {
            operations.push(CompoundOp::decode(buf)?);
        }
        Ok(CompoundRequest {
            minor_version,
            operations,
        })
    }
}

/// NFS compound reply
#[derive(Debug)]
pub struct CompoundReply {
    /// Status of the compound operation
    pub status: u32,
    /// Array of operation results
    pub results: Vec<CompoundResult>,
}

/// Result of a single operation in a compound
#[derive(Debug)]
pub struct CompoundResult {
    /// Status of this operation
    pub status: u32,
    /// Operation-specific result data
    pub data: Vec<u8>,
}

impl XdrEncode for CompoundReply {
    fn encode<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        self.status.encode(buf)?;
        let results_len = self.results.len() as u32;
        buf.put_u32(results_len);
        for result in &self.results {
            result.encode(buf)?;
        }
        Ok(())
    }
}

impl XdrDecode for CompoundReply {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        let status = u32::decode(buf)?;
        let results_len = u32::decode(buf)? as usize;
        let mut results = Vec::with_capacity(results_len);
        for _ in 0..results_len {
            results.push(CompoundResult::decode(buf)?);
        }
        Ok(CompoundReply { status, results })
    }
}

impl XdrEncode for CompoundResult {
    fn encode<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        self.status.encode(buf)?;
        let len = self.data.len() as u32;
        buf.put_u32(len);
        buf.put_slice(&self.data);
        Ok(())
    }
}

impl XdrDecode for CompoundResult {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        let status = u32::decode(buf)?;
        let len = u32::decode(buf)? as usize;
        let mut data = vec![0; len];
        buf.copy_to_slice(&mut data);
        Ok(CompoundResult { status, data })
    }
}
