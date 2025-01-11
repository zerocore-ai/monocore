use crate::error::Result;
use crate::messages::RaftMessage;
use crate::node::NodeId;
use async_trait::async_trait;

#[async_trait]
pub trait Transport: Send + Sync {
    /// Send a message to a specific node
    async fn send_message(&self, to: NodeId, msg: RaftMessage) -> Result<()>;

    /// Broadcast a message to all nodes in the cluster
    async fn broadcast(&self, msg: RaftMessage) -> Result<()>;
}
