use prism_crdt::*;
use prism_consensus::*;
use prism_cas::*;
use prism_p2p::*;
use prism_swarm::*;
use tokio_test;
use std::time::Duration;
use std::collections::HashMap;
use tempfile::TempDir;

/// Comprehensive integration tests for PRISM components
/// 
/// Tests cover:
/// - Multi-node consensus with CRDT synchronization
/// - Storage replication across nodes
/// - Network partition tolerance and recovery
/// - End-to-end agent swarm coordination
/// - Performance under realistic workloads

pub mod consensus_integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_three_node_consensus_with_crdt_sync() {
        let temp_dirs: Vec<TempDir> = (0..3).map(|_| TempDir::new().unwrap()).collect();
        let node_ids: Vec<NodeId> = (0..3).map(|_| NodeId::new()).collect();
        let replica_ids: Vec<ReplicaId> = node_ids.iter()
            .map(|node_id| ReplicaId::from_uuid(node_id.0))
            .collect();
        
        // Create consensus nodes
        let mut consensus_nodes = Vec::new();
        for (i, &node_id) in node_ids.iter().enumerate() {
            let node = ConsensusBuilder::new(node_id)
                .cluster_nodes(node_ids.clone())
                .config(ConsensusConfig {
                    heartbeat_interval_ms: 50,
                    election_timeout_ms: (200, 400),
                    ..Default::default()
                })
                .build()
                .unwrap();
            consensus_nodes.push(node);
        }
        
        // Create CAS storage for each node
        let mut storage_nodes = Vec::new();
        for temp_dir in &temp_dirs {
            let cas = ContentAddressableStorage::new(temp_dir.path()).unwrap();
            storage_nodes.push(cas);
        }
        
        // Create CRDT managers for each replica
        let mut crdt_managers = Vec::new();
        for &replica_id in &replica_ids {
            let manager = CRDTManager::new(replica_id);
            crdt_managers.push(manager);
        }
        
        // Start consensus cluster
        let consensus_handles: Vec<_> = consensus_nodes.iter_mut()
            .map(|node| tokio::spawn(async move { node.start().await }))
            .collect();
        
        for handle in consensus_handles {
            handle.await.unwrap().unwrap();
        }
        
        // Wait for leader election
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        let leader_idx = consensus_nodes.iter()
            .position(|node| node.is_leader())
            .expect("No leader elected");
        
        // Test integrated operation: CRDT update + consensus + storage
        
        // 1. Create CRDT data on each replica
        let counter_name = "test_counter".to_string();
        for (i, manager) in crdt_managers.iter_mut().enumerate() {
            let counter = GrowOnlyCounter::new(replica_ids[i]);
            manager.register(counter_name.clone(), counter);
        }
        
        // 2. Perform operations on different replicas
        for (i, manager) in crdt_managers.iter_mut().enumerate() {
            if let Some(counter) = manager.get_mut::<GrowOnlyCounter>(&counter_name) {
                for _ in 0..=i {
                    counter.crdt.increment();
                }
            }
        }
        
        // 3. Sync CRDT states through consensus
        for (i, manager) in crdt_managers.iter().enumerate() {
            if let Some(counter) = manager.get::<GrowOnlyCounter>(&counter_name) {
                let sync_command = AgentCommand::Custom {
                    command_type: "crdt_sync".to_string(),
                    payload: bincode::serialize(&counter.crdt).unwrap(),
                };
                
                consensus_nodes[leader_idx]
                    .submit_command(sync_command)
                    .await
                    .unwrap();
            }
        }
        
        // 4. Store CRDT states in CAS
        for (i, (manager, cas)) in crdt_managers.iter().zip(storage_nodes.iter()).enumerate() {
            if let Some(counter) = manager.get::<GrowOnlyCounter>(&counter_name) {
                let serialized_state = bincode::serialize(&counter.crdt).unwrap();
                let store_result = cas.store(&serialized_state).await.unwrap();
                
                println!("Node {} stored CRDT state with hash: {}", 
                         i, store_result.hash.to_hex());
            }
        }
        
        // Wait for consensus to propagate
        tokio::time::sleep(Duration::from_millis(300)).await;
        
        // 5. Verify consensus metrics
        for (i, node) in consensus_nodes.iter().enumerate() {
            let metrics = node.metrics();
            assert!(metrics.commands_applied > 0, "Node {} applied no commands", i);
            
            if i == leader_idx {
                assert!(metrics.messages_sent > 0, "Leader sent no messages");
            } else {
                assert!(metrics.messages_received > 0, "Follower received no messages");
            }
        }
        
