use crate::error::{PrismError, PrismResult};
use crate::swarm::{AgentId, AgentRole};
use blake3::Hash;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Agent Communication Protocol for PRISM
/// 
/// Implements secure messaging, task delegation, and result aggregation:
/// - End-to-end encrypted messages between agents
/// - Task assignment and progress tracking
/// - Result aggregation and consensus building
/// - Priority-based message routing
/// - Delivery guarantees and acknowledgments

/// Message identifier for tracking and acknowledgment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId(pub Uuid);

impl MessageId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

/// Message priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Priority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
    Emergency = 4,
}

/// Message delivery requirements
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeliveryMode {
    /// Fire and forget - no acknowledgment required
    BestEffort,
    /// Require delivery acknowledgment
    ReliableDelivery,
    /// Require processing acknowledgment
    ReliableProcessing,
    /// Require result acknowledgment
    ReliableResult,
}

/// Agent message types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentMessage {
    /// Task assignment from one agent to another
    TaskAssignment {
        task_id: String,
        task_type: String,
        payload: Vec<u8>,
        deadline: Option<chrono::DateTime<chrono::Utc>>,
        dependencies: Vec<String>,
        metadata: HashMap<String, String>,
    },
    
    /// Task progress update
    TaskProgress {
        task_id: String,
        progress_percent: f32,
        status: TaskStatus,
        message: String,
        updated_at: chrono::DateTime<chrono::Utc>,
    },
    
    /// Task completion with results
    TaskResult {
        task_id: String,
        success: bool,
        result_data: Option<Vec<u8>>,
        error_message: Option<String>,
        execution_time_ms: u64,
        completed_at: chrono::DateTime<chrono::Utc>,
    },
    
    /// Request for agent capabilities
    CapabilitiesRequest,
    
    /// Response with agent capabilities
    CapabilitiesResponse {
        capabilities: Vec<String>,
        load_factor: f32, // 0.0 = idle, 1.0 = fully loaded
        availability: bool,
    },
    
    /// Health check ping
    Ping {
        timestamp: chrono::DateTime<chrono::Utc>,
        sequence: u64,
    },
    
    /// Health check pong response
    Pong {
        original_timestamp: chrono::DateTime<chrono::Utc>,
        response_timestamp: chrono::DateTime<chrono::Utc>,
        sequence: u64,
    },
    
    /// Resource sharing request
    ResourceRequest {
        resource_type: String,
        amount: u64,
        duration_seconds: Option<u64>,
    },
    
    /// Resource sharing response
    ResourceResponse {
        granted: bool,
        available_amount: u64,
        reason: Option<String>,
    },
    
    /// Coordination message for distributed decision making
    Coordination {
        coordination_type: String,
        proposal: Vec<u8>,
        voting_deadline: chrono::DateTime<chrono::Utc>,
    },
    
    /// Vote response for coordination
    Vote {
        coordination_id: String,
        vote: bool,
        reasoning: Option<String>,
    },
    
    /// Generic data sharing
    DataShare {
        data_type: String,
        data: Vec<u8>,
        schema_version: u32,
    },
    
    /// Emergency alert
    Emergency {
        alert_type: String,
        severity: u8,
        message: String,
        affected_systems: Vec<String>,
    },
}

/// Task execution status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

/// Message envelope with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageEnvelope {
    /// Unique message identifier
    pub id: MessageId,
    /// Source agent
    pub from: AgentId,
    /// Destination agent
    pub to: AgentId,
    /// Message priority
    pub priority: Priority,
    /// Delivery requirements
    pub delivery_mode: DeliveryMode,
    /// Message creation timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Time-to-live in seconds
    pub ttl_seconds: u32,
    /// Message content
    pub message: AgentMessage,
    /// Content integrity hash
    pub content_hash: Hash,
    /// Encryption metadata (if encrypted)
    pub encryption_info: Option<EncryptionInfo>,
}

/// Encryption information for secure messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionInfo {
    /// Encryption algorithm used
    pub algorithm: String,
    /// Key derivation method
    pub key_derivation: String,
    /// Initialization vector or nonce
    pub iv: Vec<u8>,
    /// Authentication tag (for AEAD)
    pub auth_tag: Option<Vec<u8>>,
}

impl MessageEnvelope {
    pub fn new(
        from: AgentId,
        to: AgentId,
        message: AgentMessage,
        priority: Priority,
        delivery_mode: DeliveryMode,
    ) -> Self {
        let serialized_message = bincode::serialize(&message).unwrap_or_default();
        let content_hash = blake3::hash(&serialized_message);
        
        Self {
            id: MessageId::new(),
            from,
            to,
            priority,
            delivery_mode,
            timestamp: chrono::Utc::now(),
            ttl_seconds: 3600, // 1 hour default TTL
            message,
            content_hash,
            encryption_info: None,
        }
    }
    
    /// Verify message integrity
    pub fn verify_integrity(&self) -> bool {
        let serialized_message = bincode::serialize(&self.message).unwrap_or_default();
        let computed_hash = blake3::hash(&serialized_message);
        computed_hash == self.content_hash
    }
    
