use blake3::Hash;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, BTreeMap};
use std::fmt::Debug;
use std::hash::Hash as StdHash;
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

pub mod vector_clock;
pub mod grow_only_counter;
pub mod pn_counter;
pub mod grow_only_set;
pub mod two_phase_set;
pub mod lww_register;
pub mod or_set;
pub mod rga;
pub mod sync;

pub use vector_clock::*;
pub use grow_only_counter::*;
pub use pn_counter::*;
pub use grow_only_set::*;
pub use two_phase_set::*;
pub use lww_register::*;
pub use or_set::*;
pub use rga::*;
pub use sync::*;

/// Conflict-Free Replicated Data Type (CRDT) Foundation for PRISM
/// 
/// Implements multiple CRDT types for eventual consistency in distributed systems:
/// - State-based CRDTs (CvRDTs): Merge states to achieve consistency
/// - Operation-based CRDTs (CmRDTs): Apply operations in any order
/// - Vector clocks for causality tracking
/// - Automatic conflict resolution with deterministic outcomes
/// - Network-efficient delta synchronization
/// 
/// Key properties guaranteed:
/// - Associativity: (a ⊔ b) ⊔ c = a ⊔ (b ⊔ c)
/// - Commutativity: a ⊔ b = b ⊔ a  
/// - Idempotency: a ⊔ a = a
/// - Convergence: All replicas eventually converge to same state

/// Unique identifier for CRDT replicas
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReplicaId(pub Uuid);

impl ReplicaId {
    /// Generate a new random replica ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
    
    /// Create from existing UUID
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
    
    /// Get the underlying UUID
    pub fn uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for ReplicaId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ReplicaId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Timestamp for CRDT operations using Lamport timestamps
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LogicalTimestamp {
    /// Lamport timestamp counter
    pub counter: u64,
    /// Replica that generated this timestamp
    pub replica_id: ReplicaId,
}

impl LogicalTimestamp {
    /// Create a new timestamp
    pub fn new(counter: u64, replica_id: ReplicaId) -> Self {
        Self { counter, replica_id }
    }
    
    /// Increment timestamp for local operation
    pub fn tick(&mut self) {
        self.counter += 1;
    }
    
    /// Update timestamp on receiving remote operation
    pub fn update(&mut self, remote: &LogicalTimestamp) {
        self.counter = self.counter.max(remote.counter) + 1;
    }
}

/// Physical timestamp for wall-clock ordering
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PhysicalTimestamp(pub DateTime<Utc>);

impl PhysicalTimestamp {
    /// Create timestamp for current time
    pub fn now() -> Self {
        Self(Utc::now())
    }
    
    /// Create from specific datetime
    pub fn from_datetime(dt: DateTime<Utc>) -> Self {
        Self(dt)
    }
}

/// Hybrid logical-physical timestamp for better ordering
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct HybridTimestamp {
    /// Physical component (wall clock)
    pub physical: PhysicalTimestamp,
    /// Logical component (Lamport counter)
    pub logical: u64,
    /// Replica that created this timestamp
    pub replica_id: ReplicaId,
}

impl HybridTimestamp {
    /// Create new hybrid timestamp
    pub fn new(replica_id: ReplicaId) -> Self {
        Self {
            physical: PhysicalTimestamp::now(),
            logical: 0,
            replica_id,
        }
    }
    
    /// Update timestamp for local operation
    pub fn tick(&mut self) {
        let now = PhysicalTimestamp::now();
        if now > self.physical {
            self.physical = now;
            self.logical = 0;
        } else {
            self.logical += 1;
        }
    }
    
    /// Update timestamp when receiving remote operation
    pub fn update(&mut self, remote: &HybridTimestamp) {
        let now = PhysicalTimestamp::now();
        let max_physical = self.physical.max(remote.physical).max(now);
        
        if max_physical > self.physical && max_physical > remote.physical {
            // Physical time advanced
            self.physical = max_physical;
            self.logical = 0;
        } else if max_physical == self.physical {
            // Same physical time, increment logical
            self.logical = self.logical.max(remote.logical) + 1;
        } else {
            // Remote physical time is latest
            self.physical = max_physical;
            self.logical = remote.logical + 1;
        }
    }
}

/// Core CRDT trait defining merge semantics
pub trait CRDT: Clone + Debug + Serialize + for<'de> Deserialize<'de> {
    /// Merge this CRDT with another instance
    /// Must satisfy: associative, commutative, idempotent
    fn merge(&mut self, other: &Self);
    
    /// Create a new CRDT by merging two instances
    fn merged(mut self, other: &Self) -> Self {
        self.merge(other);
        self
    }
    
