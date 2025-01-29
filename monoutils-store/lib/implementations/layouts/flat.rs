use std::{
    cmp::Ordering,
    io::{Error, ErrorKind, SeekFrom},
    pin::Pin,
    task::{Context, Poll},
};

use aliasable::boxed::AliasableBox;
use async_stream::try_stream;
use async_trait::async_trait;
use bytes::Bytes;
use futures::{ready, stream::BoxStream, Future, StreamExt};
use ipld_core::cid::Cid;
use monoutils::SeekableReader;
use tokio::io::{AsyncRead, AsyncSeek, ReadBuf};

use crate::{IpldStore, Layout, LayoutError, LayoutSeekable, MerkleNode, StoreError, StoreResult};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A layout that organizes data into a flat array of chunks with a single merkle node parent.
///
/// ```txt
///                      ┌──────────────────┐
///                      │   Merkle Node    │
///                      └─────────┬────────┘
///                                │
///   ┌────────┬────────────┬──────┴──┬─────────┬──────────┬──────┐
///   │        │            │         │         │          │      │
/// ┌─┴─┐┌─────┴─────┐┌─────┴─────┐┌──┴──┐┌─────┴───────┐┌─┴─┐┌───┴───┐
/// │ 0 ││ 1 2 3 4 5 ││ 6 7 8 9 A ││ B C ││ D E F G H I ││ J ││ K L M │
/// └───┘└───────────┘└───────────┘└─────┘└─────────────┘└───┘└───────┘
/// 1 byte   5 bytes     5 bytes   2 byte   6 bytes      1 byte  3 bytes
/// ```
#[derive(Clone, Debug, PartialEq, Default)]
pub struct FlatLayout {}