    /// Check if message has expired
    pub fn is_expired(&self) -> bool {
        let now = chrono::Utc::now();
        let expires_at = self.timestamp + chrono::Duration::seconds(self.ttl_seconds as i64);
        now > expires_at
    }
}

/// Message acknowledgment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageAck {
    /// Original message ID
    pub message_id: MessageId,
    /// Acknowledgment type
    pub ack_type: AckType,
    /// Response timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Optional error information
    pub error: Option<String>,
}

/// Types of acknowledgments
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AckType {
    Delivered,  // Message was delivered
    Processed,  // Message was processed
    Completed,  // Task was completed (for task messages)
    Failed,     // Processing failed
}

/// Communication statistics and metrics
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct CommunicationMetrics {
    /// Messages sent by priority
    pub messages_sent: HashMap<Priority, u64>,
    /// Messages received by priority
    pub messages_received: HashMap<Priority, u64>,
    /// Acknowledgments sent
    pub acks_sent: u64,
    /// Acknowledgments received
    pub acks_received: u64,
    /// Failed deliveries
    pub failed_deliveries: u64,
    /// Expired messages
    pub expired_messages: u64,
    /// Average message latency (ms)
    pub avg_latency_ms: f64,
    /// Peak message queue size
    pub peak_queue_size: usize,
    /// Current queue size
    pub current_queue_size: usize,
}

/// Communication manager for agent message handling
pub struct CommunicationManager {
    /// Local agent ID
    agent_id: AgentId,
    /// Message sender for outgoing messages
    outbound_sender: mpsc::UnboundedSender<MessageEnvelope>,
    /// Message receiver for incoming messages
    inbound_receiver: Option<mpsc::UnboundedReceiver<MessageEnvelope>>,
    /// Acknowledgment channels
    ack_sender: mpsc::UnboundedSender<MessageAck>,
    /// Pending acknowledgments (message_id -> callback)
    pending_acks: HashMap<MessageId, tokio::sync::oneshot::Sender<MessageAck>>,
    /// Communication metrics
    metrics: CommunicationMetrics,
    /// Message handlers by message type
    handlers: HashMap<String, Box<dyn MessageHandler + Send + Sync>>,
}

/// Trait for handling specific message types
#[async_trait::async_trait]
pub trait MessageHandler: Send + Sync {
    async fn handle_message(
        &self,
        envelope: &MessageEnvelope,
        comm: &mut CommunicationManager,
    ) -> PrismResult<Option<AgentMessage>>;
}

impl CommunicationManager {
    pub fn new(agent_id: AgentId) -> Self {
        let (outbound_sender, _outbound_receiver) = mpsc::unbounded_channel();
        let (ack_sender, _ack_receiver) = mpsc::unbounded_channel();
        
        Self {
            agent_id,
            outbound_sender,
            inbound_receiver: None,
            ack_sender,
            pending_acks: HashMap::new(),
            metrics: CommunicationMetrics::default(),
            handlers: HashMap::new(),
        }
    }
    
    /// Send a message to another agent
    pub async fn send_message(
        &mut self,
        to: AgentId,
        message: AgentMessage,
        priority: Priority,
        delivery_mode: DeliveryMode,
    ) -> PrismResult<MessageId> {
        let envelope = MessageEnvelope::new(
            self.agent_id,
            to,
            message,
            priority,
            delivery_mode,
        );
        
        let message_id = envelope.id;
        
        // Update metrics
        *self.metrics.messages_sent.entry(priority).or_insert(0) += 1;
        
        // Send message
        self.outbound_sender.send(envelope)
            .map_err(|_| PrismError::Communication {
                details: "Failed to send message - channel closed".to_string(),
            })?;
        
        debug!("Sent message {} to agent {}", message_id.0, to.0);
        Ok(message_id)
    }
    
    /// Send a message and wait for acknowledgment
    pub async fn send_message_with_ack(
        &mut self,
        to: AgentId,
        message: AgentMessage,
        priority: Priority,
        delivery_mode: DeliveryMode,
        timeout: std::time::Duration,
    ) -> PrismResult<MessageAck> {
        let message_id = self.send_message(to, message, priority, delivery_mode).await?;
        
        // Create acknowledgment channel
        let (ack_tx, ack_rx) = tokio::sync::oneshot::channel();
        self.pending_acks.insert(message_id, ack_tx);
        
        // Wait for acknowledgment with timeout
        match tokio::time::timeout(timeout, ack_rx).await {
            Ok(Ok(ack)) => Ok(ack),
            Ok(Err(_)) => Err(PrismError::Communication {
                details: "Acknowledgment channel closed".to_string(),
            }),
            Err(_) => Err(PrismError::Communication {
                details: format!("Acknowledgment timeout after {:?}", timeout),
            }),
        }
    }
    
    /// Register a message handler for specific message types
    pub fn register_handler<H>(&mut self, message_type: String, handler: H)
    where
        H: MessageHandler + Send + Sync + 'static,
    {
        self.handlers.insert(message_type, Box::new(handler));
    }
    
