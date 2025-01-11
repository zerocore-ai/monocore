//! XDR (External Data Representation) implementation

use bytes::{Buf, BufMut};

use crate::error::Error;
use crate::Result;

/// Trait for types that can be encoded to XDR format
pub trait XdrEncode {
    /// Encode self into XDR format
    fn encode<B: BufMut>(&self, buf: &mut B) -> Result<()>;
}

/// Trait for types that can be decoded from XDR format
pub trait XdrDecode: Sized {
    /// Decode self from XDR format
    fn decode<B: Buf>(buf: &mut B) -> Result<Self>;
}

/// Helper functions for XDR encoding/decoding
pub mod helpers {
    use super::*;

    /// Encode a string to XDR format
    pub fn encode_string<B: BufMut>(s: &str, buf: &mut B) -> Result<()> {
        let bytes = s.as_bytes();
        let len = bytes.len() as u32;
        buf.put_u32(len);
        buf.put_slice(bytes);

        // Pad to 4-byte boundary
        let padding = (4 - (bytes.len() % 4)) % 4;
        for _ in 0..padding {
            buf.put_u8(0);
        }
        Ok(())
    }

    /// Decode a string from XDR format
    pub fn decode_string<B: Buf>(buf: &mut B) -> Result<String> {
        let len = buf.get_u32() as usize;
        let mut bytes = vec![0; len];
        buf.copy_to_slice(&mut bytes);

        // Skip padding
        let padding = (4 - (len % 4)) % 4;
        buf.advance(padding);

        String::from_utf8(bytes).map_err(|e| Error::Xdr(format!("Invalid UTF-8: {}", e)))
    }
}

// Implement XDR encoding/decoding for basic types
impl XdrEncode for u32 {
    fn encode<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        buf.put_u32(*self);
        Ok(())
    }
}

impl XdrDecode for u32 {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        Ok(buf.get_u32())
    }
}

impl XdrEncode for i32 {
    fn encode<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        buf.put_i32(*self);
        Ok(())
    }
}

impl XdrDecode for i32 {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        Ok(buf.get_i32())
    }
}

impl XdrEncode for bool {
    fn encode<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        buf.put_u32(if *self { 1 } else { 0 });
        Ok(())
    }
}

impl XdrDecode for bool {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        Ok(buf.get_u32() != 0)
    }
}
