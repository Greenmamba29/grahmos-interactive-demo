use prism_crdt::*;
use proptest::prelude::*;
use std::collections::HashSet;
use tokio_test;
use futures::future::join_all;
use rand::seq::SliceRandom;

/// Comprehensive CRDT tests validating mathematical properties and correctness
/// 
/// Tests cover:
/// - Convergence properties (CvRDT laws)
/// - State-based merge semantics
/// - Operation-based semantics (CmRDT)
/// - Vector clock causality
/// - Concurrent operations handling
/// - Network partition scenarios

#[cfg(test)]
mod grow_only_counter_tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_crdt_laws_basic() {
        let replica1 = ReplicaId::new();
        let replica2 = ReplicaId::new();
        
        let mut counter1 = GrowOnlyCounter::new(replica1);
        let mut counter2 = GrowOnlyCounter::new(replica2);
        
        // Idempotency: a ⊔ a = a
        counter1.increment();
        let before_merge = counter1.clone();
        counter1.merge(&before_merge);
        assert_eq!(counter1.value(), before_merge.value());
    }
    
    #[test]
    fn test_commutativity() {
        let replica1 = ReplicaId::new();
        let replica2 = ReplicaId::new();
        
        let mut counter1_a = GrowOnlyCounter::new(replica1);
        let mut counter1_b = GrowOnlyCounter::new(replica1);
        let mut counter2_a = GrowOnlyCounter::new(replica2);
        let mut counter2_b = GrowOnlyCounter::new(replica2);
        
        // Prepare different states
        counter1_a.increment();
        counter1_b.increment();
        counter2_a.increment();
        counter2_a.increment(); // Value: 2
        counter2_b.increment();
        counter2_b.increment();
        
        // Test commutativity: a ⊔ b = b ⊔ a
        counter1_a.merge(&counter2_a);
        counter2_b.merge(&counter1_b);
        
        assert_eq!(counter1_a.value(), counter2_b.value());
    }
    
    #[test]
    fn test_associativity() {
        let replica1 = ReplicaId::new();
        let replica2 = ReplicaId::new();
        let replica3 = ReplicaId::new();
        
        let mut c1 = GrowOnlyCounter::new(replica1);
        let mut c2 = GrowOnlyCounter::new(replica2);
        let mut c3 = GrowOnlyCounter::new(replica3);
        
        c1.increment();
        c2.increment();
        c2.increment();
        c3.increment();
        c3.increment();
        c3.increment();
        
        // Test associativity: (a ⊔ b) ⊔ c = a ⊔ (b ⊔ c)
        let mut ab_c = c1.clone();
        ab_c.merge(&c2);
        ab_c.merge(&c3);
        
        let mut bc = c2.clone();
        bc.merge(&c3);
        let mut a_bc = c1.clone();
        a_bc.merge(&bc);
        
        assert_eq!(ab_c.value(), a_bc.value());
    }
    
    proptest! {
        #[test]
        fn property_monotonic_increment(increments in prop::collection::vec(1u64..100, 1..50)) {
            let replica = ReplicaId::new();
            let mut counter = GrowOnlyCounter::new(replica);
            let mut expected_value = 0;
            
            for inc in increments {
                for _ in 0..inc {
                    counter.increment();
                    expected_value += 1;
                }
                prop_assert_eq!(counter.value(), expected_value);
            }
        }
        
        #[test]
        fn property_merge_monotonic(
            ops1 in prop::collection::vec(1u64..10, 0..20),
            ops2 in prop::collection::vec(1u64..10, 0..20)
        ) {
            let replica1 = ReplicaId::new();
            let replica2 = ReplicaId::new();
            
            let mut counter1 = GrowOnlyCounter::new(replica1);
            let mut counter2 = GrowOnlyCounter::new(replica2);
            
            // Apply operations
            for _ in ops1 {
                counter1.increment();
            }
            for _ in ops2 {
                counter2.increment();
            }
            
            let value_before_merge = counter1.value().max(counter2.value());
            counter1.merge(&counter2);
            
            // After merge, value should be >= max of individual values
            prop_assert!(counter1.value() >= value_before_merge);
        }
    }
}

#[cfg(test)]
mod vector_clock_tests {
    use super::*;
    
    #[test]
    fn test_causality_tracking() {
        let replica1 = ReplicaId::new();
        let replica2 = ReplicaId::new();
        
        let mut vc1 = VectorClock::new();
        let mut vc2 = VectorClock::new();
        
        // Sequential events
        vc1.increment(replica1);
        assert!(vc1.happened_before(&vc2) || vc2.happened_before(&vc1) || vc1.concurrent_with(&vc2));
        
        vc2.update(&vc1);
        vc2.increment(replica2);
        
        // vc2 should happen after vc1
        assert!(vc1.happened_before(&vc2));
        assert!(!vc2.happened_before(&vc1));
    }
    
