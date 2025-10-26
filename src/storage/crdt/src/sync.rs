use crate::{CRDT, CRDTResult, CRDTError, ReplicaId, VectorClock};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tokio::sync::mpsc;

/// CRDT Synchronization Protocol for P2P Networks
/// 
/// Implements efficient synchronization of CRDT state across distributed replicas:
/// - Anti-entropy: periodic full state sync
/// - Delta synchronization: incremental updates
/// - Causal consistency: vector clock ordering
/// - Compression: delta compression for bandwidth efficiency

/// Synchronization message types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncMessage<T: CRDT> {
    /// Request state summary for comparison
    StateSummary {
        replica_id: ReplicaId,
        vector_clock: VectorClock,
        content_hash: blake3::Hash,
    },
    
    /// Full state transfer
    FullState {
        replica_id: ReplicaId,
        state: T,
        vector_clock: VectorClock,
    },
    
    /// Delta update since specific state
    DeltaUpdate {
        replica_id: ReplicaId,
        base_vector_clock: VectorClock,
        delta: T, // In practice, would be T::Delta for DeltaCRDT
    },
    
    /// Request for specific state or delta
    StateRequest {
        replica_id: ReplicaId,
        requested_vector_clock: Option<VectorClock>,
    },
    
    /// Acknowledgment of received update
    Acknowledgment {
        replica_id: ReplicaId,
        ack_vector_clock: VectorClock,
    },
}

/// Synchronization strategy configuration
#[derive(Debug, Clone)]
pub struct SyncConfig {
    /// Interval for anti-entropy rounds
    pub anti_entropy_interval_ms: u64,
    /// Maximum message size for delta compression
    pub max_message_size_bytes: usize,
    /// Number of retries for failed synchronization
    pub max_retries: u32,
    /// Timeout for synchronization operations
    pub sync_timeout_ms: u64,
    /// Enable delta compression
    pub enable_delta_compression: bool,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            anti_entropy_interval_ms: 30000, // 30 seconds
            max_message_size_bytes: 1024 * 1024, // 1MB
            max_retries: 3,
            sync_timeout_ms: 10000, // 10 seconds
            enable_delta_compression: true,
        }
    }
}

/// CRDT Synchronization Manager
pub struct SyncManager<T: CRDT> {
    /// Local replica ID
    replica_id: ReplicaId,
    /// Local CRDT state
    local_state: T,
    /// Local vector clock
    local_vector_clock: VectorClock,
    /// Known remote replicas and their states
    peer_states: BTreeMap<ReplicaId, (VectorClock, blake3::Hash)>,
    /// Configuration
    config: SyncConfig,
    /// Message channel for outgoing sync messages
    message_sender: Option<mpsc::UnboundedSender<(ReplicaId, SyncMessage<T>)>>,
}

impl<T: CRDT> SyncManager<T> {
    /// Create new sync manager
    pub fn new(replica_id: ReplicaId, initial_state: T, config: SyncConfig) -> Self {
        let mut local_vector_clock = VectorClock::new();
        local_vector_clock.increment(replica_id);
        
        Self {
            replica_id,
            local_state: initial_state,
            local_vector_clock,
            peer_states: BTreeMap::new(),
            config,
            message_sender: None,
        }
    }
    
    /// Set message sender for outgoing sync messages
    pub fn set_message_sender(&mut self, sender: mpsc::UnboundedSender<(ReplicaId, SyncMessage<T>)>) {
        self.message_sender = Some(sender);
    }
    
    /// Handle incoming synchronization message
    pub fn handle_message(&mut self, from: ReplicaId, message: SyncMessage<T>) -> CRDTResult<()> {
        match message {
            SyncMessage::StateSummary { replica_id, vector_clock, content_hash } => {
                self.handle_state_summary(replica_id, vector_clock, content_hash)
            }
            
            SyncMessage::FullState { replica_id, state, vector_clock } => {
                self.handle_full_state(replica_id, state, vector_clock)
            }
            
            SyncMessage::DeltaUpdate { replica_id, base_vector_clock: _, delta } => {
                self.handle_delta_update(replica_id, delta)
            }
            
            SyncMessage::StateRequest { replica_id, requested_vector_clock } => {
                self.handle_state_request(replica_id, requested_vector_clock)
            }
            
            SyncMessage::Acknowledgment { replica_id, ack_vector_clock } => {
                self.handle_acknowledgment(replica_id, ack_vector_clock)
            }
        }
    }
    
    /// Initiate anti-entropy round with all known peers
    pub fn start_anti_entropy(&mut self) -> CRDTResult<()> {
        let summary = SyncMessage::StateSummary {
            replica_id: self.replica_id,
            vector_clock: self.local_vector_clock.clone(),
            content_hash: self.local_state.content_hash(),
        };
        
        // Send to all known peers
        for &peer_id in self.peer_states.keys() {
            self.send_message(peer_id, summary.clone())?;
        }
        
        Ok(())
    }
    
