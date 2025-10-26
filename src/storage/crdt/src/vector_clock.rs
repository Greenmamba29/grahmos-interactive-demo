use crate::{ReplicaId, CRDT, CRDTError, CRDTResult};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::cmp::Ordering;

/// Vector Clock for tracking causality in distributed systems
/// 
/// A vector clock is a data structure used to establish a partial order of events
/// in a distributed system and detect causality violations. Each replica maintains
/// a vector with timestamps for all known replicas.
/// 
/// Properties:
/// - Monotonic: timestamps only increase
/// - Causal: if event a causes event b, then VC(a) < VC(b)
/// - Concurrent: if VC(a) || VC(b), then events a and b are concurrent
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VectorClock {
    /// Map from replica ID to logical timestamp
    clocks: BTreeMap<ReplicaId, u64>,
}

impl VectorClock {
    /// Create a new empty vector clock
    pub fn new() -> Self {
        Self {
            clocks: BTreeMap::new(),
        }
    }
    
    /// Create vector clock with initial replica
    pub fn with_replica(replica_id: ReplicaId) -> Self {
        let mut vc = Self::new();
        vc.clocks.insert(replica_id, 0);
        vc
    }
    
    /// Get the timestamp for a specific replica
    pub fn get(&self, replica_id: &ReplicaId) -> u64 {
        self.clocks.get(replica_id).copied().unwrap_or(0)
    }
    
    /// Set the timestamp for a specific replica
    pub fn set(&mut self, replica_id: ReplicaId, timestamp: u64) {
        let current = self.clocks.entry(replica_id).or_insert(0);
        *current = (*current).max(timestamp);
    }
    
    /// Increment the timestamp for a specific replica
    pub fn increment(&mut self, replica_id: ReplicaId) -> u64 {
        let timestamp = self.clocks.entry(replica_id).or_insert(0);
        *timestamp += 1;
        *timestamp
    }
    
    /// Update this vector clock with another (take maximum of each component)
    pub fn update(&mut self, other: &VectorClock) {
        for (&replica_id, &timestamp) in &other.clocks {
            let current = self.clocks.entry(replica_id).or_insert(0);
            *current = (*current).max(timestamp);
        }
    }
    
    /// Merge two vector clocks (same as update but immutable)
    pub fn merged(&self, other: &VectorClock) -> VectorClock {
        let mut result = self.clone();
        result.update(other);
        result
    }
    
    /// Check if this vector clock happens-before another
    /// Returns true if self < other (strict partial order)
    pub fn happens_before(&self, other: &VectorClock) -> bool {
        let mut self_le_other = true;
        let mut exists_self_lt_other = false;
        
        // Collect all replica IDs from both clocks
        let all_replicas: std::collections::BTreeSet<ReplicaId> = self.clocks.keys()
            .chain(other.clocks.keys())
            .copied()
            .collect();
        
        for replica_id in all_replicas {
            let self_time = self.get(&replica_id);
            let other_time = other.get(&replica_id);
            
            if self_time > other_time {
                self_le_other = false;
                break;
            } else if self_time < other_time {
                exists_self_lt_other = true;
            }
        }
        
        self_le_other && exists_self_lt_other
    }
    
    /// Check if two vector clocks are concurrent (neither happens-before the other)
    pub fn concurrent_with(&self, other: &VectorClock) -> bool {
        !self.happens_before(other) && !other.happens_before(self) && self != other
    }
    
    /// Check if this vector clock dominates another (self >= other)
    pub fn dominates(&self, other: &VectorClock) -> bool {
        other.happens_before(self) || self == other
    }
    
    /// Get all replica IDs known to this vector clock
    pub fn replica_ids(&self) -> impl Iterator<Item = &ReplicaId> {
        self.clocks.keys()
    }
    
    /// Get the number of known replicas
    pub fn replica_count(&self) -> usize {
        self.clocks.len()
    }
    
    /// Check if this vector clock is empty
    pub fn is_empty(&self) -> bool {
        self.clocks.is_empty()
    }
    
    /// Get the maximum timestamp across all replicas
    pub fn max_timestamp(&self) -> u64 {
        self.clocks.values().copied().max().unwrap_or(0)
    }
    
    /// Get the sum of all timestamps
    pub fn sum(&self) -> u64 {
        self.clocks.values().sum()
    }
    
    /// Create a snapshot of timestamps for all known replicas
    pub fn snapshot(&self) -> BTreeMap<ReplicaId, u64> {
        self.clocks.clone()
    }
    
    /// Compact the vector clock by removing replicas with timestamp 0
    pub fn compact(&mut self) {
        self.clocks.retain(|_, &mut timestamp| timestamp > 0);
    }
    
    /// Get the size of the vector clock in bytes (approximate)
    pub fn size_bytes(&self) -> usize {
        std::mem::size_of::<Self>() + 
        self.clocks.len() * (std::mem::size_of::<ReplicaId>() + std::mem::size_of::<u64>())
    }
}