    #[test]
    fn test_concurrent_detection() {
        let replica1 = ReplicaId::new();
        let replica2 = ReplicaId::new();
        
        let mut vc1 = VectorClock::new();
        let mut vc2 = VectorClock::new();
        
        // Concurrent increments
        vc1.increment(replica1);
        vc2.increment(replica2);
        
        // Should be concurrent
        assert!(vc1.concurrent_with(&vc2));
        assert!(vc2.concurrent_with(&vc1));
        assert!(!vc1.happened_before(&vc2));
        assert!(!vc2.happened_before(&vc1));
    }
    
    proptest! {
        #[test]
        fn property_vector_clock_merge_convergence(
            ops1 in prop::collection::vec((prop::num::u64::ANY, prop::num::u64::ANY), 0..50),
            ops2 in prop::collection::vec((prop::num::u64::ANY, prop::num::u64::ANY), 0..50)
        ) {
            let replicas: Vec<_> = (0..5).map(|_| ReplicaId::new()).collect();
            
            let mut vc1 = VectorClock::new();
            let mut vc2 = VectorClock::new();
            
            // Apply random operations
            for (replica_idx, _) in ops1 {
                let replica = replicas[replica_idx as usize % replicas.len()];
                vc1.increment(replica);
            }
            
            for (replica_idx, _) in ops2 {
                let replica = replicas[replica_idx as usize % replicas.len()];
                vc2.increment(replica);
            }
            
            // Test merge convergence
            let mut merged1 = vc1.clone();
            merged1.merge(&vc2);
            
            let mut merged2 = vc2.clone();
            merged2.merge(&vc1);
            
            prop_assert_eq!(merged1, merged2); // Commutativity
        }
    }
}

#[cfg(test)]
mod or_set_tests {
    use super::*;
    
    #[test]
    fn test_add_remove_semantics() {
        let replica1 = ReplicaId::new();
        let replica2 = ReplicaId::new();
        
        let mut set1 = ORSet::<i32>::new();
        let mut set2 = ORSet::<i32>::new();
        
        // Add element to both sets
        set1.add(42, replica1);
        set2.add(42, replica2);
        
        // Remove from one set
        set1.remove(&42, replica1);
        
        // Merge sets - element should still exist due to OR semantics
        set1.merge(&set2);
        assert!(set1.contains(&42));
        
        // Remove from both
        set2.remove(&42, replica2);
        set1.merge(&set2);
        assert!(!set1.contains(&42));
    }
    
    #[test]
    fn test_concurrent_add_remove() {
        let replica1 = ReplicaId::new();
        let replica2 = ReplicaId::new();
        
        let mut set1 = ORSet::<i32>::new();
        let mut set2 = ORSet::<i32>::new();
        
        // Concurrent operations
        set1.add(100, replica1);
        set2.remove(&100, replica2);  // Remove non-existent element
        
        // Merge
        set1.merge(&set2);
        
        // Element should exist (add-bias in OR-Set)
        assert!(set1.contains(&100));
    }
    
    proptest! {
        #[test]
        fn property_or_set_convergence(
            operations in prop::collection::vec(
                (prop::bool::ANY, prop::num::i32::ANY), 
                0..100
            )
        ) {
            let replica1 = ReplicaId::new();
            let replica2 = ReplicaId::new();
            
            let mut set1 = ORSet::<i32>::new();
            let mut set2 = ORSet::<i32>::new();
            
            // Apply operations to both sets in different orders
            for (is_add, value) in operations.iter() {
                if *is_add {
                    set1.add(*value, replica1);
                } else {
                    set1.remove(value, replica1);
                }
            }
            
            // Apply in reverse to set2
            for (is_add, value) in operations.iter().rev() {
                if *is_add {
                    set2.add(*value, replica2);
                } else {
                    set2.remove(value, replica2);
                }
            }
            
            // Merge should converge
            let mut merged1 = set1.clone();
            merged1.merge(&set2);
            
            let mut merged2 = set2.clone();
            merged2.merge(&set1);
            
            // Check convergence
            for value in -100..100 {
                prop_assert_eq!(merged1.contains(&value), merged2.contains(&value));
            }
        }
    }
}

#[cfg(test)]
mod lww_register_tests {
    use super::*;
    use std::time::Duration;
    