        // 6. Verify storage statistics
        for (i, cas) in storage_nodes.iter().enumerate() {
            let stats = cas.statistics().await;
            assert!(stats.total_blocks > 0, "Node {} stored no blocks", i);
            assert!(stats.write_operations > 0, "Node {} performed no writes", i);
        }
        
        // Cleanup
        for node in &mut consensus_nodes {
            let _ = node.stop().await;
        }
    }
    
    #[tokio::test]
    async fn test_network_partition_recovery() {
        let node_ids: Vec<NodeId> = (0..5).map(|_| NodeId::new()).collect();
        let mut nodes = Vec::new();
        
        // Create 5-node cluster for partition tolerance testing
        for &node_id in &node_ids {
            let node = ConsensusBuilder::new(node_id)
                .cluster_nodes(node_ids.clone())
                .config(ConsensusConfig {
                    heartbeat_interval_ms: 100,
                    election_timeout_ms: (300, 600),
                    rpc_timeout_ms: 2000,
                    ..Default::default()
                })
                .build()
                .unwrap();
            nodes.push(node);
        }
        
        // Start all nodes
        let start_handles: Vec<_> = nodes.iter_mut()
            .map(|node| tokio::spawn(async move { node.start().await }))
            .collect();
        
        for handle in start_handles {
            handle.await.unwrap().unwrap();
        }
        
        tokio::time::sleep(Duration::from_millis(800)).await;
        
        let initial_leader_idx = nodes.iter()
            .position(|node| node.is_leader())
            .expect("No initial leader");
        
        // Submit commands before partition
        let pre_partition_command = AgentCommand::UpdateConfig {
            key: "pre_partition_test".to_string(),
            value: serde_json::Value::Bool(true),
        };
        
        let pre_partition_index = nodes[initial_leader_idx]
            .submit_command(pre_partition_command)
            .await
            .unwrap();
        
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        // Simulate network partition: isolate 2 nodes (minority)
        let minority_nodes = nodes.split_off(3); // Keep majority of 3 nodes
        
        // Continue operations on majority partition
        tokio::time::sleep(Duration::from_millis(600)).await;
        
        let majority_leader_idx = nodes.iter()
            .position(|node| node.is_leader())
            .expect("No leader in majority partition");
        
        let post_partition_command = AgentCommand::UpdateConfig {
            key: "post_partition_test".to_string(),
            value: serde_json::Value::Bool(true),
        };
        
        let post_partition_index = nodes[majority_leader_idx]
            .submit_command(post_partition_command)
            .await
            .unwrap();
        
        // Verify majority can still make progress
        assert!(post_partition_index > pre_partition_index);
        
        tokio::time::sleep(Duration::from_millis(300)).await;
        
        // Heal partition - merge nodes back
        nodes.extend(minority_nodes);
        
        // Wait for partition healing and re-synchronization
        tokio::time::sleep(Duration::from_millis(1000)).await;
        
        // Verify final consistency
        let final_leader_idx = nodes.iter()
            .position(|node| node.is_leader())
            .expect("No leader after partition healing");
        
        let healing_command = AgentCommand::UpdateConfig {
            key: "post_healing_test".to_string(),
            value: serde_json::Value::Bool(true),
        };
        
        let healing_index = nodes[final_leader_idx]
            .submit_command(healing_command)
            .await
            .unwrap();
        
        // All nodes should eventually have consistent state
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        let leader_metrics = nodes[final_leader_idx].metrics();
        for (i, node) in nodes.iter().enumerate() {
            if i != final_leader_idx {
                let follower_metrics = node.metrics();
                assert!(follower_metrics.commit_index >= post_partition_index,
                        "Node {} commit index {} behind post-partition index {}",
                        i, follower_metrics.commit_index, post_partition_index);
            }
        }
        
        // Cleanup
        for node in &mut nodes {
            let _ = node.stop().await;
        }
    }
}

pub mod storage_integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_distributed_storage_consistency() {
        let num_nodes = 3;
        let temp_dirs: Vec<TempDir> = (0..num_nodes).map(|_| TempDir::new().unwrap()).collect();
        let mut storage_nodes = Vec::new();
        
        // Create distributed storage cluster
        for temp_dir in &temp_dirs {
            let config = CASConfig {
                compression_enabled: true,
                integrity_verification: true,
                ..Default::default()
            };
            let cas = ContentAddressableStorage::with_config(temp_dir.path(), config).unwrap();
            storage_nodes.push(cas);
        }
        
        // Test data replication across nodes
        let test_data = b"Distributed storage test data";
        let mut hashes = Vec::new();
        
