use tokio::io::{AsyncRead, AsyncSeek};

//--------------------------------------------------------------------------------------------------
// Traits
//--------------------------------------------------------------------------------------------------

/// A trait that extends the `AsyncRead` and `AsyncSeek` traits to allow for seeking.
pub trait SeekableReader: AsyncRead + AsyncSeek {}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl<T> SeekableReader for T where T: AsyncRead + AsyncSeek {}
