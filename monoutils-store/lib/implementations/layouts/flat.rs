use std::{
    cmp::Ordering,
    io::{Error, ErrorKind, SeekFrom},
    pin::Pin,
    task::{Context, Poll},
};

use aliasable::boxed::AliasableBox;
use async_stream::try_stream;
use bytes::Bytes;
use futures::{ready, stream::BoxStream, Future, StreamExt};
use libipld::Cid;
use tokio::io::{AsyncRead, AsyncSeek, ReadBuf};

use crate::{
    IpldStore, Layout, LayoutError, LayoutSeekable, MerkleNode, SeekableReader, StoreError,
    StoreResult,
};

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A layout that organizes data into a flat array of chunks with a single merkle node parent.
///
/// ```txt
///                      ┌─────────────┐
///                      │ Merkle Node │
///                      └──────┬──────┘
///                             │
///      ┌───────────────┬──────┴────────┬─────────────────┐
///      │               │               │                 │
///  0   ▼       1       ▼         2     ▼        3        ▼
/// ┌──┬──┬──┐  ┌──┬──┬──┬──┬──┐  ┌──┬──┬──┬──┐  ┌──┬──┬──┬──┬──┬──┐
/// │0 │1 │2 │  │3 │4 │5 │6 │7 │  │8 │9 │10│11│  │12│13│14│15│16│17│
/// └──┴──┴──┘  └──┴──┴──┴──┴──┘  └──┴──┴──┴──┘  └──┴──┴──┴──┴──┴──┘
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
///  Chunk Index    = 1
///  Chunk Distance = 3
///            │
///            │
///  0         ▼ 1                 2
/// ┌──┬──┬──┐  ┌──┬──┬──┬──┬──┐  ┌──┬──┬──┬──┐
/// │0 │1 │2 │  │3 │4 │5 │6 │7 │  │8 │9 │10│11│
/// └──┴──┴──┘  └──┴──┴──┴──┴──┘  └──┴──┴──┴──┘
///                    ▲
///                    │
///                    │
///                Byte Cursor = 5
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
    /// ## Important
    ///
    /// Holds a reference to other fields in this struct. Declared first to ensure it is dropped
    /// before the other fields.
    get_raw_block_fn: Pin<Box<dyn Future<Output = StoreResult<Bytes>> + Send + Sync + 'static>>,

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
        let get_raw_block_fn: Pin<Box<dyn Future<Output = StoreResult<Bytes>> + Send + Sync>> =
            Box::pin(
                store.get_raw_block(
                    node.children
                        .first()
                        .map(|(cid, _)| cid)
                        .ok_or(StoreError::from(LayoutError::NoLeafBlock))?,
                ),
            );

        // Unsafe magic to escape Rust ownership grip.
        let get_raw_block_fn: Pin<
            Box<dyn Future<Output = StoreResult<Bytes>> + Send + Sync + 'static>,
        > = unsafe { std::mem::transmute(get_raw_block_fn) };

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
        let get_raw_block_fn: Pin<Box<dyn Future<Output = StoreResult<Bytes>> + Send + Sync>> =
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
        let get_raw_block_fn: Pin<
            Box<dyn Future<Output = StoreResult<Bytes>> + Send + Sync + 'static>,
        > = unsafe { std::mem::transmute(get_raw_block_fn) };

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
                    self.chunk_distance -= self.node.children[self.chunk_index as usize].1 as u64;
                    self.chunk_index -= 1;

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

impl Layout for FlatLayout {
    async fn organize<'a>(
        &self,
        mut stream: BoxStream<'a, StoreResult<Bytes>>,
        store: impl IpldStore + Send + 'a,
    ) -> StoreResult<BoxStream<'a, StoreResult<Cid>>> {
        let s = try_stream! {
            let mut children = Vec::new();
            while let Some(Ok(chunk)) = stream.next().await {
                let len = chunk.len();
                let cid = store.put_raw_block(chunk).await?;
                children.push((cid, len));
                yield cid;
            }

            let node = MerkleNode::new(children);
            let cid = store.put_node(&node).await?;

            yield cid;
        };

        Ok(Box::pin(s))
    }

    async fn retrieve<'a>(
        &self,
        cid: &Cid,
        store: impl IpldStore + Send + Sync + 'a,
    ) -> StoreResult<Pin<Box<dyn AsyncRead + Send + Sync + 'a>>> {
        let node = store.get_node(cid).await?;
        let reader = FlatLayoutReader::new(node, store)?;
        Ok(Box::pin(reader))
    }
}

