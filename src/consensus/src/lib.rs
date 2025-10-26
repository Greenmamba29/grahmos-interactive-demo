use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

pub mod raft;
pub mod log;
pub mod storage;
pub mod messages;

pub use raft::*;
pub use log::*;
pub use storage::*;
pub use messages::*;

/// Raft-based Consensus for PRISM Agent Swarm
/// 
/// Implements distributed consensus using the Raft algorithm adapted for agent coordination:
/// - Leader election with randomized timeouts
/// - Log replication across agent swarm
/// - Strong consistency guarantees
/// - Partition tolerance and fault recovery
/// - Agent-specific optimizations for task delegation

/// Node identifier for consensus participants
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NodeId(pub Uuid);

impl NodeId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
    
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", &self.0.to_string()[..8])
    }
}

/// Raft node states
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeState {
    /// Follower state - responds to leader requests
    Follower,
    /// Candidate state - competing for leadership
    Candidate,
    /// Leader state - replicates log to followers
    Leader,
}

/// Raft term number (monotonically increasing)
pub type Term = u64;

/// Log index (position in replicated log)
pub type LogIndex = u64;

/// Agent task or command to be replicated
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentCommand {
    /// Assign task to specific agent
    AssignTask {
        agent_id: NodeId,
        task_id: String,
        task_data: Vec<u8>,
        priority: u8,
    },
    
    /// Update agent status
    UpdateStatus {
        agent_id: NodeId,
        status: AgentStatus,
        metadata: HashMap<String, String>,
    },
    
    /// Register new agent in swarm
    RegisterAgent {
        agent_id: NodeId,
        agent_type: AgentType,
        capabilities: Vec<String>,
    },
    
    /// Remove agent from swarm
    UnregisterAgent {
        agent_id: NodeId,
        reason: String,
    },
    
    /// Configure swarm parameters
    UpdateConfig {
        key: String,
        value: serde_json::Value,
    },
    
    /// Custom application command
    Custom {
        command_type: String,
        payload: Vec<u8>,
    },
}

/// Agent status enumeration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentStatus {
    Online,
    Busy,
    Offline,
    Error(String),
}

/// Agent type classification
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentType {
    CTO,      // Chief Technology Officer - technical decisions
    PM,       // Product Manager - requirements and strategy  
    QA,       // Quality Assurance - testing and validation
    Dev,      // Developer - implementation
    Ops,      // Operations - deployment and monitoring
    Custom(String), // Extensible for future agent types
}

/// Raft log entry containing command and metadata
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogEntry {
    /// Raft term when entry was created
    pub term: Term,
    /// Index in the log
    pub index: LogIndex,
    /// Command to apply
    pub command: AgentCommand,
    /// Timestamp when entry was created
    pub timestamp: DateTime<Utc>,
    /// Node that originated this entry
    pub origin: NodeId,
    /// Content hash for integrity
    pub hash: blake3::Hash,
}

impl LogEntry {
    pub fn new(term: Term, index: LogIndex, command: AgentCommand, origin: NodeId) -> Self {
        let timestamp = Utc::now();
        let serialized = bincode::serialize(&(&term, &index, &command, &timestamp, &origin))
            .unwrap_or_default();
        let hash = blake3::hash(&serialized);
        
        Self {
            term,
            index,
            command,
            timestamp,
            origin,
            hash,
        }
    }
    
    pub fn verify_hash(&self) -> bool {
        let serialized = bincode::serialize(&(&self.term, &self.index, &self.command, &self.timestamp, &self.origin))
            .unwrap_or_default();
        let computed_hash = blake3::hash(&serialized);
        computed_hash == self.hash
    }
}

/// Consensus configuration
#[derive(Debug, Clone)]
pub struct ConsensusConfig {
    /// Election timeout range (min, max) in milliseconds
    pub election_timeout_ms: (u64, u64),
    /// Heartbeat interval in milliseconds
    pub heartbeat_interval_ms: u64,
    /// Maximum entries per AppendEntries RPC
    pub max_entries_per_message: usize,
    /// Snapshot threshold (log entries before compaction)
    pub snapshot_threshold: LogIndex,
    /// Maximum time to wait for votes during election
    pub vote_timeout_ms: u64,
    /// Network timeout for RPCs
    pub rpc_timeout_ms: u64,
}

