use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::time::{Duration, Instant};
use uuid::Uuid;

pub mod agent;
pub mod swarm_manager;
pub mod lifecycle;
pub mod heartbeat;

pub use agent::*;
pub use swarm_manager::*;

// Re-export core types from error module
pub use crate::error::{PrismError, PrismResult};

/// Unique identifier for agents in the swarm
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(Uuid);

impl AgentId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
    
    pub fn from_string(s: &str) -> PrismResult<Self> {
        let uuid = Uuid::parse_str(s)
            .map_err(|_| PrismError::Validation {
                message: format!("Invalid agent ID format: {}", s)
            })?;
        Ok(Self(uuid))
    }
    
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for AgentId {
    fn default() -> Self {
        Self::new()
    }
}

/// Role of an agent in the swarm (from PRISM PRD)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentRole {
    /// Master agents orchestrate local replication tasks (3-5 per site)
    Master,
    /// Worker agents monitor specific systems/services (1-N per system)  
    Worker,
    /// Gateway agents bridge between sites (inter-site P2P/mesh)
    Gateway,
    /// Mobile agents for lightweight monitoring on mobile devices
    Mobile,
}

impl fmt::Display for AgentRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentRole::Master => write!(f, "master"),
            AgentRole::Worker => write!(f, "worker"),
            AgentRole::Gateway => write!(f, "gateway"),
            AgentRole::Mobile => write!(f, "mobile"),
        }
    }
}

/// Current operational status of an agent
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentStatus {
    /// Agent is initializing (startup phase)
    Initializing,
    /// Agent is operational and healthy
    Healthy,
    /// Agent is operational but experiencing issues
    Degraded { reason: String },
    /// Agent has failed and needs intervention
    Failed { reason: String },
    /// Agent is shutting down gracefully
    Stopping,
    /// Agent has stopped
    Stopped,
}

impl fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentStatus::Initializing => write!(f, "initializing"),
            AgentStatus::Healthy => write!(f, "healthy"),
            AgentStatus::Degraded { reason } => write!(f, "degraded: {}", reason),
            AgentStatus::Failed { reason } => write!(f, "failed: {}", reason),
            AgentStatus::Stopping => write!(f, "stopping"),
            AgentStatus::Stopped => write!(f, "stopped"),
        }
    }
}

/// Heartbeat data transmitted between agents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatData {
    pub agent_id: AgentId,
    pub timestamp: DateTime<Utc>,
    pub sequence_number: u64,
    pub status: AgentStatus,
    pub load_metrics: LoadMetrics,
    pub site_id: Option<String>,
}

/// System load and performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadMetrics {
    pub cpu_usage_percent: f32,
    pub memory_usage_bytes: u64,
    pub memory_total_bytes: u64,
    pub disk_usage_percent: f32,
    pub network_latency_ms: f32,
    pub active_connections: u32,
    pub tasks_queued: u32,
}

impl Default for LoadMetrics {
    fn default() -> Self {
        Self {
            cpu_usage_percent: 0.0,
            memory_usage_bytes: 0,
            memory_total_bytes: 0,
            disk_usage_percent: 0.0,
            network_latency_ms: 0.0,
            active_connections: 0,
            tasks_queued: 0,
        }
    }
}

/// Core trait that all PRISM agents must implement
/// 
/// This trait defines the fundamental operations that every agent type
/// (Master, Worker, Gateway, Mobile) must support for swarm coordination.
#[async_trait]
pub trait Agent: Send + Sync {
    /// Unique identifier for this agent instance
    fn agent_id(&self) -> AgentId;
    
    /// Role of this agent in the swarm
    fn role(&self) -> AgentRole;
    
    /// Current operational status
    fn status(&self) -> AgentStatus;
    
    /// Generate heartbeat data for peer communication
    fn heartbeat(&self) -> HeartbeatData;
    
    /// Site identifier for multi-site deployments
    fn site_id(&self) -> Option<String> {
        None
    }
    
    /// Start the agent and all its subsystems
    async fn start(&mut self) -> PrismResult<()>;
    
    /// Stop the agent gracefully  
    async fn stop(&mut self) -> PrismResult<()>;
    
    /// Handle incoming message from another agent
    async fn handle_message(&mut self, message: AgentMessage) -> PrismResult<()>;
    
    /// Periodic health check and maintenance
    async fn health_check(&mut self) -> PrismResult<AgentStatus>;
    
    /// Get current performance metrics
    fn load_metrics(&self) -> LoadMetrics {
        LoadMetrics::default()
    }
}