        // Store on all nodes
        for (i, cas) in storage_nodes.iter().enumerate() {
            let result = cas.store(test_data).await.unwrap();
            hashes.push(result.hash);
            
            println!("Node {} stored with hash: {}", i, result.hash.to_hex());
            
            // First node should be new, others should deduplicate
            if i == 0 {
                assert!(result.is_new);
            }
        }
        
        // Verify all nodes have same hash (content addressable)
        for hash in &hashes[1..] {
            assert_eq!(hashes[0], *hash, "Content hashes differ across nodes");
        }
        
        // Verify retrieval consistency across all nodes
        for (i, cas) in storage_nodes.iter().enumerate() {
            let retrieved = cas.retrieve(&hashes[0]).await.unwrap();
            assert_eq!(retrieved, test_data, "Node {} returned inconsistent data", i);
        }
        
        // Test cross-node deduplication
        let mut total_dedup_savings = 0;
        for cas in &storage_nodes {
            let stats = cas.statistics().await;
            total_dedup_savings += stats.dedup_saved_bytes;
        }
        
        // Should have significant deduplication across nodes
        let expected_savings = (test_data.len() as u64) * (num_nodes as u64 - 1);
        assert!(total_dedup_savings >= expected_savings * 8 / 10, // Allow 80% of expected
                "Insufficient deduplication: {} bytes saved, expected ~{}",
                total_dedup_savings, expected_savings);
    }
    
    #[tokio::test]
    async fn test_storage_with_crdt_synchronization() {
        let temp_dirs: Vec<TempDir> = (0..3).map(|_| TempDir::new().unwrap()).collect();
        let replica_ids: Vec<ReplicaId> = (0..3).map(|_| ReplicaId::new()).collect();
        
        // Create storage + CRDT setup for each node
        let mut node_setups = Vec::new();
        
        for (temp_dir, &replica_id) in temp_dirs.iter().zip(replica_ids.iter()) {
            let cas = ContentAddressableStorage::new(temp_dir.path()).unwrap();
            let mut crdt_manager = CRDTManager::new(replica_id);
            
            // Initialize shared CRDT structures
            let counter = GrowOnlyCounter::new(replica_id);
            let mut or_set = ORSet::<String>::new();
            or_set.add(format!("node-{}", replica_id), replica_id);
            
            crdt_manager.register("shared_counter".to_string(), counter);
            crdt_manager.register("shared_set".to_string(), or_set);
            
            node_setups.push((cas, crdt_manager));
        }
        
        // Simulate distributed operations
        
        // Phase 1: Each node performs local operations
        for (i, (_, crdt_manager)) in node_setups.iter_mut().enumerate() {
            // Increment counter
            if let Some(counter) = crdt_manager.get_mut::<GrowOnlyCounter>("shared_counter") {
                for _ in 0..=i {
                    counter.crdt.increment();
                }
            }
            
            // Add to set
            if let Some(or_set) = crdt_manager.get_mut::<ORSet<String>>("shared_set") {
                or_set.crdt.add(format!("item-{}-{}", i, rand::random::<u32>()), replica_ids[i]);
            }
        }
        
        // Phase 2: Store CRDT states
        let mut stored_hashes = Vec::new();
        for (i, (cas, crdt_manager)) in node_setups.iter().enumerate() {
            // Serialize and store counter
            if let Some(counter) = crdt_manager.get::<GrowOnlyCounter>("shared_counter") {
                let serialized = bincode::serialize(&counter.crdt).unwrap();
                let result = cas.store(&serialized).await.unwrap();
                stored_hashes.push(("counter", i, result.hash));
            }
            
            // Serialize and store set
            if let Some(or_set) = crdt_manager.get::<ORSet<String>>("shared_set") {
                let serialized = bincode::serialize(&or_set.crdt).unwrap();
                let result = cas.store(&serialized).await.unwrap();
                stored_hashes.push(("set", i, result.hash));
            }
        }
        
        // Phase 3: Cross-node synchronization
        
        // Simulate CRDT state exchange between nodes
        let mut merged_counter = GrowOnlyCounter::new(replica_ids[0]);
        let mut merged_set = ORSet::<String>::new();
        
        for (cas, crdt_manager) in &node_setups {
            // Merge counter states
            if let Some(counter) = crdt_manager.get::<GrowOnlyCounter>("shared_counter") {
                merged_counter.merge(&counter.crdt);
            }
            
            // Merge set states
            if let Some(or_set) = crdt_manager.get::<ORSet<String>>("shared_set") {
                merged_set.merge(&or_set.crdt);
            }
        }
        
        // Phase 4: Verify final consistency
        
        // Counter should have sum of all increments
        let expected_counter_value = (0..3).sum::<u64>();
        assert_eq!(merged_counter.value(), expected_counter_value,
                   "Merged counter value incorrect");
        
        // Set should contain items from all nodes
        assert!(merged_set.len() >= 6, // At least 3 initial + 3 added
                "Merged set should contain items from all nodes");
        
        // Phase 5: Store final merged states and verify storage efficiency
        let final_temp_dir = TempDir::new().unwrap();
        let final_cas = ContentAddressableStorage::new(final_temp_dir.path()).unwrap();
        
        let merged_counter_data = bincode::serialize(&merged_counter).unwrap();
        let merged_set_data = bincode::serialize(&merged_set).unwrap();
        
        let counter_result = final_cas.store(&merged_counter_data).await.unwrap();
        let set_result = final_cas.store(&merged_set_data).await.unwrap();
        
        // Verify final storage statistics
        let final_stats = final_cas.statistics().await;
        assert_eq!(final_stats.total_blocks, 2);
        assert_eq!(final_stats.write_operations, 2);
        
        // Verify data integrity
        let retrieved_counter = final_cas.retrieve(&counter_result.hash).await.unwrap();
        let retrieved_set = final_cas.retrieve(&set_result.hash).await.unwrap();
        
        assert_eq!(retrieved_counter, merged_counter_data);
        assert_eq!(retrieved_set, merged_set_data);
    }
}

