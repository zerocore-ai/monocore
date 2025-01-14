use std::{
    io::{self, SeekFrom},
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, AsyncSeek, ReadBuf};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A seekable reader that always reads zero bytes and reports position as 0.
#[derive(Debug)]
pub struct EmptySeekableReader;

//--------------------------------------------------------------------------------------------------
// Traits
//--------------------------------------------------------------------------------------------------

/// A trait that extends the `AsyncRead` and `AsyncSeek` traits to allow for seeking.
pub trait SeekableReader: AsyncRead + AsyncSeek {}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl<T> SeekableReader for T where T: AsyncRead + AsyncSeek {}

// Implement AsyncRead by always reading zero bytes
impl AsyncRead for EmptySeekableReader {
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        _buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

// Implement AsyncSeek by always claiming the new position is 0
impl AsyncSeek for EmptySeekableReader {
    fn start_seek(self: Pin<&mut Self>, _position: SeekFrom) -> io::Result<()> {
        Ok(())
    }

    fn poll_complete(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<u64>> {
        Poll::Ready(Ok(0))
    }
}