impl Default for VectorClock {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for VectorClock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "VC{{")?;
        let mut first = true;
        for (replica_id, timestamp) in &self.clocks {
            if !first {
                write!(f, ", ")?;
            }
            write!(f, "{}:{}", replica_id, timestamp)?;
            first = false;
        }
        write!(f, "}}")
    }
}

impl PartialOrd for VectorClock {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.happens_before(other) {
            Some(Ordering::Less)
        } else if other.happens_before(self) {
            Some(Ordering::Greater)
        } else if self == other {
            Some(Ordering::Equal)
        } else {
            None // Concurrent
        }
    }
}

impl CRDT for VectorClock {
    fn merge(&mut self, other: &Self) {
        self.update(other);
    }
    
    fn is_subset_of(&self, other: &Self) -> bool {
        // A vector clock is subset of another if all its timestamps are <= the other's
        for (&replica_id, &timestamp) in &self.clocks {
            if timestamp > other.get(&replica_id) {
                return false;
            }
        }
        true
    }
}

/// Versioned value using vector clock for conflict resolution
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionedValue<T> {
    /// The actual value
    pub value: T,
    /// Vector clock timestamp when this value was created
    pub version: VectorClock,
    /// Replica that created this value
    pub author: ReplicaId,
}

impl<T> VersionedValue<T> {
    /// Create a new versioned value
    pub fn new(value: T, author: ReplicaId) -> Self {
        let mut version = VectorClock::new();
        version.increment(author);
        
        Self {
            value,
            version,
            author,
        }
    }
    
    /// Create versioned value with specific vector clock
    pub fn with_version(value: T, version: VectorClock, author: ReplicaId) -> Self {
        Self {
            value,
            version,
            author,
        }
    }
    
    /// Check if this value happens-before another
    pub fn happens_before(&self, other: &VersionedValue<T>) -> bool {
        self.version.happens_before(&other.version)
    }
    
    /// Check if this value is concurrent with another
    pub fn concurrent_with(&self, other: &VersionedValue<T>) -> bool {
        self.version.concurrent_with(&other.version)
    }
}

/// Multi-value register that keeps all concurrent values
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MVRegister<T> {
    /// Set of concurrent versioned values
    values: Vec<VersionedValue<T>>,
}

impl<T: Clone + PartialEq> MVRegister<T> {
    /// Create a new empty multi-value register
    pub fn new() -> Self {
        Self {
            values: Vec::new(),
        }
    }
    
    /// Set a new value
    pub fn set(&mut self, value: T, author: ReplicaId, mut version: VectorClock) {
        version.increment(author);
        let versioned_value = VersionedValue::with_version(value, version, author);
        
        // Remove any values that happen-before this new value
        self.values.retain(|v| !v.happens_before(&versioned_value));
        
        // Add the new value if it's not dominated by existing values
        if !self.values.iter().any(|v| versioned_value.happens_before(v)) {
            self.values.push(versioned_value);
        }
    }
    
    /// Get all current values (concurrent values)
    pub fn values(&self) -> &[VersionedValue<T>] {
        &self.values
    }
    
    /// Get a single value if there's no conflict, or the first value if there are conflicts
    pub fn value(&self) -> Option<&T> {
        self.values.first().map(|v| &v.value)
    }
    
    /// Check if there are conflicting concurrent values
    pub fn has_conflicts(&self) -> bool {
        self.values.len() > 1
    }
    
    /// Get the number of concurrent values
    pub fn conflict_count(&self) -> usize {
        self.values.len()
    }
    
    /// Merge with another multi-value register
    pub fn merge(&mut self, other: &MVRegister<T>) {
        for other_value in &other.values {
            // Remove values that happen-before this other value
            self.values.retain(|v| !v.happens_before(other_value));
            
            // Add the other value if it's not dominated by existing values
            if !self.values.iter().any(|v| other_value.happens_before(v)) {
                self.values.push(other_value.clone());
            }
        }
    }
    
    /// Get the combined vector clock representing the state
    pub fn vector_clock(&self) -> VectorClock {
        let mut combined = VectorClock::new();
        for versioned_value in &self.values {
            combined.update(&versioned_value.version);
        }
        combined
    }
}

