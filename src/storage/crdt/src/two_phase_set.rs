// Two-Phase Set CRDT - Add and remove operations
// Implementation: Maintain two G-Sets (added and removed)
// Value: added - removed
use crate::{CRDT, StateCRDT, GrowOnlySet};
use serde::{Deserialize, Serialize};
use std::hash::Hash as StdHash;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TwoPhaseSet<T> {
    added: GrowOnlySet<T>,
    removed: GrowOnlySet<T>,
}

impl<T> TwoPhaseSet<T> 
where T: Clone + Ord + StdHash + std::fmt::Debug + Serialize + for<'de> Deserialize<'de>
{
    pub fn new() -> Self {
        Self {
            added: GrowOnlySet::new(),
            removed: GrowOnlySet::new(),
        }
    }
    
    pub fn add(&mut self, element: T) -> bool {
        !self.removed.contains(&element) && self.added.add(element)
    }
    
    pub fn remove(&mut self, element: T) -> bool {
        self.added.contains(&element) && self.removed.add(element)
    }
    
    pub fn contains(&self, element: &T) -> bool {
        self.added.contains(element) && !self.removed.contains(element)
    }
}

impl<T> CRDT for TwoPhaseSet<T>
where T: Clone + Ord + StdHash + std::fmt::Debug + Serialize + for<'de> Deserialize<'de>
{
    fn merge(&mut self, other: &Self) {
        self.added.merge(&other.added);
        self.removed.merge(&other.removed);
    }
}

impl<T> StateCRDT for TwoPhaseSet<T>
where T: Clone + Ord + StdHash + std::fmt::Debug + Serialize + for<'de> Deserialize<'de>
{}