use crate::{ReplicaId, CRDT, StateCRDT, GrowOnlyCounter};
use serde::{Deserialize, Serialize};

/// Increment/Decrement Counter (PN-Counter) CRDT
/// 
/// A counter that supports both increment and decrement operations.
/// Implemented as two G-Counters: one for increments, one for decrements.
/// The value is the difference between the two counters.
/// 
/// Properties:
/// - Supports increment and decrement
/// - Eventually consistent across all replicas
/// - Commutative and idempotent merge operations
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PNCounter {
    /// Counter for increment operations
    increments: GrowOnlyCounter,
    /// Counter for decrement operations  
    decrements: GrowOnlyCounter,
}

impl PNCounter {
    /// Create a new PN-Counter
    pub fn new() -> Self {
        Self {
            increments: GrowOnlyCounter::new(),
            decrements: GrowOnlyCounter::new(),
        }
    }
    
    /// Create PN-Counter with initial replica
    pub fn with_replica(replica_id: ReplicaId) -> Self {
        Self {
            increments: GrowOnlyCounter::with_replica(replica_id),
            decrements: GrowOnlyCounter::with_replica(replica_id),
        }
    }
    
    /// Increment the counter for a specific replica
    pub fn increment(&mut self, replica_id: ReplicaId, amount: u64) -> i64 {
        self.increments.increment(replica_id, amount);
        self.value()
    }
    
    /// Decrement the counter for a specific replica
    pub fn decrement(&mut self, replica_id: ReplicaId, amount: u64) -> i64 {
        self.decrements.increment(replica_id, amount);
        self.value()
    }
    
    /// Get the current value (increments - decrements)
    pub fn value(&self) -> i64 {
        self.increments.value() as i64 - self.decrements.value() as i64
    }
    
    /// Get the increment count for a specific replica
    pub fn get_replica_increments(&self, replica_id: &ReplicaId) -> u64 {
        self.increments.get_replica_count(replica_id)
    }
    
    /// Get the decrement count for a specific replica
    pub fn get_replica_decrements(&self, replica_id: &ReplicaId) -> u64 {
        self.decrements.get_replica_count(replica_id)
    }
    
    /// Get the net contribution of a specific replica
    pub fn get_replica_value(&self, replica_id: &ReplicaId) -> i64 {
        self.get_replica_increments(replica_id) as i64 - 
        self.get_replica_decrements(replica_id) as i64
    }
    
    /// Get all known replicas (from both counters)
    pub fn replicas(&self) -> impl Iterator<Item = &ReplicaId> {
        self.increments.replicas()
            .chain(self.decrements.replicas())
    }
    
    /// Get total number of known replicas
    pub fn replica_count(&self) -> usize {
        use std::collections::BTreeSet;
        let replicas: BTreeSet<_> = self.replicas().collect();
        replicas.len()
    }
    
    /// Check if both counters are empty
    pub fn is_empty(&self) -> bool {
        self.increments.is_empty() && self.decrements.is_empty()
    }
    
    /// Get the total number of operations performed
    pub fn operation_count(&self) -> u64 {
        self.increments.value() + self.decrements.value()
    }
    
    /// Get reference to increment counter
    pub fn increments(&self) -> &GrowOnlyCounter {
        &self.increments
    }
    
    /// Get reference to decrement counter
    pub fn decrements(&self) -> &GrowOnlyCounter {
        &self.decrements
    }
}

impl Default for PNCounter {
    fn default() -> Self {
        Self::new()
    }
}

impl CRDT for PNCounter {
    fn merge(&mut self, other: &Self) {
        self.increments.merge(&other.increments);
        self.decrements.merge(&other.decrements);
    }
    
    fn is_subset_of(&self, other: &Self) -> bool {
        self.increments.is_subset_of(&other.increments) &&
        self.decrements.is_subset_of(&other.decrements)
    }
}

impl StateCRDT for PNCounter {}

impl std::fmt::Display for PNCounter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PNCounter({})", self.value())
    }
}

/// Operations for PN-Counter
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PNCounterOp {
    /// Increment by amount
    Increment { replica_id: ReplicaId, amount: u64 },
    /// Decrement by amount
    Decrement { replica_id: ReplicaId, amount: u64 },
}

impl PNCounterOp {
    /// Apply this operation to a PN-Counter
    pub fn apply(&self, counter: &mut PNCounter) -> i64 {
        match self {
            PNCounterOp::Increment { replica_id, amount } => {
                counter.increment(*replica_id, *amount)
            }
            PNCounterOp::Decrement { replica_id, amount } => {
                counter.decrement(*replica_id, *amount)
            }
        }
    }
    
