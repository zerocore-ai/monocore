use crate::error::Result;
use crate::messages::RaftMessage;
use crate::storage::RaftStorage;
use crate::transport::Transport;
use std::time::Duration;
use tokio::sync::mpsc;

pub type NodeId = u64;

#[derive(Debug, Clone, PartialEq)]
pub enum Role {
    Follower,
    Candidate,
    Leader,
}

pub struct RaftState {
    pub current_term: u64,
    pub voted_for: Option<NodeId>,
    pub commit_index: u64,
    pub last_applied: u64,
    pub next_index: Vec<u64>, // Leader only: next index to send to each follower
    pub match_index: Vec<u64>, // Leader only: highest log entry known to be replicated
}

pub struct RaftConfig {
    pub election_timeout_min: Duration,
    pub election_timeout_max: Duration,
    pub heartbeat_interval: Duration,
}

impl Default for RaftConfig {
    fn default() -> Self {
        Self {
            election_timeout_min: Duration::from_millis(150),
            election_timeout_max: Duration::from_millis(300),
            heartbeat_interval: Duration::from_millis(50),
        }
    }
}

pub struct RaftNode<T, S>
where
    T: Transport,
    S: RaftStorage,
{
    pub id: NodeId,
    pub role: Role,
    pub state: RaftState,
    pub config: RaftConfig,
    pub transport: T,
    pub storage: S,
    pub cluster_members: Vec<NodeId>,
    pub inbound_rx: mpsc::Receiver<RaftMessage>,
    pub inbound_tx: mpsc::Sender<RaftMessage>,
}

impl<T, S> RaftNode<T, S>
where
    T: Transport,
    S: RaftStorage,
{
    pub fn new(
        id: NodeId,
        transport: T,
        storage: S,
        cluster_members: Vec<NodeId>,
        config: RaftConfig,
    ) -> Self {
        let (tx, rx) = mpsc::channel(1000);

        Self {
            id,
            role: Role::Follower,
            state: RaftState {
                current_term: 0,
                voted_for: None,
                commit_index: 0,
                last_applied: 0,
                next_index: vec![],
                match_index: vec![],
            },
            config,
            transport,
            storage,
            cluster_members,
            inbound_rx: rx,
            inbound_tx: tx,
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        // Initialize state from storage
        let (term, voted_for) = self.storage.read_metadata().await?;
        self.state.current_term = term;
        self.state.voted_for = voted_for;

        // Start main loop
        self.run().await
    }

    async fn run(&mut self) -> Result<()> {
        loop {
            match self.role {
                Role::Follower => self.run_follower().await?,
                Role::Candidate => self.run_candidate().await?,
                Role::Leader => self.run_leader().await?,
            }
        }
    }

    async fn run_follower(&mut self) -> Result<()> {
        // TODO: Implement follower logic
        Ok(())
    }

    async fn run_candidate(&mut self) -> Result<()> {
        // TODO: Implement candidate logic
        Ok(())
    }

    async fn run_leader(&mut self) -> Result<()> {
        // TODO: Implement leader logic
        Ok(())
    }
}