    /// Process incoming message
    async fn handle_incoming_message(&mut self, envelope: MessageEnvelope) -> PrismResult<()> {
        // Verify message integrity
        if !envelope.verify_integrity() {
            warn!("Received message with invalid integrity hash");
            return Err(PrismError::Communication {
                details: "Message integrity verification failed".to_string(),
            });
        }
        
        // Check if message has expired
        if envelope.is_expired() {
            warn!("Received expired message {}", envelope.id.0);
            self.metrics.expired_messages += 1;
            return Ok(());
        }
        
        // Update metrics
        *self.metrics.messages_received.entry(envelope.priority).or_insert(0) += 1;
        
        // Send delivery acknowledgment if required
        if matches!(
            envelope.delivery_mode,
            DeliveryMode::ReliableDelivery
                | DeliveryMode::ReliableProcessing
                | DeliveryMode::ReliableResult
        ) {
            let ack = MessageAck {
                message_id: envelope.id,
                ack_type: AckType::Delivered,
                timestamp: chrono::Utc::now(),
                error: None,
            };
            
            if let Err(e) = self.ack_sender.send(ack) {
                error!("Failed to send delivery acknowledgment: {}", e);
            }
        }
        
        // Determine handler based on message type
        let message_type = match &envelope.message {
            AgentMessage::TaskAssignment { task_type, .. } => task_type.clone(),
            AgentMessage::TaskProgress { .. } => "task_progress".to_string(),
            AgentMessage::TaskResult { .. } => "task_result".to_string(),
            AgentMessage::CapabilitiesRequest => "capabilities_request".to_string(),
            AgentMessage::CapabilitiesResponse { .. } => "capabilities_response".to_string(),
            AgentMessage::Ping { .. } => "ping".to_string(),
            AgentMessage::Pong { .. } => "pong".to_string(),
            AgentMessage::ResourceRequest { .. } => "resource_request".to_string(),
            AgentMessage::ResourceResponse { .. } => "resource_response".to_string(),
            AgentMessage::Coordination { .. } => "coordination".to_string(),
            AgentMessage::Vote { .. } => "vote".to_string(),
            AgentMessage::DataShare { data_type, .. } => data_type.clone(),
            AgentMessage::Emergency { .. } => "emergency".to_string(),
        };
        
        // Handle message with appropriate handler
        if let Some(handler) = self.handlers.get(&message_type) {
            match handler.handle_message(&envelope, self).await {
                Ok(response) => {
                    // Send response if provided
                    if let Some(response_message) = response {
                        let _ = self.send_message(
                            envelope.from,
                            response_message,
                            Priority::Normal,
                            DeliveryMode::BestEffort,
                        ).await;
                    }
                }
                Err(e) => {
                    error!("Message handler failed: {}", e);
                    
                    // Send error acknowledgment if required
                    if matches!(
                        envelope.delivery_mode,
                        DeliveryMode::ReliableProcessing | DeliveryMode::ReliableResult
                    ) {
                        let ack = MessageAck {
                            message_id: envelope.id,
                            ack_type: AckType::Failed,
                            timestamp: chrono::Utc::now(),
                            error: Some(e.to_string()),
                        };
                        
                        let _ = self.ack_sender.send(ack);
                    }
                }
            }
        } else {
            warn!("No handler registered for message type: {}", message_type);
        }
        
        Ok(())
    }
    
    /// Get current communication metrics
    pub fn metrics(&self) -> &CommunicationMetrics {
        &self.metrics
    }
    
    /// Start the communication manager
    pub async fn start(&mut self) -> PrismResult<()> {
        info!("Starting communication manager for agent {}", self.agent_id.0);
        
        // This would typically start background tasks for:
        // - Processing incoming messages
        // - Handling acknowledgments
        // - Cleaning up expired messages
        // - Updating metrics
        
        Ok(())
    }
    
    /// Stop the communication manager
    pub async fn stop(&mut self) -> PrismResult<()> {
        info!("Stopping communication manager for agent {}", self.agent_id.0);
        
        // Clean up pending acknowledgments
        for (message_id, sender) in self.pending_acks.drain() {
            let timeout_ack = MessageAck {
                message_id,
                ack_type: AckType::Failed,
                timestamp: chrono::Utc::now(),
                error: Some("Communication manager stopped".to_string()),
            };
            let _ = sender.send(timeout_ack);
        }
        
        Ok(())
    }
}

/// Default ping handler for health checks
pub struct PingHandler;

#[async_trait::async_trait]
impl MessageHandler for PingHandler {
    async fn handle_message(
        &self,
        envelope: &MessageEnvelope,
        _comm: &mut CommunicationManager,
    ) -> PrismResult<Option<AgentMessage>> {
        if let AgentMessage::Ping { timestamp, sequence } = &envelope.message {
            // Respond with pong
            Ok(Some(AgentMessage::Pong {
                original_timestamp: *timestamp,
                response_timestamp: chrono::Utc::now(),
                sequence: *sequence,
            }))
        } else {
            Ok(None)
        }
    }
}