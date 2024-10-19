use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};

use aliasable::boxed::AliasableBox;
use futures::Future;
use monoutils_store::IpldStore;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use crate::FsResult;

use super::File;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A stream for reading from a `File` asynchronously.
pub struct FileInputStream<S>
where
    S: IpldStore,
{
    /// An async reader for the file content.
    ///
    /// ## Important
    ///
    /// SAFETY: Holds a reference to other fields in this struct. Declared first to ensure it is
    /// dropped before the other fields.
    reader: Pin<Box<dyn AsyncRead + Send + Sync + 'static>>,

    /// The store.
    ///
    /// ## Warning
    ///
    /// SAFETY: Field must not be moved as it is referenced by `reader`.
    #[allow(dead_code)]
    store: AliasableBox<S>,
}

/// A stream for writing to a `File` asynchronously.
pub struct FileOutputStream<S>
where
    S: IpldStore,
{
    /// The file being written to.
    file: File<S>,

    /// Buffer to accumulate written data.
    // TODO: Use a ring buffer instead
    buffer: Vec<u8>,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl<S> FileInputStream<S>
where
    S: IpldStore + Send + Sync + 'static,
{
    /// Creates a new `FileInputStream` from a `File`.
    pub async fn from(file: &File<S>) -> Self {
        // Store the handle in the heap and make it aliasable.
        let store = AliasableBox::from_unique(Box::new(file.get_store().clone()));

        // If the file contains a Cid for its content, create a reader for it.
        let reader: Pin<Box<dyn AsyncRead + Send + Sync>> = match file.get_content() {
            Some(cid) => store.get_bytes(cid).await.unwrap(),
            None => Box::pin(tokio::io::empty()),
        };

        // SAFETY: Unsafe magic to escape Rust ownership grip.
        let reader: Pin<Box<dyn AsyncRead + Send + Sync + 'static>> =
            unsafe { std::mem::transmute(reader) };

        Self { reader, store }
    }
}

impl<S> FileOutputStream<S>
where
    S: IpldStore + Send + Sync + 'static,
{
    /// Creates a new `FileOutputStream` for a `File`.
    pub fn new(file: File<S>) -> Self {
        Self {
            file,
            buffer: Vec::new(),
        }
    }

    /// Finalizes the write operation and updates the file content.
    async fn finalize(&mut self) -> FsResult<()> {
        if !self.buffer.is_empty() {
            let store = self.file.get_store();

            let cid = store.put_bytes(&self.buffer[..]).await.map(Into::into)?;

            // Update the file's content with the new CID
            self.file.set_content(Some(cid));

            // Clear the buffer
            self.buffer.clear();
        }
        Ok(())
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl<S> AsyncRead for FileInputStream<S>
where
    S: IpldStore + Send + Sync + 'static,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        self.reader.as_mut().poll_read(cx, buf)
    }
}

impl<S> AsyncWrite for FileOutputStream<S>
where
    S: IpldStore + Send + Sync + 'static,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        self.buffer.extend_from_slice(buf);
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        let finalize_future = self.finalize();
        tokio::pin!(finalize_future);

        finalize_future
            .poll(cx)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use monoutils_store::MemoryStore;
    use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};

    use crate::file::File;

    use super::*;

    #[tokio::test]
    async fn test_file_input_stream() {
        let store = MemoryStore::default();
        let mut file = File::new(store.clone());

        // Create some content for the file
        let content = b"Hello, world!";
        let cid = store.put_bytes(content.as_slice()).await.unwrap();
        file.set_content(Some(cid));

        // Create an input stream from the file
        let mut input_stream = FileInputStream::from(&file).await;

        // Read the content from the input stream
        let mut buffer = Vec::new();
        let n = input_stream.read_to_end(&mut buffer).await.unwrap();

        // Verify the content
        assert_eq!(n, content.len());
        assert_eq!(buffer, content);
    }

    #[tokio::test]
    async fn test_file_output_stream() {
        let store = MemoryStore::default();
        let file = File::new(store);
        let mut output_stream = FileOutputStream::new(file);

        let data = b"Hello, world!";
        output_stream.write_all(data).await.unwrap();
        output_stream.shutdown().await.unwrap();

        // Now read the file to verify the content
        let updated_file = output_stream.file;
        let input_stream = FileInputStream::from(&updated_file).await;
        let mut buf = BufReader::new(input_stream);
        let mut content = Vec::new();
        buf.read_to_end(&mut content).await.unwrap();

        assert_eq!(content, data);
    }
}
