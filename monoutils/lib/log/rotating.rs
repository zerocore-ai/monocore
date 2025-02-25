//! Log rotation implementation for the Monocore runtime.
//!
//! This module provides a rotating log implementation that automatically rotates log files
//! when they reach a specified size. The rotation process involves:
//! 1. Renaming the current log file to .old extension
//! 2. Creating a new empty log file
//! 3. Continuing writing to the new file
//!
//! The implementation is fully asynchronous and implements AsyncWrite.

use futures::future::BoxFuture;
use std::{
    io::{self, Write},
    path::{Path, PathBuf},
    pin::Pin,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    task::{Context, Poll},
};
use tokio::{
    fs::{remove_file, rename, File, OpenOptions},
    io::{AsyncWrite, AsyncWriteExt},
    sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
    task::JoinHandle,
};

use crate::DEFAULT_LOG_MAX_SIZE;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A rotating log file that automatically rotates when reaching a maximum size.
///
/// The log rotation process preserves the last full log file with a ".old" extension
/// while continuing to write to a new log file with the original name.
///
/// # Example
///
/// ```no_run
/// use monoutils::log::RotatingLog;
///
/// #[tokio::main]
/// async fn main() -> std::io::Result<()> {
///     let log = RotatingLog::new("app.log").await?; // 1MB max size
///     Ok(())
/// }
/// ```
pub struct RotatingLog {
    /// The current log file being written to
    file: File,

    /// Path to the current log file
    path: PathBuf,

    /// Maximum size in bytes before rotation
    max_size: u64,

    /// Current size of the log file (shared between sync and async paths)
    current_size: Arc<AtomicU64>,

    /// Current state of the log rotation
    state: State,

    /// Channel for sending data to sync writer
    tx: UnboundedSender<Vec<u8>>,

    /// Background task handle
    _background_task: JoinHandle<()>,
}

/// Internal state machine for managing log rotation
enum State {
    /// Normal operation, ready to accept writes
    Idle,

    /// Currently performing log rotation
    Rotating(RotationFuture),

    /// Currently writing data
    Writing,
}

/// A sync writer that sends all written data to a channel.
pub struct SyncChannelWriter {
    tx: UnboundedSender<Vec<u8>>,
}

type RotationFuture = BoxFuture<'static, io::Result<(File, PathBuf)>>;

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl RotatingLog {
    /// Creates a new rotating log file with the default maximum size.
    ///
    /// This is a convenience wrapper around [`with_max_size`] that uses the default
    /// maximum log file size defined in `DEFAULT_LOG_MAX_SIZE`.
    ///
    /// ## Arguments
    ///
    /// * `path` - Path to the log file
    ///
    /// ## Errors
    ///
    /// Will return an error if:
    /// * The file cannot be created or opened
    /// * File metadata cannot be read
    pub async fn new(path: impl AsRef<Path>) -> io::Result<Self> {
        Self::with_max_size(path, DEFAULT_LOG_MAX_SIZE).await
    }

    /// Creates a new rotating log file.
    ///
    /// ## Errors
    ///
    /// Will return an error if:
    /// * The file cannot be created or opened
    /// * File metadata cannot be read
    pub async fn with_max_size(path: impl AsRef<Path>, max_size: u64) -> io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await?;
        let metadata = file.metadata().await?;
        let (tx, rx) = mpsc::unbounded_channel();

        // Create shared atomic counter for current size
        let current_size = Arc::new(AtomicU64::new(metadata.len()));

        // Create a clone of the file and size counter for the background task
        let bg_file = file.try_clone().await?;
        let bg_path = path.clone();
        let bg_max_size = max_size;
        let bg_size = Arc::clone(&current_size);

        // Spawn background task to handle channel data
        let background_task = tokio::spawn(async move {
            handle_channel_data(rx, bg_file, bg_path, bg_max_size, bg_size).await
        });

        Ok(Self {
            file,
            path,
            max_size,
            current_size,
            state: State::Idle,
            tx,
            _background_task: background_task,
        })
    }

    /// Get a sync writer that implements std::io::Write
    pub fn get_sync_writer(&self) -> SyncChannelWriter {
        SyncChannelWriter::new(self.tx.clone())
    }
}