    /// Get the replica that generated this operation
    pub fn replica_id(&self) -> ReplicaId {
        match self {
            PNCounterOp::Increment { replica_id, .. } => *replica_id,
            PNCounterOp::Decrement { replica_id, .. } => *replica_id,
        }
    }
    
    /// Get the amount of this operation
    pub fn amount(&self) -> u64 {
        match self {
            PNCounterOp::Increment { amount, .. } => *amount,
            PNCounterOp::Decrement { amount, .. } => *amount,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pncounter_creation() {
        let counter = PNCounter::new();
        assert_eq!(counter.value(), 0);
        assert!(counter.is_empty());
        
        let replica = ReplicaId::new();
        let counter = PNCounter::with_replica(replica);
        assert_eq!(counter.get_replica_increments(&replica), 0);
        assert_eq!(counter.get_replica_decrements(&replica), 0);
    }

    #[test]
    fn test_pncounter_operations() {
        let replica1 = ReplicaId::new();
        let replica2 = ReplicaId::new();
        
        let mut counter = PNCounter::new();
        
        assert_eq!(counter.increment(replica1, 5), 5);
        assert_eq!(counter.increment(replica2, 3), 8);
        assert_eq!(counter.decrement(replica1, 2), 6);
        assert_eq!(counter.decrement(replica2, 4), 2);
        
        assert_eq!(counter.value(), 2);
        assert_eq!(counter.get_replica_value(&replica1), 3); // 5 - 2
        assert_eq!(counter.get_replica_value(&replica2), -1); // 3 - 4
        assert_eq!(counter.operation_count(), 14); // (5+3) + (2+4)
    }

    #[test]
    fn test_pncounter_merge() {
        let replica1 = ReplicaId::new();
        let replica2 = ReplicaId::new();
        
        let mut counter1 = PNCounter::new();
        counter1.increment(replica1, 5);
        counter1.decrement(replica2, 2);
        
        let mut counter2 = PNCounter::new();
        counter2.increment(replica1, 3);
        counter2.decrement(replica2, 4);
        
        counter1.merge(&counter2);
        
        // Should take max of each replica's operations
        assert_eq!(counter1.get_replica_increments(&replica1), 5); // max(5, 3)
        assert_eq!(counter1.get_replica_decrements(&replica2), 4); // max(2, 4)
        assert_eq!(counter1.value(), 1); // 5 - 4
    }

    #[test]
    fn test_pncounter_operations_enum() {
        let replica = ReplicaId::new();
        let mut counter = PNCounter::new();
        
        let inc_op = PNCounterOp::Increment { replica_id: replica, amount: 10 };
        let dec_op = PNCounterOp::Decrement { replica_id: replica, amount: 3 };
        
        assert_eq!(inc_op.apply(&mut counter), 10);
        assert_eq!(dec_op.apply(&mut counter), 7);
        
        assert_eq!(inc_op.replica_id(), replica);
        assert_eq!(inc_op.amount(), 10);
        assert_eq!(dec_op.amount(), 3);
    }

    #[test]
    fn test_pncounter_crdt_laws() {
        use crate::utils::*;
        
        let replica1 = ReplicaId::new();
        let replica2 = ReplicaId::new();
        
        let mut c1 = PNCounter::new();
        c1.increment(replica1, 5);
        c1.decrement(replica1, 2);
        
        let mut c2 = PNCounter::new();
        c2.increment(replica2, 3);
        c2.decrement(replica2, 1);
        
        let mut c3 = PNCounter::new();
        c3.increment(replica1, 1);
        c3.increment(replica2, 2);
        
        assert!(test_idempotency(&c1));
        assert!(test_commutativity(&c1, &c2));
        assert!(test_associativity(&c1, &c2, &c3));
        assert!(verify_crdt_laws(&[c1, c2, c3]));
    }

    #[test]
    fn test_pncounter_negative_values() {
        let replica = ReplicaId::new();
        let mut counter = PNCounter::new();
        
        // Start with decrements
        counter.decrement(replica, 10);
        assert_eq!(counter.value(), -10);
        
        // Add some increments
        counter.increment(replica, 3);
        assert_eq!(counter.value(), -7);
        
        // More increments to go positive
        counter.increment(replica, 15);
        assert_eq!(counter.value(), 8);
    }
}