use crate::{ReplicaId, CRDT, StateCRDT};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Grow-Only Counter (G-Counter) CRDT
/// 
/// A counter that can only be incremented, never decremented.
/// Each replica maintains its own counter, and the global value
/// is the sum of all replica counters.
/// 
/// Properties:
/// - Monotonic: value can only increase
/// - Eventually consistent: all replicas converge to same sum
/// - Partition tolerant: works during network splits
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrowOnlyCounter {
    /// Per-replica counter values
    counters: BTreeMap<ReplicaId, u64>,
}

impl GrowOnlyCounter {
    /// Create a new G-Counter
    pub fn new() -> Self {
        Self {
            counters: BTreeMap::new(),
        }
    }
    
    /// Create G-Counter with initial replica
    pub fn with_replica(replica_id: ReplicaId) -> Self {
        let mut counter = Self::new();
        counter.counters.insert(replica_id, 0);
        counter
    }
    
    /// Increment the counter for a specific replica
    pub fn increment(&mut self, replica_id: ReplicaId, amount: u64) -> u64 {
        let counter = self.counters.entry(replica_id).or_insert(0);
        *counter += amount;
        *counter
    }
    
    /// Get the current value (sum of all replica counters)
    pub fn value(&self) -> u64 {
        self.counters.values().sum()
    }
    
    /// Get the counter value for a specific replica
    pub fn get_replica_count(&self, replica_id: &ReplicaId) -> u64 {
        self.counters.get(replica_id).copied().unwrap_or(0)
    }
    
    /// Get all known replicas
    pub fn replicas(&self) -> impl Iterator<Item = &ReplicaId> {
        self.counters.keys()
    }
    
    /// Get number of known replicas
    pub fn replica_count(&self) -> usize {
        self.counters.len()
    }
    
    /// Check if counter is empty
    pub fn is_empty(&self) -> bool {
        self.counters.is_empty() || self.value() == 0
    }
    
    /// Get snapshot of all replica counters
    pub fn snapshot(&self) -> BTreeMap<ReplicaId, u64> {
        self.counters.clone()
    }
}

impl Default for GrowOnlyCounter {
    fn default() -> Self {
        Self::new()
    }
}

impl CRDT for GrowOnlyCounter {
    fn merge(&mut self, other: &Self) {
        for (&replica_id, &count) in &other.counters {
            let current = self.counters.entry(replica_id).or_insert(0);
            *current = (*current).max(count);
        }
    }
    
    fn is_subset_of(&self, other: &Self) -> bool {
        for (&replica_id, &count) in &self.counters {
            if count > other.get_replica_count(&replica_id) {
                return false;
            }
        }
        true
    }
}

impl StateCRDT for GrowOnlyCounter {}

impl std::fmt::Display for GrowOnlyCounter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "GCounter({})", self.value())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gcounter_creation() {
        let counter = GrowOnlyCounter::new();
        assert_eq!(counter.value(), 0);
        assert!(counter.is_empty());
        
        let replica = ReplicaId::new();
        let counter = GrowOnlyCounter::with_replica(replica);
        assert_eq!(counter.get_replica_count(&replica), 0);
    }

    #[test]
    fn test_gcounter_increment() {
        let replica1 = ReplicaId::new();
        let replica2 = ReplicaId::new();
        
        let mut counter = GrowOnlyCounter::new();
        
        assert_eq!(counter.increment(replica1, 5), 5);
        assert_eq!(counter.increment(replica2, 3), 3);
        assert_eq!(counter.increment(replica1, 2), 7);
        
        assert_eq!(counter.value(), 10); // 7 + 3
        assert_eq!(counter.get_replica_count(&replica1), 7);
        assert_eq!(counter.get_replica_count(&replica2), 3);
    }

    #[test]
    fn test_gcounter_merge() {
        let replica1 = ReplicaId::new();
        let replica2 = ReplicaId::new();
        
        let mut counter1 = GrowOnlyCounter::new();
        counter1.increment(replica1, 5);
        counter1.increment(replica2, 2);
        
        let mut counter2 = GrowOnlyCounter::new();
        counter2.increment(replica1, 3);
        counter2.increment(replica2, 4);
        
        counter1.merge(&counter2);
        
        assert_eq!(counter1.get_replica_count(&replica1), 5); // max(5, 3)
        assert_eq!(counter1.get_replica_count(&replica2), 4); // max(2, 4)
        assert_eq!(counter1.value(), 9);
    }

    #[test]
    fn test_gcounter_crdt_laws() {
        use crate::utils::*;
        
        let replica1 = ReplicaId::new();
        let replica2 = ReplicaId::new();
        
        let mut c1 = GrowOnlyCounter::new();
        c1.increment(replica1, 3);
        
        let mut c2 = GrowOnlyCounter::new();
        c2.increment(replica2, 2);
        
        let mut c3 = GrowOnlyCounter::new();
        c3.increment(replica1, 1);
        c3.increment(replica2, 1);
        
        assert!(test_idempotency(&c1));
        assert!(test_commutativity(&c1, &c2));
        assert!(test_associativity(&c1, &c2, &c3));
        assert!(verify_crdt_laws(&[c1, c2, c3]));
    }
}