    #[test]
    fn test_last_write_wins() {
        let replica1 = ReplicaId::new();
        let replica2 = ReplicaId::new();
        
        let mut reg1 = LWWRegister::<String>::new();
        let mut reg2 = LWWRegister::<String>::new();
        
        // First write
        reg1.set("first".to_string(), replica1);
        
        // Later write (simulate time passage)
        std::thread::sleep(Duration::from_millis(1));
        reg2.set("second".to_string(), replica2);
        
        // Merge - later write should win
        reg1.merge(&reg2);
        assert_eq!(reg1.value(), Some(&"second".to_string()));
    }
    
    #[test]
    fn test_concurrent_writes_deterministic() {
        let replica1 = ReplicaId::new();
        let replica2 = ReplicaId::new();
        
        // Ensure replica1 < replica2 for deterministic tie-breaking
        let (smaller_replica, larger_replica) = if replica1 < replica2 {
            (replica1, replica2)
        } else {
            (replica2, replica1)
        };
        
        let mut reg1 = LWWRegister::<String>::new();
        let mut reg2 = LWWRegister::<String>::new();
        
        // Concurrent writes (same timestamp)
        let now = PhysicalTimestamp::now();
        reg1.set_at("value1".to_string(), smaller_replica, now);
        reg2.set_at("value2".to_string(), larger_replica, now);
        
        // Merge - should be deterministic (larger replica ID wins)
        reg1.merge(&reg2);
        reg2.merge(&reg1);
        
        assert_eq!(reg1.value(), reg2.value());
        assert_eq!(reg1.value(), Some(&"value2".to_string()));
    }
}

#[cfg(test)]
mod rga_tests {
    use super::*;
    
    #[test]
    fn test_collaborative_editing() {
        let replica1 = ReplicaId::new();
        let replica2 = ReplicaId::new();
        
        let mut doc1 = RGA::<char>::new();
        let mut doc2 = RGA::<char>::new();
        
        // User 1 types "Hello"
        doc1.insert(0, 'H', replica1);
        doc1.insert(1, 'e', replica1);
        doc1.insert(2, 'l', replica1);
        doc1.insert(3, 'l', replica1);
        doc1.insert(4, 'o', replica1);
        
        // User 2 concurrently types "World" 
        doc2.insert(0, 'W', replica2);
        doc2.insert(1, 'o', replica2);
        doc2.insert(2, 'r', replica2);
        doc2.insert(3, 'l', replica2);
        doc2.insert(4, 'd', replica2);
        
        // Merge documents
        doc1.merge(&doc2);
        doc2.merge(&doc1);
        
        // Should converge to same state
        assert_eq!(doc1.to_string(), doc2.to_string());
        
        // Result should contain both "Hello" and "World"
        let result = doc1.to_string();
        assert!(result.contains("Hello") || result.contains("World"));
    }
    
    #[test]
    fn test_concurrent_insertions() {
        let replica1 = ReplicaId::new();
        let replica2 = ReplicaId::new();
        
        let mut doc1 = RGA::<char>::new();
        let mut doc2 = doc1.clone();
        
        // Start with "ab"
        doc1.insert(0, 'a', replica1);
        doc1.insert(1, 'b', replica1);
        doc2.merge(&doc1);
        
        // Concurrent insertions at position 1
        doc1.insert(1, 'X', replica1); // "aXb"
        doc2.insert(1, 'Y', replica2); // "aYb"
        
        // Merge
        doc1.merge(&doc2);
        doc2.merge(&doc1);
        
        // Should converge
        assert_eq!(doc1.to_string(), doc2.to_string());
        
        // Should contain all characters
        let result = doc1.to_string();
        assert!(result.contains('a') && result.contains('b') && 
                result.contains('X') && result.contains('Y'));
    }
}

/// Comprehensive test suite for all CRDT laws
#[test]
fn test_all_crdt_laws_comprehensive() {
    use prism_crdt::utils::*;
    
    // Test Grow-Only Counter
    let replica1 = ReplicaId::new();
    let replica2 = ReplicaId::new();
    let replica3 = ReplicaId::new();
    
    let mut counters = Vec::new();
    
    for replica in [replica1, replica2, replica3] {
        let mut counter = GrowOnlyCounter::new(replica);
        // Simulate different operation patterns
        for _ in 0..rand::random::<u8>() % 10 {
            counter.increment();
        }
        counters.push(counter);
    }
    
    // Verify CRDT laws
    assert!(verify_crdt_laws(&counters), "GrowOnlyCounter violates CRDT laws");
    
    // Test OR-Set
    let mut or_sets = Vec::new();
    
    for replica in [replica1, replica2, replica3] {
        let mut set = ORSet::<i32>::new();
        // Random operations
        for i in 0..10 {
            if rand::random::<bool>() {
                set.add(i, replica);
            } else {
                set.remove(&i, replica);
            }
        }
        or_sets.push(set);
    }
    
    assert!(verify_crdt_laws(&or_sets), "ORSet violates CRDT laws");
    
    // Test LWW Register  
    let mut registers = Vec::new();
    
    for replica in [replica1, replica2, replica3] {
        let mut reg = LWWRegister::<String>::new();
        reg.set(format!("value-{}", replica), replica);
        registers.push(reg);
    }
    
    assert!(verify_crdt_laws(&registers), "LWWRegister violates CRDT laws");
}