    /// Add a new peer to synchronize with
    pub fn add_peer(&mut self, peer_id: ReplicaId) {
        self.peer_states.insert(peer_id, (VectorClock::new(), blake3::Hash::from([0; 32])));
        
        // Immediately request state from new peer
        let request = SyncMessage::StateRequest {
            replica_id: self.replica_id,
            requested_vector_clock: None,
        };
        
        if let Err(e) = self.send_message(peer_id, request) {
            eprintln!("Failed to send state request to new peer {}: {}", peer_id, e);
        }
    }
    
    /// Remove a peer
    pub fn remove_peer(&mut self, peer_id: &ReplicaId) {
        self.peer_states.remove(peer_id);
    }
    
    /// Get current local state
    pub fn local_state(&self) -> &T {
        &self.local_state
    }
    
    /// Update local state (after local operation)
    pub fn update_local_state(&mut self, new_state: T) {
        self.local_state = new_state;
        self.local_vector_clock.increment(self.replica_id);
    }
    
    /// Get synchronization statistics
    pub fn sync_statistics(&self) -> SyncStatistics {
        SyncStatistics {
            local_replica_id: self.replica_id,
            peer_count: self.peer_states.len(),
            local_vector_clock: self.local_vector_clock.clone(),
        }
    }
    
    // Private helper methods
    
    fn handle_state_summary(&mut self, replica_id: ReplicaId, vector_clock: VectorClock, content_hash: blake3::Hash) -> CRDTResult<()> {
        // Update peer state tracking
        self.peer_states.insert(replica_id, (vector_clock.clone(), content_hash));
        
        // Check if we need to sync
        let local_hash = self.local_state.content_hash();
        let needs_sync = content_hash != local_hash &&
            !self.local_vector_clock.dominates(&vector_clock) &&
            !vector_clock.dominates(&self.local_vector_clock);
        
        if needs_sync {
            // Request full state if vectors are concurrent
            let request = SyncMessage::StateRequest {
                replica_id: self.replica_id,
                requested_vector_clock: Some(vector_clock),
            };
            self.send_message(replica_id, request)?;
        }
        
        Ok(())
    }
    
    fn handle_full_state(&mut self, replica_id: ReplicaId, state: T, vector_clock: VectorClock) -> CRDTResult<()> {
        // Merge the received state
        let old_state = self.local_state.clone();
        self.local_state.merge(&state);
        
        // Update vector clock
        self.local_vector_clock.update(&vector_clock);
        self.local_vector_clock.increment(self.replica_id);
        
        // Update peer tracking
        let state_hash = state.content_hash();
        self.peer_states.insert(replica_id, (vector_clock.clone(), state_hash));
        
        // Send acknowledgment
        let ack = SyncMessage::Acknowledgment {
            replica_id: self.replica_id,
            ack_vector_clock: self.local_vector_clock.clone(),
        };
        self.send_message(replica_id, ack)?;
        
        Ok(())
    }
    
    fn handle_delta_update(&mut self, replica_id: ReplicaId, delta: T) -> CRDTResult<()> {
        // Apply delta (treating it as full state merge for now)
        self.local_state.merge(&delta);
        self.local_vector_clock.increment(self.replica_id);
        
        Ok(())
    }
    
    fn handle_state_request(&mut self, replica_id: ReplicaId, _requested_vector_clock: Option<VectorClock>) -> CRDTResult<()> {
        // Send current full state
        let response = SyncMessage::FullState {
            replica_id: self.replica_id,
            state: self.local_state.clone(),
            vector_clock: self.local_vector_clock.clone(),
        };
        
        self.send_message(replica_id, response)?;
        Ok(())
    }
    
    fn handle_acknowledgment(&mut self, replica_id: ReplicaId, ack_vector_clock: VectorClock) -> CRDTResult<()> {
        // Update our knowledge of peer's state
        if let Some((peer_vc, _)) = self.peer_states.get_mut(&replica_id) {
            peer_vc.update(&ack_vector_clock);
        }
        Ok(())
    }
    
    fn send_message(&self, to: ReplicaId, message: SyncMessage<T>) -> CRDTResult<()> {
        if let Some(sender) = &self.message_sender {
            sender.send((to, message))
                .map_err(|_| CRDTError::SyncFailed {
                    reason: "Message channel closed".to_string(),
                })?;
        }
        Ok(())
    }
}

/// Synchronization statistics
#[derive(Debug, Clone)]
pub struct SyncStatistics {
    pub local_replica_id: ReplicaId,
    pub peer_count: usize,
    pub local_vector_clock: VectorClock,
}