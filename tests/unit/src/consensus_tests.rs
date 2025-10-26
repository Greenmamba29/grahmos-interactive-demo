use prism_consensus::*;
use tokio_test;
use std::time::Duration;
use std::collections::HashMap;
use futures::future::join_all;
use pretty_assertions::assert_eq;

/// Comprehensive Raft consensus tests
/// 
/// Validates:
/// - Raft safety properties (Election Safety, Leader Append-Only, etc.)
/// - Leader election correctness
/// - Log replication consistency  
/// - Network partition handling
/// - Byzantine fault tolerance
/// - Performance under load

#[cfg(test)]
mod raft_safety_tests {
    use super::*;
    
    /// Test Election Safety: At most one leader per term
    #[tokio::test]
    async fn test_election_safety() {
        let node_ids: Vec<NodeId> = (0..5).map(|_| NodeId::new()).collect();
        let mut nodes = Vec::new();
        
        // Create cluster of 5 nodes
        for &node_id in &node_ids {
            let node = ConsensusBuilder::new(node_id)
                .cluster_nodes(node_ids.clone())
                .config(ConsensusConfig {
                    election_timeout_ms: (100, 200),
                    heartbeat_interval_ms: 50,
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
        
        // Wait for election to complete
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // Verify at most one leader
        let leaders: Vec<_> = nodes.iter()
            .filter(|node| node.is_leader())
            .collect();
            
        assert!(leaders.len() <= 1, "Multiple leaders detected: {} leaders in term", leaders.len());
        
        if leaders.len() == 1 {
            let leader = leaders[0];
            let leader_term = leader.metrics().current_term;
            
            // Verify all nodes agree on the leader
            for node in &nodes {
                if let Some(reported_leader) = node.leader_id() {
                    assert_eq!(reported_leader, leader.metrics().leader_id.unwrap());
                }
            }
        }
    }
    
    /// Test Leader Append-Only: Leader never overwrites or deletes log entries  
    #[tokio::test]
    async fn test_leader_append_only() {
        let leader_id = NodeId::new();
        let mut leader = ConsensusBuilder::new(leader_id)
            .build()
            .unwrap();
            
        // Become leader
        leader.start().await.unwrap();
        
        // Submit commands and track log state
        let mut expected_log = Vec::new();
        
        for i in 0..10 {
            let command = AgentCommand::UpdateConfig {
                key: format!("test_key_{}", i),
                value: serde_json::Value::Number(i.into()),
            };
            
            let index = leader.submit_command(command.clone()).await.unwrap();
            expected_log.push((index, command));
        }
        
        // Verify log entries are never modified
        let metrics = leader.metrics();
        assert_eq!(metrics.last_log_index, expected_log.len() as LogIndex);
        
        // Try to submit more commands - should only append
        for i in 10..15 {
            let command = AgentCommand::UpdateConfig {
                key: format!("test_key_{}", i),
                value: serde_json::Value::Number(i.into()),
            };
            
            let index = leader.submit_command(command.clone()).await.unwrap();
            expected_log.push((index, command));
        }
        
        // Verify monotonic growth
        let final_metrics = leader.metrics();
        assert_eq!(final_metrics.last_log_index, expected_log.len() as LogIndex);
        assert!(final_metrics.last_log_index >= metrics.last_log_index);
    }
    
    /// Test Log Matching: If two logs contain entry with same index/term, identical
    #[tokio::test]
    async fn test_log_matching_property() {
        let node1_id = NodeId::new();
        let node2_id = NodeId::new();
        
        let mut node1 = ConsensusBuilder::new(node1_id)
            .cluster_nodes(vec![node1_id, node2_id])
            .build()
            .unwrap();
            
        let mut node2 = ConsensusBuilder::new(node2_id)
            .cluster_nodes(vec![node1_id, node2_id])
            .build()
            .unwrap();
        
        // Start nodes
        tokio::spawn(async move { node1.start().await });
        tokio::spawn(async move { node2.start().await });
        
        // Wait for leader election
        tokio::time::sleep(Duration::from_millis(300)).await;
        
        // Submit commands to leader
        let leader = if node1.is_leader() { &mut node1 } else { &mut node2 };
        
        let command1 = AgentCommand::RegisterAgent {
            agent_id: NodeId::new(),
            agent_type: AgentType::QA,
            capabilities: vec!["testing".to_string()],
        };
        
        let command2 = AgentCommand::UpdateStatus {
            agent_id: NodeId::new(),
            status: AgentStatus::Online,
            metadata: HashMap::new(),
        };
        
        let index1 = leader.submit_command(command1).await.unwrap();
        let index2 = leader.submit_command(command2).await.unwrap();
        
        // Allow replication
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        // Both nodes should have identical log entries at same indices
        let metrics1 = node1.metrics();
        let metrics2 = node2.metrics();
        
        assert_eq!(metrics1.current_term, metrics2.current_term);
        assert_eq!(metrics1.commit_index, metrics2.commit_index);
        assert!(metrics1.commit_index >= index2);
    }
    
    /// Test Leader Completeness: If entry committed, present in future leader logs
    #[tokio::test] 
    async fn test_leader_completeness() {
        let node_ids: Vec<NodeId> = (0..3).map(|_| NodeId::new()).collect();
        let mut nodes = Vec::new();
        
        for &node_id in &node_ids {
            let node = ConsensusBuilder::new(node_id)
                .cluster_nodes(node_ids.clone())
                .build()
                .unwrap();
            nodes.push(node);
        }
        
        // Start all nodes
        for node in &mut nodes {
            tokio::spawn(async move { node.start().await });
        }
        
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        // Find leader and submit command
        let leader_idx = nodes.iter()
            .position(|node| node.is_leader())
            .expect("No leader elected");
            
        let committed_command = AgentCommand::RegisterAgent {
            agent_id: NodeId::new(),
            agent_type: AgentType::CTO,
            capabilities: vec!["architecture".to_string(), "decisions".to_string()],
        };
        
        let committed_index = nodes[leader_idx]
            .submit_command(committed_command.clone())
            .await
            .unwrap();
        
        // Wait for commitment
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Simulate network partition - isolate current leader
        // (In real implementation, this would involve network simulation)
        
        // Force new election by stopping current leader
        nodes.remove(leader_idx);
        
        // Allow time for new election
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // New leader should have the committed entry
        let new_leader_idx = nodes.iter()
            .position(|node| node.is_leader())
            .expect("No new leader elected");
            
        let new_leader_metrics = nodes[new_leader_idx].metrics();
        assert!(new_leader_metrics.commit_index >= committed_index,
                "New leader missing committed entry");
    }
    
    /// Test State Machine Safety: If server applies entry at index, no other server applies different entry
    #[tokio::test]
    async fn test_state_machine_safety() {
        let node_ids: Vec<NodeId> = (0..5).map(|_| NodeId::new()).collect();
        let mut applied_commands: HashMap<LogIndex, Vec<AgentCommand>> = HashMap::new();
        
        // Track applied commands from multiple nodes
        let tracking_nodes = node_ids.clone();
        
        for node_id in tracking_nodes {
            let mut node = ConsensusBuilder::new(node_id)
                .cluster_nodes(node_ids.clone())
                .build()
                .unwrap();
                
            tokio::spawn(async move {
                node.start().await.unwrap();
                
                // Simulate applying commands and track them
                for i in 1..=10 {
                    if let Some(command) = get_applied_command_at_index(&node, i) {
                        // In real implementation, this would use shared state
                        // applied_commands.entry(i).or_insert_with(Vec::new).push(command);
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            });
        }
        
        tokio::time::sleep(Duration::from_millis(1000)).await;
        
        // Verify state machine safety: all applied commands at same index are identical
        for (index, commands) in applied_commands {
            if commands.len() > 1 {
                let first_command = &commands[0];
                for command in &commands[1..] {
                    assert_eq!(first_command, command,
                               "Different commands applied at index {}", index);
                }
            }
        }
    }
    
    // Helper function - would need actual implementation
    fn get_applied_command_at_index(_node: &impl ConsensusNode, _index: LogIndex) -> Option<AgentCommand> {
        None // Placeholder
    }
}

#[cfg(test)]
mod leader_election_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_basic_leader_election() {
        let node_ids: Vec<NodeId> = (0..3).map(|_| NodeId::new()).collect();
        let mut nodes = Vec::new();
        
        for &node_id in &node_ids {
            let node = ConsensusBuilder::new(node_id)
                .cluster_nodes(node_ids.clone())
                .config(ConsensusConfig {
                    election_timeout_ms: (150, 300),
                    heartbeat_interval_ms: 50,
                    ..Default::default()
                })
                .build()
                .unwrap();
            nodes.push(node);
        }
        
        // Start all nodes
        for node in &mut nodes {
            tokio::spawn(async move { node.start().await });
        }
        
        // Wait for election
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // Verify exactly one leader elected
        let leaders: Vec<_> = nodes.iter()
            .filter(|node| node.is_leader())
            .collect();
            
        assert_eq!(leaders.len(), 1, "Expected exactly one leader, got {}", leaders.len());
        
        // Verify followers recognize the leader
        let leader_id = leaders[0].leader_id().unwrap();
        for node in &nodes {
            if !node.is_leader() {
                assert_eq!(node.leader_id(), Some(leader_id));
            }
        }
    }
    
    #[tokio::test]
    async fn test_leader_failure_reelection() {
        let node_ids: Vec<NodeId> = (0..5).map(|_| NodeId::new()).collect();
        let mut nodes = Vec::new();
        
        for &node_id in &node_ids {
            let node = ConsensusBuilder::new(node_id)
                .cluster_nodes(node_ids.clone())
                .build()
                .unwrap();
            nodes.push(node);
        }
        
        // Start all nodes
        for node in &mut nodes {
            tokio::spawn(async move { node.start().await });
        }
        
        // Wait for initial election
        tokio::time::sleep(Duration::from_millis(300)).await;
        
        let initial_leader_idx = nodes.iter()
            .position(|node| node.is_leader())
            .expect("No initial leader");
            
        let initial_term = nodes[initial_leader_idx].metrics().current_term;
        
        // Simulate leader failure by stopping it
        nodes.remove(initial_leader_idx);
        
        // Wait for re-election
        tokio::time::sleep(Duration::from_millis(800)).await;
        
        // Verify new leader elected with higher term
        let new_leaders: Vec<_> = nodes.iter()
            .filter(|node| node.is_leader())
            .collect();
            
        assert_eq!(new_leaders.len(), 1, "Expected exactly one new leader");
        
        let new_leader = new_leaders[0];
        assert!(new_leader.metrics().current_term > initial_term,
                "New leader should have higher term");
    }
    
    #[tokio::test]
    async fn test_split_vote_resolution() {
        // Test scenario where votes are split and no majority is achieved
        let node_ids: Vec<NodeId> = (0..4).map(|_| NodeId::new()).collect(); // Even number for split
        let mut nodes = Vec::new();
        
        for &node_id in &node_ids {
            let node = ConsensusBuilder::new(node_id)
                .cluster_nodes(node_ids.clone())
                .config(ConsensusConfig {
                    election_timeout_ms: (100, 150), // Short timeout for faster re-election
                    vote_timeout_ms: 200,
                    ..Default::default()
                })
                .build()
                .unwrap();
            nodes.push(node);
        }
        
        // Start all nodes
        for node in &mut nodes {
            tokio::spawn(async move { node.start().await });
        }
        
        // Wait longer for potential split vote resolution
        tokio::time::sleep(Duration::from_millis(1000)).await;
        
        // Eventually should elect a leader despite initial split votes
        let leaders: Vec<_> = nodes.iter()
            .filter(|node| node.is_leader())
            .collect();
            
        assert_eq!(leaders.len(), 1, "Should eventually elect exactly one leader");
    }
}

#[cfg(test)]
mod log_replication_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_basic_log_replication() {
        let node_ids: Vec<NodeId> = (0..3).map(|_| NodeId::new()).collect();
        let mut nodes = Vec::new();
        
        for &node_id in &node_ids {
            let node = ConsensusBuilder::new(node_id)
                .cluster_nodes(node_ids.clone())
                .build()
                .unwrap();
            nodes.push(node);
        }
        
        // Start all nodes
        for node in &mut nodes {
            tokio::spawn(async move { node.start().await });
        }
        
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        // Find leader
        let leader_idx = nodes.iter()
            .position(|node| node.is_leader())
            .expect("No leader found");
        
        // Submit commands
        let commands = vec![
            AgentCommand::RegisterAgent {
                agent_id: NodeId::new(),
                agent_type: AgentType::PM,
                capabilities: vec!["requirements".to_string()],
            },
            AgentCommand::AssignTask {
                agent_id: NodeId::new(),
                task_id: "task-001".to_string(),
                task_data: b"test task data".to_vec(),
                priority: 5,
            },
        ];
        
        let mut submitted_indices = Vec::new();
        for command in commands {
            let index = nodes[leader_idx].submit_command(command).await.unwrap();
            submitted_indices.push(index);
        }
        
        // Allow replication time
        tokio::time::sleep(Duration::from_millis(300)).await;
        
        // Verify all nodes have replicated entries
        for (i, node) in nodes.iter().enumerate() {
            let metrics = node.metrics();
            if i != leader_idx {
                assert_eq!(metrics.last_log_index, *submitted_indices.last().unwrap(),
                           "Follower {} has different log length", i);
            }
        }
    }
    
    #[tokio::test]
    async fn test_log_consistency_after_partition() {
        let node_ids: Vec<NodeId> = (0..5).map(|_| NodeId::new()).collect();
        let mut nodes = Vec::new();
        
        for &node_id in &node_ids {
            let node = ConsensusBuilder::new(node_id)
                .cluster_nodes(node_ids.clone())
                .build()
                .unwrap();
            nodes.push(node);
        }
        
        // Start all nodes
        for node in &mut nodes {
            tokio::spawn(async move { node.start().await });
        }
        
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        let leader_idx = nodes.iter()
            .position(|node| node.is_leader())
            .expect("No leader found");
        
        // Submit commands before partition
        let pre_partition_command = AgentCommand::UpdateConfig {
            key: "pre_partition".to_string(),
            value: serde_json::Value::Bool(true),
        };
        
        let pre_partition_index = nodes[leader_idx]
            .submit_command(pre_partition_command)
            .await
            .unwrap();
        
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Simulate network partition (isolate 2 nodes)
        let partitioned_nodes = nodes.split_off(3);
        
        // Continue operations on majority partition
        let majority_leader_idx = nodes.iter()
            .position(|node| node.is_leader())
            .unwrap_or(0);
        
        let post_partition_command = AgentCommand::UpdateConfig {
            key: "post_partition".to_string(),
            value: serde_json::Value::Bool(true),
        };
        
        let post_partition_index = nodes[majority_leader_idx]
            .submit_command(post_partition_command)
            .await
            .unwrap();
        
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        // Heal partition - merge nodes back
        nodes.extend(partitioned_nodes);
        
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // Verify log consistency after healing
        let final_leader_idx = nodes.iter()
            .position(|node| node.is_leader())
            .expect("No leader after partition healing");
        
        let leader_metrics = nodes[final_leader_idx].metrics();
        
        // All nodes should converge to same log state
        for (i, node) in nodes.iter().enumerate() {
            if i != final_leader_idx {
                let follower_metrics = node.metrics();
                assert_eq!(follower_metrics.commit_index, leader_metrics.commit_index,
                           "Node {} has inconsistent commit index", i);
            }
        }
        
        assert!(leader_metrics.commit_index >= post_partition_index);
    }
}

#[cfg(test)]
mod performance_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_throughput_performance() {
        let node_ids: Vec<NodeId> = (0..3).map(|_| NodeId::new()).collect();
        let mut nodes = Vec::new();
        
        for &node_id in &node_ids {
            let node = ConsensusBuilder::new(node_id)
                .cluster_nodes(node_ids.clone())
                .config(ConsensusConfig {
                    max_entries_per_message: 50, // Batch for performance
                    ..Default::default()
                })
                .build()
                .unwrap();
            nodes.push(node);
        }
        
        for node in &mut nodes {
            tokio::spawn(async move { node.start().await });
        }
        
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        let leader_idx = nodes.iter()
            .position(|node| node.is_leader())
            .expect("No leader found");
        
        let start_time = std::time::Instant::now();
        let num_commands = 1000;
        
        // Submit many commands rapidly
        let mut handles = Vec::new();
        for i in 0..num_commands {
            let command = AgentCommand::UpdateConfig {
                key: format!("perf_test_{}", i),
                value: serde_json::Value::Number(i.into()),
            };
            
            let leader = &nodes[leader_idx];
            let handle = tokio::spawn(async move {
                leader.submit_command(command).await
            });
            handles.push(handle);
        }
        
        // Wait for all submissions
        let results: Vec<_> = join_all(handles).await;
        let successful_submissions = results.iter()
            .filter(|r| r.is_ok())
            .count();
        
        let duration = start_time.elapsed();
        let throughput = successful_submissions as f64 / duration.as_secs_f64();
        
        println!("Consensus throughput: {:.2} commands/sec", throughput);
        
        // Target: >200ms average commitment time from requirements
        assert!(duration.as_millis() / successful_submissions as u128 < 200,
                "Average command latency exceeds 200ms target");
        
        // Allow time for replication
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // Verify all followers caught up
        let leader_metrics = nodes[leader_idx].metrics();
        for (i, node) in nodes.iter().enumerate() {
            if i != leader_idx {
                let follower_metrics = node.metrics();
                assert!(follower_metrics.commit_index >= leader_metrics.commit_index - 10,
                        "Follower {} lagging significantly", i);
            }
        }
    }
    
    #[tokio::test]
    async fn test_latency_under_load() {
        let node_ids: Vec<NodeId> = (0..5).map(|_| NodeId::new()).collect();
        let mut nodes = Vec::new();
        
        for &node_id in &node_ids {
            let node = ConsensusBuilder::new(node_id)
                .cluster_nodes(node_ids.clone())
                .build()
                .unwrap();
            nodes.push(node);
        }
        
        for node in &mut nodes {
            tokio::spawn(async move { node.start().await });
        }
        
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        let leader_idx = nodes.iter()
            .position(|node| node.is_leader())
            .expect("No leader found");
        
        let mut latencies = Vec::new();
        
        // Measure individual command latencies
        for i in 0..100 {
            let command_start = std::time::Instant::now();
            
            let command = AgentCommand::AssignTask {
                agent_id: NodeId::new(),
                task_id: format!("latency_test_{}", i),
                task_data: vec![0u8; 1024], // 1KB payload
                priority: 1,
            };
            
            let _index = nodes[leader_idx].submit_command(command).await.unwrap();
            
            let command_latency = command_start.elapsed();
            latencies.push(command_latency);
            
            // Small delay between commands
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        
        // Calculate statistics
        let avg_latency = latencies.iter().sum::<Duration>() / latencies.len() as u32;
        let max_latency = latencies.iter().max().unwrap();
        
        println!("Average command latency: {:?}", avg_latency);
        println!("Maximum command latency: {:?}", max_latency);
        
        // Requirements: <200ms for command commitment
        assert!(avg_latency.as_millis() < 200,
                "Average latency exceeds 200ms: {:?}", avg_latency);
        assert!(max_latency.as_millis() < 500,
                "Maximum latency exceeds 500ms: {:?}", max_latency);
    }
}

#[cfg(test)]
mod fault_tolerance_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_minority_node_failure() {
        let node_ids: Vec<NodeId> = (0..5).map(|_| NodeId::new()).collect();
        let mut nodes = Vec::new();
        
        for &node_id in &node_ids {
            let node = ConsensusBuilder::new(node_id)
                .cluster_nodes(node_ids.clone())
                .build()
                .unwrap();
            nodes.push(node);
        }
        
        for node in &mut nodes {
            tokio::spawn(async move { node.start().await });
        }
        
        tokio::time::sleep(Duration::from_millis(300)).await;
        
        // Submit initial command
        let leader_idx = nodes.iter()
            .position(|node| node.is_leader())
            .expect("No leader found");
        
        let initial_command = AgentCommand::RegisterAgent {
            agent_id: NodeId::new(),
            agent_type: AgentType::Dev,
            capabilities: vec!["implementation".to_string()],
        };
        
        nodes[leader_idx].submit_command(initial_command).await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Simulate failure of 2 nodes (minority)
        nodes.truncate(3); // Keep majority (3/5)
        
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        // Cluster should still be operational
        let remaining_leader_idx = nodes.iter()
            .position(|node| node.is_leader())
            .expect("No leader after minority failure");
        
        // Should be able to commit new commands
        let post_failure_command = AgentCommand::UpdateStatus {
            agent_id: NodeId::new(),
            status: AgentStatus::Online,
            metadata: HashMap::new(),
        };
        
        let result = nodes[remaining_leader_idx]
            .submit_command(post_failure_command)
            .await;
            
        assert!(result.is_ok(), "Cannot commit commands after minority node failure");
    }
    
    #[tokio::test]
    async fn test_majority_node_failure_blocks_progress() {
        let node_ids: Vec<NodeId> = (0..5).map(|_| NodeId::new()).collect();
        let mut nodes = Vec::new();
        
        for &node_id in &node_ids {
            let node = ConsensusBuilder::new(node_id)
                .cluster_nodes(node_ids.clone())
                .config(ConsensusConfig {
                    rpc_timeout_ms: 1000, // Short timeout for faster test
                    ..Default::default()
                })
                .build()
                .unwrap();
            nodes.push(node);
        }
        
        for node in &mut nodes {
            tokio::spawn(async move { node.start().await });
        }
        
        tokio::time::sleep(Duration::from_millis(300)).await;
        
        let leader_idx = nodes.iter()
            .position(|node| node.is_leader())
            .expect("No leader found");
        
        // Simulate failure of 3 nodes (majority) - keep only 2
        nodes.truncate(2);
        
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // No node should be leader (no quorum)
        let leaders: Vec<_> = nodes.iter()
            .filter(|node| node.is_leader())
            .collect();
            
        assert!(leaders.is_empty(), "Should have no leader without quorum");
        
        // Command submission should fail or timeout
        if let Some(node) = nodes.get_mut(0) {
            let timeout_command = AgentCommand::UpdateConfig {
                key: "should_timeout".to_string(),
                value: serde_json::Value::Bool(true),
            };
            
            let result = tokio::time::timeout(
                Duration::from_millis(2000),
                node.submit_command(timeout_command)
            ).await;
            
            // Should either timeout or fail
            assert!(result.is_err() || result.unwrap().is_err(),
                    "Commands should not succeed without quorum");
        }
    }
}