/// A reader for the flat DAG layout.
///
/// The reader maintains three state variables:
///
/// - The current byte position, `byte_cursor`.
/// - The index of the current chunk within the node's children array, `chunk_index`.
/// - The distance (in bytes) of the current chunk index from the start, `chunk_distance`.
///
/// These state variables are used to determine the current chunk to read from and the byte position
/// within the chunk to read from. It basically enables seeking to any byte position within the
/// chunk array.
///
/// ```txt
///             Chunk Index = 2
///             Chunk Distance = 6
///                   │
/// ┌───┐┌───────────┐▼─────────────┐┌───────┐
/// │ A ││ B C D E F ││ G H I J K L ││ M N O │
/// └───┘└───────────┘└───────▲─────┘└───────┘
///                           │
///                           │
///                    Byte Cursor = 9
/// ```
pub struct FlatLayoutReader<S>
where
    S: IpldStore,
{
    /// The current byte position.
    byte_cursor: u64,

    /// The index of the current chunk within the node's children array.
    chunk_index: u64,

    /// The distance (in bytes) of the current chunk index from the start.
    chunk_distance: u64,

    /// A function to get a raw block.
    ///
    /// ## Warning
    ///
    /// Holds a reference to other fields in this struct. Declared first to ensure it is dropped
    /// before the other fields.
    get_raw_block_fn: Pin<Box<dyn Future<Output = StoreResult<Bytes>> + Send + 'static>>,

    /// The store associated with the reader.
    ///
    /// ## Warning
    ///
    /// Field must not be moved as it is referenced by `get_raw_block_fn`.
    store: AliasableBox<S>,

    /// The node that the reader is reading from.
    ///
    /// ## Warning
    ///
    /// Field must not be moved as it is referenced by `get_raw_block_fn`.
    node: AliasableBox<MerkleNode>,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl FlatLayout {
    /// Create a new flat DAG layout.
    pub fn new() -> Self {
        FlatLayout {}
    }
}

impl<S> FlatLayoutReader<S>
where
    S: IpldStore + Sync,
{
    /// Create a new flat DAG reader.
    fn new(node: MerkleNode, store: S) -> StoreResult<Self> {
        // Store node and store in the heap and make them aliasable.
        let node = AliasableBox::from_unique(Box::new(node));
        let store = AliasableBox::from_unique(Box::new(store));

        // Create future to get the first node child.
        let get_raw_block_fn: Pin<Box<dyn Future<Output = StoreResult<Bytes>> + Send>> = Box::pin(
            store.get_raw_block(
                node.children
                    .first()
                    .map(|(cid, _)| cid)
                    .ok_or(StoreError::from(LayoutError::NoLeafBlock))?,
            ),
        );

        // Unsafe magic to escape Rust ownership grip.
        let get_raw_block_fn: Pin<Box<dyn Future<Output = StoreResult<Bytes>> + Send + 'static>> =
            unsafe { std::mem::transmute(get_raw_block_fn) };

        Ok(FlatLayoutReader {
            byte_cursor: 0,
            chunk_index: 0,
            chunk_distance: 0,
            get_raw_block_fn,
            node,
            store,
        })
    }

    fn fix_future(&mut self) {
        // Create future to get the next child.
        let get_raw_block_fn: Pin<Box<dyn Future<Output = StoreResult<Bytes>> + Send>> =
            Box::pin(async {
                let bytes = self
                    .store
                    .get_raw_block(
                        self.node
                            .children
                            .get(self.chunk_index as usize)
                            .map(|(cid, _)| cid)
                            .ok_or(StoreError::from(LayoutError::NoLeafBlock))?,
                    )
                    .await?;

                // We just need bytes starting from byte cursor.
                let bytes = Bytes::copy_from_slice(
                    &bytes[(self.byte_cursor - self.chunk_distance) as usize..],
                );

                Ok(bytes)
            });

        // Unsafe magic to escape Rust ownership grip.
        let get_raw_block_fn: Pin<Box<dyn Future<Output = StoreResult<Bytes>> + Send + 'static>> =
            unsafe { std::mem::transmute(get_raw_block_fn) };

        // Update type's future.
        self.get_raw_block_fn = get_raw_block_fn;
    }

    fn read_update(&mut self, left_over: &[u8], consumed: u64) -> StoreResult<()> {
        // Update the byte cursor.
        self.byte_cursor += consumed;

        // If there's left over bytes, we create a future to return the left over bytes.
        if !left_over.is_empty() {
            let bytes = Bytes::copy_from_slice(left_over);
            let get_raw_block_fn = Box::pin(async { Ok(bytes) });
            self.get_raw_block_fn = get_raw_block_fn;
            return Ok(());
        }

        // If we've reached the end of the bytes, create a future that returns empty bytes.
        if self.byte_cursor >= self.node.size as u64 {
            let get_raw_block_fn = Box::pin(async { Ok(Bytes::new()) });
            self.get_raw_block_fn = get_raw_block_fn;
            return Ok(());
        }

        // Update the chunk distance and chunk index.
        self.chunk_distance += self.node.children[self.chunk_index as usize].1 as u64;
        self.chunk_index += 1;

        // Update the future.
        self.fix_future();

        Ok(())
    }

    fn seek_update(&mut self, byte_cursor: u64) -> StoreResult<()> {
        // Update the byte cursor.
        self.byte_cursor = byte_cursor;

        // If we've reached the end of the bytes, create a future that returns empty bytes.
        if self.byte_cursor >= self.node.size as u64 {
            let get_raw_block_fn = Box::pin(async { Ok(Bytes::new()) });
            self.get_raw_block_fn = get_raw_block_fn;
            return Ok(());
        }

        // We need to update the chunk index and distance essentially making sure that chunk index and distance
        // are referring to the chunk that the byte cursor is pointing to.
        loop {
            match self.chunk_distance.cmp(&byte_cursor) {
                Ordering::Less => {
                    if self.chunk_distance + self.node.children[self.chunk_index as usize].1 as u64
                        > byte_cursor
                    {
                        break;
                    }

                    self.chunk_distance += self.node.children[self.chunk_index as usize].1 as u64;
                    self.chunk_index += 1;

                    continue;
                }
                Ordering::Greater => {
                    self.chunk_index -= 1;
                    self.chunk_distance -= self.node.children[self.chunk_index as usize].1 as u64;

                    continue;
                }
                _ => break,
            }
        }

        // Update the future.
        self.fix_future();

        Ok(())
    }
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

#[async_trait]
impl Layout for FlatLayout {
    async fn organize<'a>(
        &'a self,
        mut stream: BoxStream<'a, StoreResult<Bytes>>,
        store: impl IpldStore + Send + Sync + 'static,
    ) -> StoreResult<BoxStream<'a, StoreResult<Cid>>> {
        let s = try_stream! {
            let mut children = Vec::new();
            while let Some(Ok(chunk)) = stream.next().await {
                let len = chunk.len();
                tracing::trace!("organizing by putting raw block: {:?}", len);
                let cid = store.put_raw_block(chunk).await?;
                tracing::trace!("successfully put raw block");
                children.push((cid, len));
                yield cid;
            }

            if children.is_empty() {
                Err(StoreError::from(LayoutError::EmptyStream))?;
            }

            let node = MerkleNode::new(children);
            let cid = store.put_node(&node).await?;

            yield cid;
        };

        Ok(Box::pin(s))
    }

    async fn retrieve(
        &self,
        cid: &Cid,
        store: impl IpldStore + Send + Sync + 'static,
    ) -> StoreResult<Pin<Box<dyn AsyncRead + Send>>> {
        let node = store.get_node(cid).await?;
        let reader = FlatLayoutReader::new(node, store)?;
        Ok(Box::pin(reader))
    }

    async fn get_size(&self, cid: &Cid, store: impl IpldStore + Send + Sync) -> StoreResult<u64> {
        let node: MerkleNode = store.get_node(cid).await?;
        Ok(node.size as u64)
    }
}