    /// Check if this CRDT is a subset of (or equal to) another
    /// Used for determining if synchronization is needed
    fn is_subset_of(&self, _other: &Self) -> bool {
        false // Default conservative implementation
    }
    
    /// Get content hash for deduplication and integrity
    fn content_hash(&self) -> Hash {
        let serialized = bincode::serialize(self)
            .unwrap_or_default();
        blake3::hash(&serialized)
    }
    
    /// Get human-readable debug representation
    fn debug_info(&self) -> String {
        format!("{:?}", self)
    }
}

/// State-based CRDT (CvRDT) with explicit state merging
pub trait StateCRDT: CRDT {
    /// Get the current state for synchronization
    fn state(&self) -> Self {
        self.clone()
    }
    
    /// Merge with state from remote replica
    fn merge_state(&mut self, state: &Self) {
        self.merge(state);
    }
}

/// Operation-based CRDT (CmRDT) with operation application
pub trait OpCRDT<Op>: CRDT 
where 
    Op: Clone + Debug + Serialize + for<'de> Deserialize<'de>,
{
    type Error;
    
    /// Apply an operation to this CRDT
    fn apply(&mut self, op: &Op) -> Result<(), Self::Error>;
    
    /// Generate operation for a mutation
    fn generate_op(&self, mutation: &dyn std::any::Any) -> Result<Op, Self::Error>;
    
    /// Check if operation can be applied (precondition)
    fn can_apply(&self, op: &Op) -> bool;
    
    /// Apply operation and return success status
    fn try_apply(&mut self, op: &Op) -> Result<bool, Self::Error> {
        if self.can_apply(op) {
            self.apply(op)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

/// Delta CRDT supporting efficient incremental synchronization
pub trait DeltaCRDT: CRDT {
    type Delta: Clone + Debug + Serialize + for<'de> Deserialize<'de>;
    
    /// Apply a delta to this CRDT
    fn apply_delta(&mut self, delta: &Self::Delta);
    
    /// Generate delta since a given state
    fn delta_since(&self, base: &Self) -> Option<Self::Delta>;
    
    /// Merge deltas into a single delta
    fn merge_deltas(deltas: &[Self::Delta]) -> Self::Delta;
}

/// Metadata about CRDT operations and state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CRDTMetadata {
    /// When this CRDT state was created
    pub created_at: DateTime<Utc>,
    /// Last modification timestamp
    pub modified_at: DateTime<Utc>,
    /// Replica that last modified this CRDT
    pub modified_by: ReplicaId,
    /// Version vector for causality tracking
    pub version: VectorClock,
    /// Content hash for integrity verification
    pub content_hash: Option<Hash>,
    /// Size metrics
    pub size_bytes: usize,
    /// Operation count since creation
    pub operation_count: u64,
}

impl CRDTMetadata {
    /// Create new metadata for a CRDT
    pub fn new(replica_id: ReplicaId) -> Self {
        let now = Utc::now();
        Self {
            created_at: now,
            modified_at: now,
            modified_by: replica_id,
            version: VectorClock::new(),
            content_hash: None,
            size_bytes: 0,
            operation_count: 0,
        }
    }
    
    /// Update metadata after an operation
    pub fn update_after_operation(&mut self, replica_id: ReplicaId, size_bytes: usize) {
        self.modified_at = Utc::now();
        self.modified_by = replica_id;
        self.version.increment(replica_id);
        self.size_bytes = size_bytes;
        self.operation_count += 1;
    }
    
    /// Merge metadata from remote replica
    pub fn merge(&mut self, other: &CRDTMetadata) {
        // Take latest modification time
        if other.modified_at > self.modified_at {
            self.modified_at = other.modified_at;
            self.modified_by = other.modified_by;
        }
        
        // Merge version vectors
        self.version.merge(&other.version);
        
        // Update operation count
        self.operation_count = self.operation_count.max(other.operation_count);
    }
}

/// Container for CRDT with associated metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CRDTContainer<T: CRDT> {
    /// The CRDT data structure
    pub crdt: T,
    /// Associated metadata
    pub metadata: CRDTMetadata,
}

impl<T: CRDT> CRDTContainer<T> {
    /// Create new CRDT container
    pub fn new(crdt: T, replica_id: ReplicaId) -> Self {
        let mut metadata = CRDTMetadata::new(replica_id);
        metadata.content_hash = Some(crdt.content_hash());
        metadata.size_bytes = std::mem::size_of_val(&crdt);
        
        Self { crdt, metadata }
    }
    
    /// Update after local operation
    pub fn after_operation(&mut self, replica_id: ReplicaId) {
        self.metadata.content_hash = Some(self.crdt.content_hash());
        self.metadata.size_bytes = std::mem::size_of_val(&self.crdt);
        self.metadata.update_after_operation(replica_id, self.metadata.size_bytes);
    }
    
    /// Merge with another CRDT container
    pub fn merge(&mut self, other: &CRDTContainer<T>) {
        self.crdt.merge(&other.crdt);
        self.metadata.merge(&other.metadata);
        self.metadata.content_hash = Some(self.crdt.content_hash());
    }
}

/// Errors that can occur in CRDT operations
#[derive(thiserror::Error, Debug)]
pub enum CRDTError {
    #[error("Invalid operation: {reason}")]
    InvalidOperation { reason: String },
    
    #[error("Precondition failed: {condition}")]
    PreconditionFailed { condition: String },
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),
    
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    
    #[error("Timestamp ordering violation")]
    TimestampViolation,
    
    #[error("Unknown replica: {replica_id}")]
    UnknownReplica { replica_id: ReplicaId },
    
    #[error("Network synchronization failed: {reason}")]
    SyncFailed { reason: String },
    
    #[error("Content hash mismatch")]
    HashMismatch,
    
    #[error("CRDT invariant violated: {invariant}")]
    InvariantViolation { invariant: String },
}

pub type CRDTResult<T> = Result<T, CRDTError>;

/// CRDT statistics and performance metrics
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct CRDTStatistics {
    /// Total operations applied
    pub total_operations: u64,
    /// Merge operations performed
    pub merge_operations: u64,
    /// Synchronization events
    pub sync_events: u64,
    /// Conflicts resolved
    pub conflicts_resolved: u64,
    /// Current state size in bytes
    pub current_size_bytes: usize,
    /// Network messages sent
    pub messages_sent: u64,
    /// Network messages received
    pub messages_received: u64,
    /// Time spent in merge operations
    pub merge_time_ms: u64,
    /// Time spent in synchronization
    pub sync_time_ms: u64,
}

impl CRDTStatistics {
    /// Update statistics after a merge operation
    pub fn record_merge(&mut self, duration_ms: u64) {
        self.merge_operations += 1;
        self.merge_time_ms += duration_ms;
    }
    