/// Messages exchanged between agents in the swarm
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentMessage {
    /// Heartbeat message with agent status
    Heartbeat(HeartbeatData),
    
    /// Request to join the swarm
    JoinRequest {
        agent_id: AgentId,
        role: AgentRole,
        site_id: Option<String>,
        capabilities: Vec<String>,
    },
    
    /// Response to join request
    JoinResponse {
        accepted: bool,
        reason: Option<String>,
        cluster_info: Option<ClusterInfo>,
    },
    
    /// Notification of agent leaving
    LeaveNotification {
        agent_id: AgentId,
        reason: String,
    },
    
    /// Request for current cluster state
    StateRequest,
    
    /// Response with cluster state information
    StateResponse {
        agents: Vec<AgentInfo>,
        cluster_leader: Option<AgentId>,
        consensus_view: u64,
    },
    
    /// Task assignment for worker agents
    TaskAssignment {
        task_id: String,
        task_type: String,
        parameters: HashMap<String, String>,
        priority: TaskPriority,
    },
    
    /// Task completion notification
    TaskCompletion {
        task_id: String,
        result: TaskResult,
    },
}

/// Basic information about an agent in the cluster
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub agent_id: AgentId,
    pub role: AgentRole,
    pub status: AgentStatus,
    pub site_id: Option<String>,
    pub last_heartbeat: DateTime<Utc>,
    pub load_metrics: LoadMetrics,
    pub capabilities: Vec<String>,
}

/// Information about the overall cluster
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterInfo {
    pub cluster_id: String,
    pub total_agents: usize,
    pub healthy_agents: usize,
    pub sites: Vec<String>,
    pub leader_agent: Option<AgentId>,
    pub consensus_view: u64,
}

/// Task priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskPriority {
    Low,
    Normal,
    High,
    Critical,
}

/// Result of task execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskResult {
    Success {
        duration_ms: u64,
        output: Option<String>,
    },
    Failure {
        error: String,
        retry_count: u32,
    },
    Timeout {
        duration_ms: u64,
    },
}

/// Configuration for agent behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub agent_id: Option<AgentId>,
    pub role: AgentRole,
    pub site_id: Option<String>,
    pub heartbeat_interval: Duration,
    pub heartbeat_timeout: Duration,
    pub max_retry_count: u32,
    pub capabilities: Vec<String>,
    pub resource_limits: ResourceLimits,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            agent_id: None,
            role: AgentRole::Worker,
            site_id: None,
            heartbeat_interval: Duration::from_millis(50), // 50ms from PRD
            heartbeat_timeout: Duration::from_millis(150), // 3x heartbeat interval
            max_retry_count: 3,
            capabilities: vec![],
            resource_limits: ResourceLimits::default(),
        }
    }
}

/// Resource limits for agent operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub max_memory_bytes: u64,
    pub max_cpu_percent: f32,
    pub max_disk_usage_percent: f32,
    pub max_network_bandwidth_mbps: u32,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_memory_bytes: 2 * 1024 * 1024 * 1024, // 2GB from PRD target
            max_cpu_percent: 80.0,
            max_disk_usage_percent: 90.0,
            max_network_bandwidth_mbps: 100,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_id_creation() {
        let id1 = AgentId::new();
        let id2 = AgentId::new();
        assert_ne!(id1, id2);
        
        let id_str = id1.to_string();
        let id3 = AgentId::from_string(&id_str).unwrap();
        assert_eq!(id1, id3);
    }

    #[test]
    fn test_agent_role_display() {
        assert_eq!(AgentRole::Master.to_string(), "master");
        assert_eq!(AgentRole::Worker.to_string(), "worker");
        assert_eq!(AgentRole::Gateway.to_string(), "gateway");  
        assert_eq!(AgentRole::Mobile.to_string(), "mobile");
    }

    #[test]
    fn test_agent_status_serialization() {
        let status = AgentStatus::Degraded {
            reason: "High CPU usage".to_string(),
        };
        
        let json = serde_json::to_string(&status).unwrap();
        let deserialized: AgentStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status, deserialized);
    }

    #[test]
    fn test_heartbeat_data_creation() {
        let agent_id = AgentId::new();
        let heartbeat = HeartbeatData {
            agent_id,
            timestamp: Utc::now(),
            sequence_number: 42,
            status: AgentStatus::Healthy,
            load_metrics: LoadMetrics::default(),
            site_id: Some("site-a".to_string()),
        };
        
        assert_eq!(heartbeat.agent_id, agent_id);
        assert_eq!(heartbeat.sequence_number, 42);
    }
}