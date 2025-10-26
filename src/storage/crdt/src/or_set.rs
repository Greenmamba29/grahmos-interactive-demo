// Observed-Remove Set CRDT
use crate::{CRDT, StateCRDT, ReplicaId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::hash::Hash as StdHash;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ORSet<T> {
    added: BTreeMap<T, BTreeMap<ReplicaId, u64>>, // element -> (replica -> tag)
    removed: BTreeMap<T, BTreeMap<ReplicaId, u64>>, // element -> (replica -> tag)
    next_tag: BTreeMap<ReplicaId, u64>,
}

impl<T> ORSet<T> 
where T: Clone + Ord + StdHash + std::fmt::Debug + Serialize + for<'de> Deserialize<'de>
{
    pub fn new() -> Self {
        Self {
            added: BTreeMap::new(),
            removed: BTreeMap::new(),
            next_tag: BTreeMap::new(),
        }
    }
    
    pub fn add(&mut self, element: T, replica_id: ReplicaId) {
        let tag = self.next_tag.entry(replica_id).or_insert(0);
        *tag += 1;
        
        self.added.entry(element)
            .or_insert_with(BTreeMap::new)
            .insert(replica_id, *tag);
    }
    
    pub fn contains(&self, element: &T) -> bool {
        if let Some(added_tags) = self.added.get(element) {
            if let Some(removed_tags) = self.removed.get(element) {
                // Element is present if any added tag is not in removed
                added_tags.iter().any(|(replica, tag)| {
                    removed_tags.get(replica).map_or(true, |removed_tag| tag > removed_tag)
                })
            } else {
                !added_tags.is_empty()
            }
        } else {
            false
        }
    }
}

impl<T> CRDT for ORSet<T>
where T: Clone + Ord + StdHash + std::fmt::Debug + Serialize + for<'de> Deserialize<'de>
{
    fn merge(&mut self, other: &Self) {
        // Merge added elements (union)
        for (element, tags) in &other.added {
            let entry = self.added.entry(element.clone()).or_insert_with(BTreeMap::new);
            for (&replica, &tag) in tags {
                entry.insert(replica, entry.get(&replica).map_or(tag, |&current| current.max(tag)));
            }
        }
        
        // Merge removed elements (union)
        for (element, tags) in &other.removed {
            let entry = self.removed.entry(element.clone()).or_insert_with(BTreeMap::new);
            for (&replica, &tag) in tags {
                entry.insert(replica, entry.get(&replica).map_or(tag, |&current| current.max(tag)));
            }
        }
        
        // Merge next tags
        for (&replica, &tag) in &other.next_tag {
            let current = self.next_tag.entry(replica).or_insert(0);
            *current = (*current).max(tag);
        }
    }
}

impl<T> StateCRDT for ORSet<T>
where T: Clone + Ord + StdHash + std::fmt::Debug + Serialize + for<'de> Deserialize<'de>
{}