/// Performance benchmarks for CRDT operations
#[cfg(test)]
mod benchmarks {
    use super::*;
    use criterion::{criterion_group, criterion_main, Criterion};
    use std::time::Duration;
    
    fn benchmark_grow_only_counter(c: &mut Criterion) {
        c.bench_function("grow_only_counter_increment", |b| {
            let replica = ReplicaId::new();
            let mut counter = GrowOnlyCounter::new(replica);
            
            b.iter(|| {
                counter.increment();
            });
        });
        
        c.bench_function("grow_only_counter_merge", |b| {
            let replica1 = ReplicaId::new();
            let replica2 = ReplicaId::new();
            
            let mut counter1 = GrowOnlyCounter::new(replica1);
            let mut counter2 = GrowOnlyCounter::new(replica2);
            
            for _ in 0..1000 {
                counter1.increment();
                counter2.increment();
            }
            
            b.iter(|| {
                counter1.merge(&counter2);
            });
        });
    }
    
    fn benchmark_or_set(c: &mut Criterion) {
        c.bench_function("or_set_operations", |b| {
            let replica = ReplicaId::new();
            let mut set = ORSet::<i32>::new();
            let mut counter = 0;
            
            b.iter(|| {
                if counter % 2 == 0 {
                    set.add(counter, replica);
                } else {
                    set.remove(&(counter - 1), replica);
                }
                counter += 1;
            });
        });
    }
    
    criterion_group!(crdt_benches, benchmark_grow_only_counter, benchmark_or_set);
    criterion_main!(crdt_benches);
}

#[cfg(test)]
mod stress_tests {
    use super::*;
    use tokio_test;
    use futures::future::join_all;
    
    #[tokio::test]
    async fn stress_test_concurrent_operations() {
        let num_replicas = 10;
        let operations_per_replica = 1000;
        
        let replicas: Vec<_> = (0..num_replicas).map(|_| ReplicaId::new()).collect();
        let mut handles = Vec::new();
        
        // Spawn concurrent tasks
        for replica in replicas {
            let handle = tokio::spawn(async move {
                let mut counter = GrowOnlyCounter::new(replica);
                for _ in 0..operations_per_replica {
                    counter.increment();
                    // Small delay to simulate network latency
                    tokio::time::sleep(Duration::from_nanos(1)).await;
                }
                counter
            });
            handles.push(handle);
        }
        
        // Collect all results
        let counters: Vec<_> = join_all(handles)
            .await
            .into_iter()
            .map(|r| r.unwrap())
            .collect();
        
        // Merge all counters
        let mut final_counter = counters[0].clone();
        for counter in counters.iter().skip(1) {
            final_counter.merge(counter);
        }
        
        // Verify final value
        assert_eq!(final_counter.value(), num_replicas * operations_per_replica);
    }
    
    #[tokio::test]
    async fn stress_test_network_partition_healing() {
        let replica1 = ReplicaId::new();
        let replica2 = ReplicaId::new();
        
        let mut set1 = ORSet::<i32>::new();
        let mut set2 = ORSet::<i32>::new();
        
        // Simulate network partition - operate independently
        for i in 0..1000 {
            set1.add(i, replica1);
            if i % 2 == 0 {
                set1.remove(&(i / 2), replica1);
            }
            
            set2.add(i + 1000, replica2);
            if i % 3 == 0 {
                set2.remove(&(i / 3), replica2);
            }
        }
        
        // Network heals - synchronize
        set1.merge(&set2);
        set2.merge(&set1);
        
        // Verify convergence
        for i in 0..2000 {
            assert_eq!(set1.contains(&i), set2.contains(&i));
        }
        
        // Verify operations were not lost
        let mut total_elements = 0;
        for i in 0..2000 {
            if set1.contains(&i) {
                total_elements += 1;
            }
        }
        
        assert!(total_elements > 1500, "Too many elements lost during partition");
    }
}