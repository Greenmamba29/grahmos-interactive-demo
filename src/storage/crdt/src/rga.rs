// Replicated Growable Array (RGA) CRDT for sequences
use crate::{CRDT, StateCRDT, ReplicaId, HybridTimestamp};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RGAElement<T> {
    pub value: T,
    pub timestamp: HybridTimestamp,
    pub removed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]  
pub struct RGA<T> {
    elements: BTreeMap<HybridTimestamp, RGAElement<T>>,
}

impl<T> RGA<T>
where T: Clone + std::fmt::Debug + Serialize + for<'de> Deserialize<'de>
{
    pub fn new() -> Self {
        Self {
            elements: BTreeMap::new(),
        }
    }
    
    pub fn insert(&mut self, value: T, replica_id: ReplicaId) {
        let timestamp = HybridTimestamp::new(replica_id);
        let element = RGAElement {
            value,
            timestamp,
            removed: false,
        };
        self.elements.insert(timestamp, element);
    }
    
    pub fn to_string(&self) -> String 
    where T: std::fmt::Display
    {
        self.elements
            .values()
            .filter(|e| !e.removed)
            .map(|e| e.value.to_string())
            .collect::<Vec<_>>()
            .join("")
    }
}

impl<T> CRDT for RGA<T>
where T: Clone + std::fmt::Debug + Serialize + for<'de> Deserialize<'de>
{
    fn merge(&mut self, other: &Self) {
        for (timestamp, element) in &other.elements {
            self.elements.entry(*timestamp)
                .and_modify(|e| e.removed = e.removed || element.removed)
                .or_insert_with(|| element.clone());
        }
    }
}

impl<T> StateCRDT for RGA<T>
where T: Clone + std::fmt::Debug + Serialize + for<'de> Deserialize<'de>
{}