impl Default for ConsensusConfig {
    fn default() -> Self {
        Self {
            election_timeout_ms: (150, 300), // Raft paper recommendations
            heartbeat_interval_ms: 50,       // Frequent heartbeats for responsiveness
            max_entries_per_message: 100,    // Batch optimization
            snapshot_threshold: 10000,       // Compact after 10K entries
            vote_timeout_ms: 1000,           // 1 second for vote collection
            rpc_timeout_ms: 5000,           // 5 seconds for network operations
        }
    }
}

/// Consensus errors
#[derive(thiserror::Error, Debug)]
pub enum ConsensusError {
    #[error("Node is not the leader")]
    NotLeader,
    
    #[error("Election timeout")]
    ElectionTimeout,
    
    #[error("Log inconsistency: expected index {expected}, got {actual}")]
    LogInconsistency { expected: LogIndex, actual: LogIndex },
    
    #[error("Invalid term: current {current}, received {received}")]
    InvalidTerm { current: Term, received: Term },
    
    #[error("Storage error: {0}")]
    Storage(String),
    
    #[error("Network error: {0}")]
    Network(String),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),
    
    #[error("Timeout: operation took longer than {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },
    
    #[error("Node {node_id} not found in cluster")]
    NodeNotFound { node_id: NodeId },
    
    #[error("Cluster not ready: only {ready_nodes} of {total_nodes} nodes available")]
    ClusterNotReady { ready_nodes: usize, total_nodes: usize },
}

pub type ConsensusResult<T> = Result<T, ConsensusError>;

/// Consensus statistics and metrics
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ConsensusMetrics {
    /// Current term
    pub current_term: Term,
    /// Current node state
    pub state: Option<NodeState>,
    /// Current leader (if known)
    pub leader_id: Option<NodeId>,
    /// Last log index
    pub last_log_index: LogIndex,
    /// Commit index (last committed entry)
    pub commit_index: LogIndex,
    /// Number of cluster nodes
    pub cluster_size: usize,
    /// Elections held
    pub elections_held: u64,
    /// Messages sent
    pub messages_sent: u64,
    /// Messages received
    pub messages_received: u64,
    /// Commands applied
    pub commands_applied: u64,
    /// Leadership changes
    pub leadership_changes: u64,
    /// Average command latency (ms)
    pub avg_command_latency_ms: f64,
}

/// Main consensus node interface
pub trait ConsensusNode {
    /// Submit a command for replication
    async fn submit_command(&mut self, command: AgentCommand) -> ConsensusResult<LogIndex>;
    
    /// Get current metrics
    fn metrics(&self) -> &ConsensusMetrics;
    
    /// Check if this node is the current leader
    fn is_leader(&self) -> bool;
    
    /// Get current leader ID
    fn leader_id(&self) -> Option<NodeId>;
    
    /// Get cluster membership
    fn cluster_nodes(&self) -> Vec<NodeId>;
    
    /// Add node to cluster
    async fn add_node(&mut self, node_id: NodeId) -> ConsensusResult<()>;
    
    /// Remove node from cluster
    async fn remove_node(&mut self, node_id: NodeId) -> ConsensusResult<()>;
    
    /// Trigger snapshot creation
    async fn create_snapshot(&mut self) -> ConsensusResult<()>;
    
    /// Install snapshot from leader
    async fn install_snapshot(&mut self, snapshot_data: Vec<u8>) -> ConsensusResult<()>;
    
    /// Start the consensus algorithm
    async fn start(&mut self) -> ConsensusResult<()>;
    
    /// Stop the consensus algorithm
    async fn stop(&mut self) -> ConsensusResult<()>;
}

/// Builder for creating consensus nodes
pub struct ConsensusBuilder {
    node_id: NodeId,
    config: ConsensusConfig,
    initial_cluster: Vec<NodeId>,
}

impl ConsensusBuilder {
    pub fn new(node_id: NodeId) -> Self {
        Self {
            node_id,
            config: ConsensusConfig::default(),
            initial_cluster: Vec::new(),
        }
    }
    
    pub fn config(mut self, config: ConsensusConfig) -> Self {
        self.config = config;
        self
    }
    
    pub fn cluster_nodes(mut self, nodes: Vec<NodeId>) -> Self {
        self.initial_cluster = nodes;
        self
    }
    
    pub fn build(self) -> ConsensusResult<impl ConsensusNode> {
        RaftNode::new(self.node_id, self.config, self.initial_cluster)
    }
}