impl<T: Clone + PartialEq> Default for MVRegister<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone + PartialEq + std::fmt::Debug> CRDT for MVRegister<T> 
where 
    T: serde::Serialize + for<'de> serde::Deserialize<'de>
{
    fn merge(&mut self, other: &Self) {
        MVRegister::merge(self, other);
    }
    
    fn is_subset_of(&self, other: &Self) -> bool {
        // This register is subset if all its values are dominated by values in other
        self.values.iter().all(|self_val| {
            other.values.iter().any(|other_val| {
                self_val.happens_before(other_val) || self_val == other_val
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_clock_creation() {
        let vc = VectorClock::new();
        assert!(vc.is_empty());
        assert_eq!(vc.replica_count(), 0);
        
        let replica = ReplicaId::new();
        let vc = VectorClock::with_replica(replica);
        assert!(!vc.is_empty());
        assert_eq!(vc.replica_count(), 1);
        assert_eq!(vc.get(&replica), 0);
    }

    #[test]
    fn test_vector_clock_operations() {
        let replica1 = ReplicaId::new();
        let replica2 = ReplicaId::new();
        
        let mut vc = VectorClock::new();
        
        // Test increment
        assert_eq!(vc.increment(replica1), 1);
        assert_eq!(vc.increment(replica1), 2);
        assert_eq!(vc.increment(replica2), 1);
        
        // Test get
        assert_eq!(vc.get(&replica1), 2);
        assert_eq!(vc.get(&replica2), 1);
        
        // Test set
        vc.set(replica1, 5);
        assert_eq!(vc.get(&replica1), 5); // Should take max
    }

    #[test]
    fn test_happens_before() {
        let replica1 = ReplicaId::new();
        let replica2 = ReplicaId::new();
        
        let mut vc1 = VectorClock::new();
        vc1.increment(replica1); // {r1: 1}
        
        let mut vc2 = VectorClock::new();
        vc2.increment(replica1); // {r1: 1}
        vc2.increment(replica2); // {r1: 1, r2: 1}
        
        // vc1 happens-before vc2
        assert!(vc1.happens_before(&vc2));
        assert!(!vc2.happens_before(&vc1));
        assert!(!vc1.concurrent_with(&vc2));
    }

    #[test]
    fn test_concurrent() {
        let replica1 = ReplicaId::new();
        let replica2 = ReplicaId::new();
        
        let mut vc1 = VectorClock::new();
        vc1.increment(replica1); // {r1: 1}
        
        let mut vc2 = VectorClock::new();
        vc2.increment(replica2); // {r2: 1}
        
        // vc1 and vc2 are concurrent
        assert!(vc1.concurrent_with(&vc2));
        assert!(vc2.concurrent_with(&vc1));
        assert!(!vc1.happens_before(&vc2));
        assert!(!vc2.happens_before(&vc1));
    }

    #[test]
    fn test_vector_clock_merge() {
        let replica1 = ReplicaId::new();
        let replica2 = ReplicaId::new();
        let replica3 = ReplicaId::new();
        
        let mut vc1 = VectorClock::new();
        vc1.set(replica1, 3);
        vc1.set(replica2, 1);
        
        let mut vc2 = VectorClock::new();
        vc2.set(replica1, 2);
        vc2.set(replica2, 4);
        vc2.set(replica3, 1);
        
        vc1.update(&vc2);
        
        assert_eq!(vc1.get(&replica1), 3); // max(3, 2)
        assert_eq!(vc1.get(&replica2), 4); // max(1, 4)
        assert_eq!(vc1.get(&replica3), 1); // max(0, 1)
    }

    #[test]
    fn test_versioned_value() {
        let replica1 = ReplicaId::new();
        let replica2 = ReplicaId::new();
        
        let value1 = VersionedValue::new("hello", replica1);
        assert_eq!(value1.value, "hello");
        assert_eq!(value1.author, replica1);
        assert_eq!(value1.version.get(&replica1), 1);
        
        let mut vc2 = VectorClock::new();
        vc2.increment(replica1);
        vc2.increment(replica2);
        let value2 = VersionedValue::with_version("world", vc2, replica2);
        
        assert!(value1.happens_before(&value2));
        assert!(!value2.happens_before(&value1));
    }

    #[test]
    fn test_mv_register() {
        let replica1 = ReplicaId::new();
        let replica2 = ReplicaId::new();
        
        let mut register = MVRegister::new();
        
        // Set initial value
        register.set("hello", replica1, VectorClock::new());
        assert_eq!(register.values().len(), 1);
        assert_eq!(register.value(), Some(&"hello"));
        assert!(!register.has_conflicts());
        
        // Set concurrent value
        register.set("world", replica2, VectorClock::new());
        assert_eq!(register.values().len(), 2);
        assert!(register.has_conflicts());
        
        // Update with causally later value
        let mut later_vc = register.vector_clock();
        register.set("final", replica1, later_vc);
        assert_eq!(register.values().len(), 1);
        assert_eq!(register.value(), Some(&"final"));
        assert!(!register.has_conflicts());
    }

    #[test]
    fn test_crdt_laws_vector_clock() {
        let replica1 = ReplicaId::new();
        let replica2 = ReplicaId::new();
        
        let mut vc1 = VectorClock::new();
        vc1.increment(replica1);
        
        let mut vc2 = VectorClock::new();
        vc2.increment(replica2);
        
        let mut vc3 = VectorClock::new();
        vc3.increment(replica1);
        vc3.increment(replica2);
        
        // Test CRDT laws using utility functions
        use crate::utils::*;
        
        assert!(test_idempotency(&vc1));
        assert!(test_commutativity(&vc1, &vc2));
        assert!(test_associativity(&vc1, &vc2, &vc3));
        
        assert!(verify_crdt_laws(&[vc1, vc2, vc3]));
    }
}