    /// Update statistics after a sync event
    pub fn record_sync(&mut self, duration_ms: u64) {
        self.sync_events += 1;
        self.sync_time_ms += duration_ms;
    }
    
    /// Calculate average merge time
    pub fn avg_merge_time_ms(&self) -> f64 {
        if self.merge_operations == 0 {
            0.0
        } else {
            self.merge_time_ms as f64 / self.merge_operations as f64
        }
    }
    
    /// Calculate conflict resolution rate
    pub fn conflict_resolution_rate(&self) -> f64 {
        if self.merge_operations == 0 {
            0.0
        } else {
            self.conflicts_resolved as f64 / self.merge_operations as f64
        }
    }
}

/// CRDT manager for handling multiple CRDT instances
#[derive(Debug)]
pub struct CRDTManager {
    /// Local replica identifier
    pub replica_id: ReplicaId,
    /// Statistics tracking
    pub stats: CRDTStatistics,
    /// Registered CRDT instances (type-erased)
    instances: HashMap<String, Box<dyn std::any::Any + Send + Sync>>,
}

impl CRDTManager {
    /// Create new CRDT manager
    pub fn new(replica_id: ReplicaId) -> Self {
        Self {
            replica_id,
            stats: CRDTStatistics::default(),
            instances: HashMap::new(),
        }
    }
    
    /// Register a CRDT instance
    pub fn register<T: CRDT + Send + Sync + 'static>(&mut self, name: String, crdt: T) {
        let container = CRDTContainer::new(crdt, self.replica_id);
        self.instances.insert(name, Box::new(container));
    }
    
    /// Get a CRDT instance by name
    pub fn get<T: CRDT + Send + Sync + 'static>(&self, name: &str) -> Option<&CRDTContainer<T>> {
        self.instances.get(name)?.downcast_ref()
    }
    
    /// Get a mutable CRDT instance by name
    pub fn get_mut<T: CRDT + Send + Sync + 'static>(&mut self, name: &str) -> Option<&mut CRDTContainer<T>> {
        self.instances.get_mut(name)?.downcast_mut()
    }
    
    /// Merge CRDT from remote replica
    pub fn merge_remote<T: CRDT + Send + Sync + 'static>(
        &mut self, 
        name: &str, 
        remote: &CRDTContainer<T>
    ) -> CRDTResult<()> {
        let start = std::time::Instant::now();
        
        if let Some(local) = self.get_mut::<T>(name) {
            local.merge(remote);
            self.stats.record_merge(start.elapsed().as_millis() as u64);
            Ok(())
        } else {
            Err(CRDTError::InvalidOperation {
                reason: format!("CRDT '{}' not found", name),
            })
        }
    }
    
    /// Get current statistics
    pub fn statistics(&self) -> &CRDTStatistics {
        &self.stats
    }
}

