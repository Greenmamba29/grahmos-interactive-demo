// Last-Writer-Wins Register CRDT
use crate::{CRDT, StateCRDT, ReplicaId, HybridTimestamp};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LWWRegister<T> {
    value: Option<T>,
    timestamp: HybridTimestamp,
}

impl<T> LWWRegister<T> 
where T: Clone + std::fmt::Debug + Serialize + for<'de> Deserialize<'de>
{
    pub fn new(replica_id: ReplicaId) -> Self {
        Self {
            value: None,
            timestamp: HybridTimestamp::new(replica_id),
        }
    }
    
    pub fn set(&mut self, value: T, replica_id: ReplicaId) {
        self.value = Some(value);
        self.timestamp = HybridTimestamp::new(replica_id);
    }
    
    pub fn get(&self) -> Option<&T> {
        self.value.as_ref()
    }
}

impl<T> CRDT for LWWRegister<T>
where T: Clone + std::fmt::Debug + Serialize + for<'de> Deserialize<'de>
{
    fn merge(&mut self, other: &Self) {
        if other.timestamp > self.timestamp {
            self.value = other.value.clone();
            self.timestamp = other.timestamp;
        }
    }
}

impl<T> StateCRDT for LWWRegister<T>
where T: Clone + std::fmt::Debug + Serialize + for<'de> Deserialize<'de>
{}