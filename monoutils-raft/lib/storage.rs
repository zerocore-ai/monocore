use crate::error::Result;
use crate::messages::LogEntry;
use crate::node::NodeId;
use async_trait::async_trait;

#[async_trait]
pub trait RaftStorage: Send + Sync {
    async fn persist_metadata(
        &mut self,
        current_term: u64,
        voted_for: Option<NodeId>,
    ) -> Result<()>;

    async fn read_metadata(&self) -> Result<(u64, Option<NodeId>)>;

    async fn append_log_entries(&mut self, entries: &[LogEntry]) -> Result<()>;

    async fn read_log_entry(&self, index: u64) -> Result<Option<LogEntry>>;

    async fn get_last_log_entry(&self) -> Result<Option<LogEntry>>;
}