/// Utility functions for CRDT operations
pub mod utils {
    use super::*;
    
    /// Compare two CRDTs for equality
    pub fn crdts_equal<T: CRDT>(a: &T, b: &T) -> bool {
        a.content_hash() == b.content_hash()
    }
    
    /// Check if CRDT merge is idempotent
    pub fn test_idempotency<T: CRDT>(crdt: &T) -> bool {
        let mut merged = crdt.clone();
        merged.merge(crdt);
        crdts_equal(crdt, &merged)
    }
    
    /// Check if CRDT merge is commutative
    pub fn test_commutativity<T: CRDT>(a: &T, b: &T) -> bool {
        let mut ab = a.clone();
        ab.merge(b);
        
        let mut ba = b.clone();
        ba.merge(a);
        
        crdts_equal(&ab, &ba)
    }
    
    /// Check if CRDT merge is associative
    pub fn test_associativity<T: CRDT>(a: &T, b: &T, c: &T) -> bool {
        // Test (a ⊔ b) ⊔ c = a ⊔ (b ⊔ c)
        let mut ab_c = a.clone();
        ab_c.merge(b);
        ab_c.merge(c);
        
        let mut bc = b.clone();
        bc.merge(c);
        let mut a_bc = a.clone();
        a_bc.merge(&bc);
        
        crdts_equal(&ab_c, &a_bc)
    }
    
    /// Verify all CRDT laws for a given instance
    pub fn verify_crdt_laws<T: CRDT>(crdts: &[T]) -> bool {
        if crdts.len() < 3 {
            return true; // Need at least 3 instances to test associativity
        }
        
        // Test idempotency
        for crdt in crdts {
            if !test_idempotency(crdt) {
                return false;
            }
        }
        
        // Test commutativity for all pairs
        for i in 0..crdts.len() {
            for j in i + 1..crdts.len() {
                if !test_commutativity(&crdts[i], &crdts[j]) {
                    return false;
                }
            }
        }
        
        // Test associativity for triplets
        for i in 0..crdts.len() {
            for j in 0..crdts.len() {
                for k in 0..crdts.len() {
                    if i != j && j != k && i != k {
                        if !test_associativity(&crdts[i], &crdts[j], &crdts[k]) {
                            return false;
                        }
                    }
                }
            }
        }
        
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_replica_id() {
        let id1 = ReplicaId::new();
        let id2 = ReplicaId::new();
        
        assert_ne!(id1, id2);
        assert_eq!(id1, ReplicaId::from_uuid(id1.uuid()));
    }

    #[test]
    fn test_logical_timestamp() {
        let replica1 = ReplicaId::new();
        let replica2 = ReplicaId::new();
        
        let mut ts1 = LogicalTimestamp::new(0, replica1);
        let mut ts2 = LogicalTimestamp::new(0, replica2);
        
        ts1.tick(); // ts1 = 1
        assert_eq!(ts1.counter, 1);
        
        ts2.update(&ts1); // ts2 = max(0, 1) + 1 = 2
        assert_eq!(ts2.counter, 2);
        
        ts1.update(&ts2); // ts1 = max(1, 2) + 1 = 3
        assert_eq!(ts1.counter, 3);
    }

    #[test]
    fn test_hybrid_timestamp() {
        let replica1 = ReplicaId::new();
        let replica2 = ReplicaId::new();
        
        let mut ts1 = HybridTimestamp::new(replica1);
        let mut ts2 = HybridTimestamp::new(replica2);
        
        // Simulate rapid operations within same millisecond
        ts1.tick();
        ts1.tick();
        assert_eq!(ts1.logical, 2);
        
        ts2.update(&ts1);
        assert!(ts2.logical > ts1.logical);
    }

    #[test]
    fn test_crdt_metadata() {
        let replica = ReplicaId::new();
        let mut metadata = CRDTMetadata::new(replica);
        
        // Simulate operation
        std::thread::sleep(Duration::from_millis(1));
        metadata.update_after_operation(replica, 100);
        
        assert!(metadata.modified_at > metadata.created_at);
        assert_eq!(metadata.size_bytes, 100);
        assert_eq!(metadata.operation_count, 1);
    }

    #[test]
    fn test_crdt_statistics() {
        let mut stats = CRDTStatistics::default();
        
        stats.record_merge(10);
        stats.record_merge(20);
        stats.record_sync(15);
        
        assert_eq!(stats.merge_operations, 2);
        assert_eq!(stats.merge_time_ms, 30);
        assert_eq!(stats.avg_merge_time_ms(), 15.0);
    }
}