impl SyncChannelWriter {
    /// Creates a new `SyncChannelWriter` with the given channel sender.
    pub fn new(tx: UnboundedSender<Vec<u8>>) -> Self {
        Self { tx }
    }
}

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Performs the actual log rotation operation.
///
/// # Arguments
///
/// * `file` - The current log file to be rotated
/// * `path` - Path to the current log file
///
/// # Returns
///
/// Returns a tuple containing:
/// * The newly created log file
/// * The path to the new log file
///
/// # Errors
///
/// Will return an error if:
/// * File synchronization fails
/// * Old backup file cannot be removed
/// * File rename operation fails
/// * New log file cannot be created
async fn do_rotation(file: File, path: PathBuf) -> io::Result<(File, PathBuf)> {
    file.sync_all().await?;
    let backup_path = path.with_extension("old");
    if backup_path.exists() {
        remove_file(&backup_path).await?;
    }

    rename(&path, &backup_path).await?;

    let new_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .await?;

    Ok((new_file, path))
}

/// Background task that handles data from the sync channel
async fn handle_channel_data(
    mut rx: UnboundedReceiver<Vec<u8>>,
    mut file: File,
    path: PathBuf,
    max_size: u64,
    current_size: Arc<AtomicU64>,
) {
    while let Some(data) = rx.recv().await {
        let data_len = data.len() as u64;
        let size = current_size.fetch_add(data_len, Ordering::Relaxed);

        if size + data_len > max_size {
            // Clone the file handle before rotation
            if let Ok(file_clone) = file.try_clone().await {
                match do_rotation(file_clone, path.clone()).await {
                    Ok((new_file, _)) => {
                        file = new_file;
                        current_size.store(0, Ordering::Relaxed);
                    }
                    Err(e) => {
                        tracing::error!("failed to rotate log file: {}", e);
                        continue;
                    }
                }
            } else {
                tracing::error!("failed to clone file handle for rotation");
                continue;
            }
        }

        if let Err(e) = file.write_all(&data).await {
            tracing::error!("failed to write to log file: {}", e);
            // On write error, subtract the size we added
            current_size.fetch_sub(data_len, Ordering::Relaxed);
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl AsyncWrite for RotatingLog {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = &mut *self;
        let buf_len = buf.len() as u64;

        loop {
            match &mut this.state {
                State::Idle => {
                    let size = this.current_size.fetch_add(buf_len, Ordering::Relaxed);
                    if size + buf_len > this.max_size {
                        let old_file = std::mem::replace(
                            &mut this.file,
                            File::from_std(std::fs::File::open("/dev/null").unwrap()),
                        );
                        let old_path = this.path.clone();
                        let fut = Box::pin(do_rotation(old_file, old_path));
                        this.state = State::Rotating(fut);
                    } else {
                        this.state = State::Writing;
                    }
                }
                State::Rotating(fut) => {
                    match fut.as_mut().poll(cx) {
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(Err(e)) => {
                            this.state = State::Idle;
                            // On rotation error, subtract the size we added
                            this.current_size.fetch_sub(buf_len, Ordering::Relaxed);
                            return Poll::Ready(Err(e));
                        }
                        Poll::Ready(Ok((new_file, new_path))) => {
                            this.file = new_file;
                            this.path = new_path;
                            this.current_size.store(0, Ordering::Relaxed);
                            this.state = State::Writing;
                        }
                    }
                }
                State::Writing => {
                    let pinned_file = Pin::new(&mut this.file);
                    match pinned_file.poll_write(cx, buf) {
                        Poll::Ready(Ok(written)) => {
                            this.state = State::Idle;
                            return Poll::Ready(Ok(written));
                        }
                        Poll::Ready(Err(e)) => {
                            this.state = State::Idle;
                            // On write error, subtract the size we added
                            this.current_size.fetch_sub(buf_len, Ordering::Relaxed);
                            return Poll::Ready(Err(e));
                        }
                        Poll::Pending => return Poll::Pending,
                    }
                }
            }
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.file).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.file).poll_shutdown(cx)
    }
}

impl Write for SyncChannelWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let data = buf.to_vec();
        self.tx.send(data).map_err(|_| {
            io::Error::new(io::ErrorKind::Other, "failed to send log data to channel")
        })?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;
    use tokio::io::AsyncWriteExt;

    #[tokio::test]
    async fn test_create_new_log() -> io::Result<()> {
        let dir = tempdir()?;
        let log_path = dir.path().join("test.log");

        let log = RotatingLog::with_max_size(&log_path, 1024).await?;
        assert!(log_path.exists());
        assert_eq!(log.max_size, 1024);
        assert_eq!(log.current_size.load(Ordering::Relaxed), 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_write_to_log() -> io::Result<()> {
        let dir = tempdir()?;
        let log_path = dir.path().join("test.log");

        let mut log = RotatingLog::with_max_size(&log_path, 1024).await?;
        let test_data = b"test log entry\n";
        log.write_all(test_data).await?;
        log.flush().await?;

        let content = fs::read_to_string(&log_path)?;
        assert_eq!(content, String::from_utf8_lossy(test_data));
        assert_eq!(
            log.current_size.load(Ordering::Relaxed),
            test_data.len() as u64
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_log_rotation() -> io::Result<()> {
        let dir = tempdir()?;
        let log_path = dir.path().join("test.log");
        let max_size = 20; // Small size to trigger rotation

        let mut log = RotatingLog::with_max_size(&log_path, max_size).await?;

        // Write data until we trigger rotation
        let first_entry = b"first entry\n";
        log.write_all(first_entry).await?;
        log.flush().await?;

        let second_entry = b"second entry\n";
        log.write_all(second_entry).await?;
        log.flush().await?;

        // Give some time for rotation to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Check that both current and old log files exist
        assert!(log_path.exists());
        assert!(log_path.with_extension("old").exists());

        // Verify old file contains our first entry
        let old_content = fs::read_to_string(log_path.with_extension("old"))?;
        assert_eq!(old_content, String::from_utf8_lossy(first_entry));

        // Verify new file contains our second entry
        let new_content = fs::read_to_string(&log_path)?;
        assert_eq!(new_content, String::from_utf8_lossy(second_entry));

        Ok(())
    }

    #[tokio::test]
    async fn test_oversized_write() -> io::Result<()> {
        let dir = tempdir()?;
        let log_path = dir.path().join("test.log");
        let max_size = 10; // Small size

        let mut log = RotatingLog::with_max_size(&log_path, max_size).await?;

        // Write data much larger than max_size
        let large_entry = b"this is a very large log entry that exceeds the maximum size\n";
        log.write_all(large_entry).await?;
        log.flush().await?;

        // Give some time for rotation to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Verify the content was written (even though it exceeds max_size)
        assert!(log_path.exists());
        let content = fs::read_to_string(&log_path)?;
        assert_eq!(content, String::from_utf8_lossy(large_entry));

        Ok(())
    }

    #[tokio::test]
    async fn test_sync_writer() -> io::Result<()> {
        let dir = tempdir()?;
        let log_path = dir.path().join("test.log");

        let log = RotatingLog::with_max_size(&log_path, 1024).await?;
        let mut sync_writer = log.get_sync_writer();

        let test_data = b"sync writer test\n";
        sync_writer.write_all(test_data)?;
        sync_writer.flush()?;

        // Give some time for async processing
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let content = fs::read_to_string(&log_path)?;
        assert_eq!(content, String::from_utf8_lossy(test_data));

        Ok(())
    }

    #[tokio::test]
    async fn test_multiple_rotations() -> io::Result<()> {
        let dir = tempdir()?;
        let log_path = dir.path().join("test.log");
        let max_size = 20;

        let mut log = RotatingLog::with_max_size(&log_path, max_size).await?;

        // Perform multiple writes to trigger multiple rotations
        for i in 0..3 {
            let test_data = format!("rotation test {}\n", i).into_bytes();
            log.write_all(&test_data).await?;
            log.flush().await?;

            // Give time for rotation
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        // Verify only one .old file exists (latest rotation)
        assert!(log_path.exists());
        assert!(log_path.with_extension("old").exists());

        Ok(())
    }
}
