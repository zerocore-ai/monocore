use std::{
    io::{self, SeekFrom},
    pin::Pin,
    task::{Context, Poll},
};

use chrono::Utc;
use futures::Future;
use monoutils::{EmptySeekableReader, SeekableReader};
use monoutils_store::{IpldStore, IpldStoreSeekable};
use tokio::io::{AsyncRead, AsyncSeek, AsyncWrite, ReadBuf};

use crate::{filesystem::File, FsResult};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A stream for reading from a `File` asynchronously.
pub struct FileInputStream<'a> {
    reader: Pin<Box<dyn SeekableReader + Send + Sync + 'a>>,
}

/// A stream for writing to a `File` asynchronously.
pub struct FileOutputStream<'a, S>
where
    S: IpldStore,
{
    file: &'a mut File<S>,
    buffer: Vec<u8>,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl<S> File<S>
where
    S: IpldStore + Send + Sync + 'static,
{
    /// Gets an input stream for reading the file's content.
    pub async fn get_input_stream(&self) -> io::Result<FileInputStream<'_>>
    where
        S: IpldStoreSeekable,
    {
        FileInputStream::new(self).await
    }

    /// Gets an output stream for writing to the file.
    pub fn get_output_stream(&mut self) -> FileOutputStream<'_, S> {
        FileOutputStream::new(self)
    }
}

impl<'a> FileInputStream<'a> {
    /// Creates a new `FileInputStream` from a `File`.
    pub async fn new<S>(file: &'a File<S>) -> io::Result<Self>
    where
        S: IpldStoreSeekable + Send + Sync + 'a,
    {
        let store = file.get_store();
        let reader = match file.get_content() {
            Some(cid) => store
                .get_seekable_bytes(cid)
                .await
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?,
            None => {
                Box::pin(EmptySeekableReader) as Pin<Box<dyn SeekableReader + Send + Sync + 'a>>
            }
        };

        Ok(Self { reader })
    }
}

impl<'a, S> FileOutputStream<'a, S>
where
    S: IpldStore + Send + Sync + 'static,
{
    /// Creates a new `FileOutputStream` for a `File`.
    pub fn new(file: &'a mut File<S>) -> Self {
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
            self.file.set_content(Some(cid));
            self.file.get_metadata_mut().set_modified_at(Utc::now());
            self.buffer.clear();
        }
        Ok(())
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl AsyncRead for FileInputStream<'_> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.reader).poll_read(cx, buf)
    }
}

impl AsyncSeek for FileInputStream<'_> {
    fn start_seek(mut self: Pin<&mut Self>, position: SeekFrom) -> io::Result<()> {
        Pin::new(&mut self.reader).start_seek(position)
    }

    fn poll_complete(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<u64>> {
        Pin::new(&mut self.reader).poll_complete(cx)
    }
}

impl<S> AsyncWrite for FileOutputStream<'_, S>
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
    use anyhow::Result;
    use monoutils_store::MemoryStore;
    use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt, BufReader};

    use crate::filesystem::File;

    use super::*;

    #[tokio::test]
    async fn test_file_input_stream() -> Result<()> {
        let store = MemoryStore::default();
        let mut file = File::new(store.clone());

        // Create some content for the file
        let content = b"Hello, world!";
        let cid = store.put_bytes(content.as_slice()).await?;
        file.set_content(Some(cid));

        // Create an input stream from the file
        let mut input_stream = FileInputStream::new(&file).await?;

        // Read the content from the input stream
        let mut buffer = Vec::new();
        let n = input_stream.read_to_end(&mut buffer).await?;

        // Verify the content
        assert_eq!(n, content.len());
        assert_eq!(buffer, content);

        Ok(())
    }

    #[tokio::test]
    async fn test_file_output_stream() -> Result<()> {
        let store = MemoryStore::default();
        let mut file = File::new(store);
        let mut output_stream = FileOutputStream::new(&mut file);

        let data = b"Hello, world!";
        output_stream.write_all(data).await?;
        output_stream.shutdown().await?;

        // Now read the file to verify the content
        let input_stream = FileInputStream::new(&file).await?;
        let mut buf = BufReader::new(input_stream);
        let mut content = Vec::new();
        buf.read_to_end(&mut content).await?;

        assert_eq!(content, data);

        Ok(())
    }

    #[tokio::test]
    async fn test_file_input_stream_seek() -> Result<()> {
        let store = MemoryStore::default();
        let mut file = File::new(store.clone());

        // Create some content for the file
        let content = b"Hello, world!";
        let cid = store.put_bytes(content.as_slice()).await?;
        file.set_content(Some(cid));

        // Create an input stream from the file
        let mut input_stream = FileInputStream::new(&file).await?;

        // Test seeking from start
        input_stream.seek(SeekFrom::Start(7)).await?;
        let mut buffer = [0u8; 6];
        input_stream.read_exact(&mut buffer).await?;
        assert_eq!(&buffer, b"world!");

        // Test seeking from current position
        input_stream.seek(SeekFrom::Current(-6)).await?;
        let mut buffer = [0u8; 5];
        input_stream.read_exact(&mut buffer).await?;
        assert_eq!(&buffer, b"world");

        // Test seeking from end
        input_stream.seek(SeekFrom::End(-13)).await?;
        let mut buffer = [0u8; 5];
        input_stream.read_exact(&mut buffer).await?;
        assert_eq!(&buffer, b"Hello");

        Ok(())
    }
}
