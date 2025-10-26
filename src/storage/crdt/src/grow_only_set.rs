use crate::{ReplicaId, CRDT, StateCRDT};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::hash::Hash as StdHash;

/// Grow-Only Set (G-Set) CRDT
/// 
/// A set that supports only add operations, no removals.
/// Elements can be added but never removed.
/// 
/// Properties:
/// - Add-only: elements can only be added
/// - Eventually consistent: all replicas converge to union
/// - Idempotent: adding same element multiple times has no effect
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrowOnlySet<T> {
    /// Set of elements
    elements: BTreeSet<T>,
}

impl<T> GrowOnlySet<T>
where 
    T: Clone + Ord + StdHash
{
    /// Create a new G-Set
    pub fn new() -> Self {
        Self {
            elements: BTreeSet::new(),
        }
    }
    
    /// Add an element to the set
    pub fn add(&mut self, element: T) -> bool {
        self.elements.insert(element)
    }
    
    /// Check if an element is in the set
    pub fn contains(&self, element: &T) -> bool {
        self.elements.contains(element)
    }
    
    /// Get the number of elements in the set
    pub fn len(&self) -> usize {
        self.elements.len()
    }
    
    /// Check if the set is empty
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }
    
    /// Get an iterator over the elements
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.elements.iter()
    }
    
    /// Get all elements as a vector
    pub fn elements(&self) -> Vec<T> {
        self.elements.iter().cloned().collect()
    }
    
    /// Check if this set is a subset of another
    pub fn is_subset(&self, other: &GrowOnlySet<T>) -> bool {
        self.elements.is_subset(&other.elements)
    }
    
    /// Check if this set is a superset of another
    pub fn is_superset(&self, other: &GrowOnlySet<T>) -> bool {
        self.elements.is_superset(&other.elements)
    }
    
    /// Get the union with another set (without modifying this set)
    pub fn union(&self, other: &GrowOnlySet<T>) -> GrowOnlySet<T> {
        let union: BTreeSet<T> = self.elements.union(&other.elements).cloned().collect();
        GrowOnlySet { elements: union }
    }
    
    /// Get the intersection with another set
    pub fn intersection(&self, other: &GrowOnlySet<T>) -> GrowOnlySet<T> {
        let intersection: BTreeSet<T> = self.elements.intersection(&other.elements).cloned().collect();
        GrowOnlySet { elements: intersection }
    }
    
    /// Get the difference with another set (elements in self but not in other)
    pub fn difference(&self, other: &GrowOnlySet<T>) -> GrowOnlySet<T> {
        let difference: BTreeSet<T> = self.elements.difference(&other.elements).cloned().collect();
        GrowOnlySet { elements: difference }
    }
}

impl<T> Default for GrowOnlySet<T>
where 
    T: Clone + Ord + StdHash
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T> CRDT for GrowOnlySet<T>
where 
    T: Clone + Ord + StdHash + std::fmt::Debug + Serialize + for<'de> Deserialize<'de>
{
    fn merge(&mut self, other: &Self) {
        for element in &other.elements {
            self.elements.insert(element.clone());
        }
    }
    
    fn is_subset_of(&self, other: &Self) -> bool {
        self.is_subset(other)
    }
}

impl<T> StateCRDT for GrowOnlySet<T>
where 
    T: Clone + Ord + StdHash + std::fmt::Debug + Serialize + for<'de> Deserialize<'de>
{}

impl<T> std::fmt::Display for GrowOnlySet<T>
where 
    T: Clone + Ord + StdHash + std::fmt::Display
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "GSet{{")?;
        let mut first = true;
        for element in &self.elements {
            if !first {
                write!(f, ", ")?;
            }
            write!(f, "{}", element)?;
            first = false;
        }
        write!(f, "}}")
    }
}

// Implement FromIterator for convenience
impl<T> FromIterator<T> for GrowOnlySet<T>
where 
    T: Clone + Ord + StdHash
{
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let elements = iter.into_iter().collect();
        Self { elements }
    }
}

// Implement IntoIterator
impl<T> IntoIterator for GrowOnlySet<T>
where 
    T: Clone + Ord + StdHash
{
    type Item = T;
    type IntoIter = std::collections::btree_set::IntoIter<T>;
    
    fn into_iter(self) -> Self::IntoIter {
        self.elements.into_iter()
    }
}

impl<'a, T> IntoIterator for &'a GrowOnlySet<T>
where 
    T: Clone + Ord + StdHash
{
    type Item = &'a T;
    type IntoIter = std::collections::btree_set::Iter<'a, T>;
    
    fn into_iter(self) -> Self::IntoIter {
        self.elements.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gset_creation() {
        let set: GrowOnlySet<i32> = GrowOnlySet::new();
        assert!(set.is_empty());
        assert_eq!(set.len(), 0);
    }

    #[test]
    fn test_gset_add() {
        let mut set = GrowOnlySet::new();
        
        assert!(set.add(1));  // First add returns true
        assert!(!set.add(1)); // Second add returns false (already exists)
        assert!(set.add(2));
        
        assert_eq!(set.len(), 2);
        assert!(set.contains(&1));
        assert!(set.contains(&2));
        assert!(!set.contains(&3));
    }

    #[test]
    fn test_gset_merge() {
        let mut set1 = GrowOnlySet::new();
        set1.add(1);
        set1.add(2);
        
        let mut set2 = GrowOnlySet::new();
        set2.add(2);
        set2.add(3);
        
        set1.merge(&set2);
        
        assert_eq!(set1.len(), 3);
        assert!(set1.contains(&1));
        assert!(set1.contains(&2));
        assert!(set1.contains(&3));
    }

    #[test]
    fn test_gset_operations() {
        let mut set1 = GrowOnlySet::new();
        set1.add(1);
        set1.add(2);
        set1.add(3);
        
        let mut set2 = GrowOnlySet::new();
        set2.add(2);
        set2.add(3);
        set2.add(4);
        
        assert!(set2.is_subset(&set1.union(&set2)));
        
        let intersection = set1.intersection(&set2);
        assert_eq!(intersection.len(), 2);
        assert!(intersection.contains(&2));
        assert!(intersection.contains(&3));
        
        let difference = set1.difference(&set2);
        assert_eq!(difference.len(), 1);
        assert!(difference.contains(&1));
    }

    #[test]
    fn test_gset_crdt_laws() {
        use crate::utils::*;
        
        let mut s1 = GrowOnlySet::new();
        s1.add("a");
        s1.add("b");
        
        let mut s2 = GrowOnlySet::new();
        s2.add("b");
        s2.add("c");
        
        let mut s3 = GrowOnlySet::new();
        s3.add("c");
        s3.add("d");
        
        assert!(test_idempotency(&s1));
        assert!(test_commutativity(&s1, &s2));
        assert!(test_associativity(&s1, &s2, &s3));
        assert!(verify_crdt_laws(&[s1, s2, s3]));
    }

    #[test]
    fn test_gset_iterators() {
        let mut set = GrowOnlySet::new();
        set.add(1);
        set.add(2);
        set.add(3);
        
        let elements: Vec<_> = set.iter().cloned().collect();
        assert_eq!(elements, vec![1, 2, 3]);
        
        let from_iter: GrowOnlySet<i32> = vec![4, 5, 6].into_iter().collect();
        assert_eq!(from_iter.len(), 3);
        assert!(from_iter.contains(&5));
    }
}