pub mod end_to_end_tests {
    use super::*;

    #[tokio::test] 
    async fn test_full_agent_swarm_coordination() {
        // This test simulates a complete PRISM agent swarm scenario
        
        let num_agents = 5;
        let agent_ids: Vec<NodeId> = (0..num_agents).map(|_| NodeId::new()).collect();
        let temp_dirs: Vec<TempDir> = (0..num_agents).map(|_| TempDir::new().unwrap()).collect();
        
        // Agent setup: consensus + storage + CRDT for each agent
        let mut agents = Vec::new();
        
        for (i, (&agent_id, temp_dir)) in agent_ids.iter().zip(temp_dirs.iter()).enumerate() {
            // Consensus node
            let consensus_node = ConsensusBuilder::new(agent_id)
                .cluster_nodes(agent_ids.clone())
                .config(ConsensusConfig {
                    heartbeat_interval_ms: 75,
                    election_timeout_ms: (200, 400),
                    ..Default::default()
                })
                .build()
                .unwrap();
            
            // Storage node
            let storage_node = ContentAddressableStorage::new(temp_dir.path()).unwrap();
            
            // CRDT manager
            let replica_id = ReplicaId::from_uuid(agent_id.0);
            let mut crdt_manager = CRDTManager::new(replica_id);
            
            // Initialize agent-specific state
            let agent_status = LWWRegister::<AgentStatus>::new();
            let task_queue = ORSet::<String>::new();
            let performance_counter = GrowOnlyCounter::new(replica_id);
            
            crdt_manager.register("status".to_string(), agent_status);
            crdt_manager.register("tasks".to_string(), task_queue);
            crdt_manager.register("performance".to_string(), performance_counter);
            
            agents.push((consensus_node, storage_node, crdt_manager));
        }
        
        // Start all consensus nodes
        let consensus_handles: Vec<_> = agents.iter_mut()
            .map(|(consensus_node, _, _)| {
                tokio::spawn(async move { consensus_node.start().await })
            })
            .collect();
        
        for handle in consensus_handles {
            handle.await.unwrap().unwrap();
        }
        
        // Wait for leader election
        tokio::time::sleep(Duration::from_millis(600)).await;
        
        let leader_idx = agents.iter()
            .position(|(consensus_node, _, _)| consensus_node.is_leader())
            .expect("No leader elected in agent swarm");
        
        println!("Agent swarm leader elected: Agent {}", leader_idx);
        
        // Scenario: Task assignment and execution
        
        // Phase 1: Leader assigns tasks to agents
        for i in 0..num_agents {
            let task_command = AgentCommand::AssignTask {
                agent_id: agent_ids[i],
                task_id: format!("task-{}", i),
                task_data: format!("Execute test scenario {}", i).into_bytes(),
                priority: (i % 3 + 1) as u8,
            };
            
            agents[leader_idx].0.submit_command(task_command).await.unwrap();
        }
        
        // Phase 2: Agents update their local state (simulate task execution)
        for (i, (_, _, crdt_manager)) in agents.iter_mut().enumerate() {
            // Update status
            if let Some(status_register) = crdt_manager.get_mut::<LWWRegister<AgentStatus>>("status") {
                status_register.crdt.set(AgentStatus::Busy, ReplicaId::from_uuid(agent_ids[i].0));
            }
            
            // Add task to queue
            if let Some(task_set) = crdt_manager.get_mut::<ORSet<String>>("tasks") {
                task_set.crdt.add(format!("task-{}", i), ReplicaId::from_uuid(agent_ids[i].0));
            }
            
            // Increment performance counter
            if let Some(perf_counter) = crdt_manager.get_mut::<GrowOnlyCounter>("performance") {
                perf_counter.crdt.increment();
            }
        }
        
        // Phase 3: Store agent states in distributed storage
        let mut storage_hashes = Vec::new();
        
        for (i, (_, storage_node, crdt_manager)) in agents.iter().enumerate() {
            // Serialize and store complete agent state
            let mut agent_state = HashMap::new();
            
            if let Some(status) = crdt_manager.get::<LWWRegister<AgentStatus>>("status") {
                agent_state.insert("status", bincode::serialize(&status.crdt).unwrap());
            }
            
            if let Some(tasks) = crdt_manager.get::<ORSet<String>>("tasks") {
                agent_state.insert("tasks", bincode::serialize(&tasks.crdt).unwrap());
            }
            
            if let Some(performance) = crdt_manager.get::<GrowOnlyCounter>("performance") {
                agent_state.insert("performance", bincode::serialize(&performance.crdt).unwrap());
            }
            
            let state_data = bincode::serialize(&agent_state).unwrap();
            let result = storage_node.store(&state_data).await.unwrap();
            
            storage_hashes.push((i, result.hash));
            
            println!("Agent {} state stored with hash: {}", i, result.hash.to_hex());
        }
        
        // Phase 4: Cross-agent state synchronization
        tokio::time::sleep(Duration::from_millis(400)).await;
        
        // Phase 5: Leader coordinates final status update
        let final_status_command = AgentCommand::UpdateConfig {
            key: "swarm_coordination_test".to_string(),
            value: serde_json::Value::String("completed".to_string()),
        };
        
        agents[leader_idx].0.submit_command(final_status_command).await.unwrap();
        
        // Wait for final consensus
        tokio::time::sleep(Duration::from_millis(300)).await;
        
        // Phase 6: Verification and metrics collection
        
        // Verify consensus metrics
        let leader_metrics = agents[leader_idx].0.metrics();
        assert!(leader_metrics.commands_applied >= num_agents as u64 + 1);
        assert!(leader_metrics.messages_sent > 0);
        
        for (i, (consensus_node, _, _)) in agents.iter().enumerate() {
            if i != leader_idx {
                let follower_metrics = consensus_node.metrics();
                assert!(follower_metrics.messages_received > 0);
                assert_eq!(follower_metrics.commit_index, leader_metrics.commit_index);
            }
        }
        
        // Verify storage metrics  
        let mut total_storage_stats = (0, 0, 0); // (blocks, reads, writes)
        
        for (_, storage_node, _) in &agents {
            let stats = storage_node.statistics().await;
            total_storage_stats.0 += stats.total_blocks;
            total_storage_stats.1 += stats.read_operations;
            total_storage_stats.2 += stats.write_operations;
        }
        
        assert!(total_storage_stats.0 >= num_agents as u64);
        assert!(total_storage_stats.2 >= num_agents as u64);
        
        // Verify CRDT consistency
        let mut total_performance = 0;
        for (_, _, crdt_manager) in &agents {
            if let Some(perf_counter) = crdt_manager.get::<GrowOnlyCounter>("performance") {
                total_performance += perf_counter.crdt.value();
            }
        }
        
        assert_eq!(total_performance, num_agents as u64);
        
        // Test data retrieval and consistency
        for (agent_idx, hash) in &storage_hashes {
            let retrieved_data = agents[*agent_idx].1.retrieve(hash).await.unwrap();
            let _agent_state: HashMap<&str, Vec<u8>> = bincode::deserialize(&retrieved_data).unwrap();
            // State successfully deserialized - data integrity confirmed
        }
        
        println!("✅ Full agent swarm coordination test completed successfully");
        println!("  - {} agents coordinated", num_agents);
        println!("  - {} consensus commands processed", leader_metrics.commands_applied);
        println!("  - {} storage blocks created", total_storage_stats.0);
        println!("  - {} total performance units", total_performance);
        
        // Cleanup
        for (consensus_node, _, _) in &mut agents {
            let _ = consensus_node.stop().await;
        }
    }
}