impl LayoutSeekable for FlatLayout {
    async fn retrieve_seekable<'a>(
        &self,
        cid: &'a Cid,
        store: impl IpldStore + Send + Sync + 'a,
    ) -> StoreResult<Pin<Box<dyn SeekableReader + Send + 'a>>> {
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
    use futures::TryStreamExt;
    use tokio::io::{AsyncReadExt, AsyncSeekExt};

    use crate::MemoryStore;

    use super::*;

    #[tokio::test]
    async fn test_flat_dag_layout_organize_and_retrieve() -> anyhow::Result<()> {
        let store = MemoryStore::default();
        let (data, _, chunk_stream) = fixtures::data_and_chunk_stream();

        // Organize chunks into a DAG.
        let layout = FlatLayout::default();
        let cid_stream = layout.organize(chunk_stream, store.clone()).await?;

        // Get the CID of the merkle node.
        let cids = cid_stream.try_collect::<Vec<_>>().await?;
        let cid = cids.last().unwrap();

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
    async fn test_flat_dag_layout_seek() -> anyhow::Result<()> {
        let store = MemoryStore::default();
        let (_, chunks, chunk_stream) = fixtures::data_and_chunk_stream();

        // Organize chunks into a DAG.
        let layout = FlatLayout::default();
        let cid_stream = layout.organize(chunk_stream, store.clone()).await?;

        // Get the CID of the first chunk.
        let cids = cid_stream.try_collect::<Vec<_>>().await?;
        let cid = cids.last().unwrap();

        // Get seekable reader.
        let mut reader = layout.retrieve_seekable(cid, store).await?;

        // Case: read the first chunk.
        let mut buf = vec![0; 5];
        reader.read(&mut buf).await?;

        assert_eq!(buf, chunks[0]);

        // Case: skip a chunk by seeking from current and have cursor be at boundary of chunk.
        let mut buf = vec![0; 5];
        reader.seek(SeekFrom::Current(5)).await?;
        reader.read(&mut buf).await?;

        assert_eq!(buf, chunks[2]);

        // Case: seek to the next chunk from current and have cursor be in the middle of chunk.
        let mut buf = vec![0; 3];
        reader.seek(SeekFrom::Current(3)).await?;
        reader.read(&mut buf).await?;

        assert_eq!(buf, chunks[3][3..]);

        // Case: Seek to some chunk before end.
        let mut buf = vec![0; 5];
        reader.seek(SeekFrom::End(-5)).await?;
        reader.read(&mut buf).await?;

        assert_eq!(buf, chunks[9]);

        // Case: Seek to some chunk after start.
        let mut buf = vec![0; 5];
        reader.seek(SeekFrom::Start(5)).await?;
        reader.read(&mut buf).await?;

        assert_eq!(buf, chunks[1]);

        // Case: Fail: Seek beyond end.
        let result = reader.seek(SeekFrom::End(5)).await;
        assert!(result.is_err());

        let result = reader.seek(SeekFrom::End(0)).await;
        assert!(result.is_err());

        let result = reader.seek(SeekFrom::Start(100)).await;
        assert!(result.is_err());

        let result = reader.seek(SeekFrom::Current(100)).await;
        assert!(result.is_err());

        // Case: Fail: Seek before start.
        let result = reader.seek(SeekFrom::Current(-100)).await;
        assert!(result.is_err());

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
            Bytes::from("Lorem"),
            Bytes::from(" ipsu"),
            Bytes::from("m dol"),
            Bytes::from("or sit"),
            Bytes::from(" amet,"),
            Bytes::from(" conse"),
            Bytes::from("ctetur"),
            Bytes::from(" adipi"),
            Bytes::from("scing "),
            Bytes::from("elit."),
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