#[async_trait]
impl LayoutSeekable for FlatLayout {
    async fn retrieve_seekable(
        &self,
        cid: &Cid,
        store: impl IpldStore + Send + Sync + 'static,
    ) -> StoreResult<Pin<Box<dyn SeekableReader + Send>>> {
        let node = store.get_node(cid).await?;
        let reader = FlatLayoutReader::new(node, store)?;
        Ok(Box::pin(reader))
    }
}

impl<S> AsyncRead for FlatLayoutReader<S>
where
    S: IpldStore + Sync,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        // Get the next chunk of bytes.
        let bytes = ready!(self.get_raw_block_fn.as_mut().poll(cx))
            .map_err(|e| Error::new(ErrorKind::Other, e))?;

        // If the bytes is longer than the buffer, we only take the amount that fits.
        let (taken, left_over) = if bytes.len() > buf.remaining() {
            bytes.split_at(buf.remaining())
        } else {
            (&bytes[..], &[][..])
        };

        // Copy the slice to the buffer.
        buf.put_slice(taken);

        // Update the reader's state.
        self.read_update(left_over, taken.len() as u64)
            .map_err(|e| Error::new(ErrorKind::Other, e))?;

        Poll::Ready(Ok(()))
    }
}

impl<S> AsyncSeek for FlatLayoutReader<S>
where
    S: IpldStore + Sync,
{
    fn start_seek(mut self: Pin<&mut Self>, position: SeekFrom) -> std::io::Result<()> {
        let byte_cursor = match position {
            SeekFrom::Start(offset) => {
                if offset >= self.node.size as u64 {
                    return Err(Error::new(
                        ErrorKind::InvalidInput,
                        "Seek from start position out of bounds",
                    ));
                }

                offset
            }
            SeekFrom::Current(offset) => {
                let new_cursor = self.byte_cursor as i64 + offset;
                if new_cursor < 0 || new_cursor >= self.node.size as i64 {
                    return Err(Error::new(
                        ErrorKind::InvalidInput,
                        "Seek from current position out of bounds",
                    ));
                }

                new_cursor as u64
            }
            SeekFrom::End(offset) => {
                let new_cursor = self.node.size as i64 + offset;
                if new_cursor < 0 || new_cursor >= self.node.size as i64 {
                    return Err(Error::new(
                        ErrorKind::InvalidInput,
                        "Seek from end position out of bounds",
                    ));
                }

                new_cursor as u64
            }
        };

        // Update the reader's state.
        self.seek_update(byte_cursor)
            .map_err(|e| Error::new(ErrorKind::Other, e))?;

        Ok(())
    }

    fn poll_complete(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<std::io::Result<u64>> {
        Poll::Ready(Ok(self.byte_cursor))
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use futures::{stream, TryStreamExt};
    use tokio::io::{AsyncReadExt, AsyncSeekExt};

    use crate::MemoryStore;

    use super::*;

    #[tokio::test]
    async fn test_flat_layout_organize_and_retrieve() -> anyhow::Result<()> {
        let store = MemoryStore::default();
        let (data, _, chunk_stream) = fixtures::data_and_chunk_stream();

        // Organize chunks into a DAG.
        let layout = FlatLayout::default();
        let cid_stream = layout.organize(chunk_stream, store.clone()).await?;

        // Get the CID of the merkle node.
        let cids = cid_stream.try_collect::<Vec<_>>().await?;
        let cid = cids.last().unwrap();

        // Verify the size matches the original data
        let size = layout.get_size(cid, store.clone()).await?;
        assert_eq!(size, data.len() as u64);

        // Case: fill buffer automatically with `read_to_end`
        let mut reader = layout.retrieve(cid, store.clone()).await?;
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;

        assert_eq!(bytes, data);

        // Case: fill buffer manually with `read`
        let mut reader = layout.retrieve(cid, store).await?;
        let mut bytes: Vec<u8> = vec![];
        loop {
            let mut buf = vec![0; 5];
            let filled = reader.read(&mut buf).await?;
            if filled == 0 {
                break;
            }

            bytes.extend(&buf[..filled]);
        }

        assert_eq!(bytes, data);

        Ok(())
    }

    #[tokio::test]
    async fn test_flat_layout_seek() -> anyhow::Result<()> {
        let store = MemoryStore::default();
        let (data, _, chunk_stream) = fixtures::data_and_chunk_stream();

        // Organize chunks into a DAG.
        let layout = FlatLayout::default();
        let cid_stream = layout.organize(chunk_stream, store.clone()).await?;

        // Get the CID of the merkle node.
        let cids = cid_stream.try_collect::<Vec<_>>().await?;
        let cid = cids.last().unwrap();

        // Get seekable reader.
        let mut reader = layout.retrieve_seekable(cid, store).await?;

        // Case: read from start
        let mut buf = vec![0; 3];
        reader.read_exact(&mut buf).await?;
        assert_eq!(&buf, &data[..3]); // "Lor"

        // Case: seek to arbitrary position and read across chunk boundary
        reader.seek(SeekFrom::Start(10)).await?;
        let mut buf = vec![0; 8];
        reader.read_exact(&mut buf).await?;
        assert_eq!(&buf, &data[10..18]); // "dolor sit"

        // Case: seek forward from current position to middle of data
        reader.seek(SeekFrom::Current(7)).await?;
        let mut buf = vec![0; 6];
        reader.read_exact(&mut buf).await?;
        assert_eq!(&buf, &data[25..31]); // "consec"

        // Case: seek backwards from current position
        reader.seek(SeekFrom::Current(-10)).await?;
        let mut buf = vec![0; 4];
        reader.read_exact(&mut buf).await?;
        assert_eq!(&buf, &data[21..25]); // "met,"

        // Case: seek from end and read
        reader.seek(SeekFrom::End(-8)).await?;
        let mut buf = vec![0; 5];
        reader.read_exact(&mut buf).await?;
        assert_eq!(&buf, &data[data.len() - 8..data.len() - 3]); // "g eli"

        // Case: seek to start
        reader.seek(SeekFrom::Start(0)).await?;
        let mut buf = vec![0; 4];
        reader.read_exact(&mut buf).await?;
        assert_eq!(&buf, &data[..4]); // "Lore"

        // Case: Fail: Seek beyond end
        let result = reader.seek(SeekFrom::End(1)).await;
        assert!(result.is_err());

        let result = reader.seek(SeekFrom::End(0)).await;
        assert!(result.is_err());

        let result = reader.seek(SeekFrom::Start(data.len() as u64 + 1)).await;
        assert!(result.is_err());

        // Case: Fail: Seek before start
        let _ = reader.seek(SeekFrom::Start(0)).await?;
        let result = reader.seek(SeekFrom::Current(-1)).await;
        assert!(result.is_err());

        // Case: Fail: Read beyond end
        reader.seek(SeekFrom::End(-2)).await?;
        let mut buf = vec![0; 3];
        let result = reader.read_exact(&mut buf).await;
        assert!(result.is_err());

        Ok(())
    }

    #[tokio::test]
    async fn test_flat_layout_sizes() -> anyhow::Result<()> {
        let store = MemoryStore::default();
        let layout = FlatLayout::default();

        // Test empty data
        let empty_stream = Box::pin(stream::iter(vec![Ok(Bytes::new())]));
        let cid_stream = layout.organize(empty_stream, store.clone()).await?;
        let empty_cid = cid_stream.try_collect::<Vec<_>>().await?.pop().unwrap();
        let size = layout.get_size(&empty_cid, store.clone()).await?;
        assert_eq!(size, 0);

        // Test single chunk data
        let single_chunk = Bytes::from("small data");
        let single_stream = Box::pin(stream::iter(vec![Ok(single_chunk.clone())]));
        let cid_stream = layout.organize(single_stream, store.clone()).await?;
        let single_cid = cid_stream.try_collect::<Vec<_>>().await?.pop().unwrap();
        let size = layout.get_size(&single_cid, store.clone()).await?;
        assert_eq!(size, single_chunk.len() as u64);

        // Test multi-chunk data
        let (data, _, chunk_stream) = fixtures::data_and_chunk_stream();
        let cid_stream = layout.organize(chunk_stream, store.clone()).await?;
        let multi_cid = cid_stream.try_collect::<Vec<_>>().await?.pop().unwrap();
        let size = layout.get_size(&multi_cid, store.clone()).await?;
        assert_eq!(size, data.len() as u64);

        Ok(())
    }

    #[tokio::test]
    async fn test_flat_layout_empty_stream() -> anyhow::Result<()> {
        let store = MemoryStore::default();
        let layout = FlatLayout::default();

        // Create an empty stream
        let empty_stream = Box::pin(stream::iter(Vec::<StoreResult<Bytes>>::new()));
        let cid_stream = layout.organize(empty_stream, store.clone()).await?;

        // Collecting the stream should fail with EmptyStream error
        let result = cid_stream.try_collect::<Vec<_>>().await;
        assert!(matches!(
            result,
            Err(StoreError::LayoutError(LayoutError::EmptyStream))
        ));

        Ok(())
    }
}

#[cfg(test)]
mod fixtures {
    use futures::{stream, Stream};

    use super::*;

    pub(super) fn data_and_chunk_stream() -> (
        [u8; 56],
        Vec<Bytes>,
        Pin<Box<dyn Stream<Item = StoreResult<Bytes>> + Send + 'static>>,
    ) {
        let data = b"Lorem ipsum dolor sit amet, consectetur adipiscing elit.".to_owned();

        let chunks = vec![
            Bytes::from("L"),               // 1 byte
            Bytes::from("orem "),           // 5 bytes
            Bytes::from("ipsum dol"),       // 9 bytes
            Bytes::from("or"),              // 2 bytes
            Bytes::from(" sit amet, cons"), // 14 bytes
            Bytes::from("ectetur adi"),     // 10 bytes
            Bytes::from("p"),               // 1 byte
            Bytes::from("iscing"),          // 6 bytes
            Bytes::from(" eli"),            // 4 bytes
            Bytes::from("t."),              // 2 bytes
        ];

        let chunks_result = chunks
            .iter()
            .cloned()
            .map(|b| crate::Ok(b))
            .collect::<Vec<_>>();

        let chunk_stream = Box::pin(stream::iter(chunks_result));

        (data, chunks, chunk_stream)
    }
}
