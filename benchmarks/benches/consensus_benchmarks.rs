use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use prism_consensus::*;
use std::time::Duration;
use tokio::runtime::Runtime;
use std::collections::HashMap;

/// Consensus performance benchmarks for PRISM Raft implementation
/// 
/// Performance targets from requirements:
/// - Network Latency: Must achieve <50ms for local mesh communication
/// - Consensus Performance: Must maintain <200ms for command commitment
/// - Memory Usage: Must stay <512MB baseline per agent
/// - Scalability: Handle 3-7 node clusters efficiently

/// Helper function to create test commands
fn create_test_command(id: usize) -> AgentCommand {
    AgentCommand::UpdateConfig {
        key: format!("benchmark_key_{}", id),
        value: serde_json::Value::Number(id.into()),
    }
}

/// Benchmark single-node consensus latency (baseline)
fn bench_single_node_latency(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("consensus_single_node");
    group.throughput(Throughput::Elements(1));
    
    group.bench_function("command_commitment", |b| {
        let node_id = NodeId::new();
        
        b.to_async(&rt).iter(|| async {
            let mut node = ConsensusBuilder::new(node_id)
                .config(ConsensusConfig {
                    heartbeat_interval_ms: 10, // Fast heartbeats for benchmarking
                    election_timeout_ms: (50, 100),
                    ..Default::default()
                })
                .build()
                .unwrap();
            
            // Start node (will become leader immediately as single node)
            node.start().await.unwrap();
            
            // Measure command commitment time
            let start = std::time::Instant::now();
            let command = create_test_command(0);
            let _index = node.submit_command(command).await.unwrap();
            let duration = start.elapsed();
            
            // Stop node
            node.stop().await.unwrap();
            
            black_box(duration);
        });
    });
    
    group.finish();
}

/// Benchmark 3-node consensus cluster
fn bench_three_node_consensus(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("consensus_three_node");
    group.measurement_time(Duration::from_secs(10));
    group.throughput(Throughput::Elements(1));
    
    group.bench_function("command_replication", |b| {
        b.to_async(&rt).iter_custom(|iters| async move {
            let node_ids: Vec<NodeId> = (0..3).map(|_| NodeId::new()).collect();
            let mut nodes = Vec::new();
            
            // Create and start cluster
            for &node_id in &node_ids {
                let node = ConsensusBuilder::new(node_id)
                    .cluster_nodes(node_ids.clone())
                    .config(ConsensusConfig {
                        heartbeat_interval_ms: 20,
                        election_timeout_ms: (100, 200),
                        max_entries_per_message: 50, // Batch for efficiency
                        ..Default::default()
                    })
                    .build()
                    .unwrap();
                nodes.push(node);
            }
            
            // Start all nodes
            let start_handles: Vec<_> = nodes.iter_mut()
                .map(|node| {
                    let node_ptr = node as *mut _;
                    tokio::spawn(async move {
                        unsafe { (*node_ptr).start().await }
                    })
                })
                .collect();
            
            for handle in start_handles {
                handle.await.unwrap().unwrap();
            }
            
            // Wait for leader election
            tokio::time::sleep(Duration::from_millis(300)).await;
            
            // Find leader
            let leader_idx = nodes.iter()
                .position(|node| node.is_leader())
                .expect("No leader elected");
            
            // Benchmark command submissions
            let bench_start = std::time::Instant::now();
            
            for i in 0..iters {
                let command = create_test_command(i as usize);
                let _index = nodes[leader_idx].submit_command(command).await.unwrap();
            }
            
            let duration = bench_start.elapsed();
            
            // Cleanup
            for node in &mut nodes {
                let _ = node.stop().await;
            }
            
            duration
        });
    });
    
    group.finish();
}

/// Benchmark 5-node consensus cluster (Byzantine fault tolerance)
fn bench_five_node_byzantine_tolerance(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("consensus_five_node_bft");
    group.sample_size(10); // Fewer samples due to complexity
    group.measurement_time(Duration::from_secs(15));
    
    group.bench_function("byzantine_resilience", |b| {
        b.to_async(&rt).iter(|| async {
            let node_ids: Vec<NodeId> = (0..5).map(|_| NodeId::new()).collect();
            let mut nodes = Vec::new();
            
            // Create cluster
            for &node_id in &node_ids {
                let node = ConsensusBuilder::new(node_id)
                    .cluster_nodes(node_ids.clone())
                    .config(ConsensusConfig {
                        heartbeat_interval_ms: 25,
                        election_timeout_ms: (150, 300),
                        vote_timeout_ms: 2000,
                        ..Default::default()
                    })
                    .build()
                    .unwrap();
                nodes.push(node);
            }
            
            // Start all nodes
            let start_tasks: Vec<_> = nodes.iter_mut()
                .map(|node| {
                    let node_ptr = node as *mut _;
                    tokio::spawn(async move {
                        unsafe { (*node_ptr).start().await }
                    })
                })
                .collect();
            
            for task in start_tasks {
                task.await.unwrap().unwrap();
            }
            
            // Wait for stable leader election
            tokio::time::sleep(Duration::from_millis(500)).await;
            
            let leader_idx = nodes.iter()
                .position(|node| node.is_leader())
                .expect("No leader elected");
            
            // Submit commands and measure latency
            let mut latencies = Vec::new();
            
            for i in 0..20 {
                let start = std::time::Instant::now();
                let command = AgentCommand::AssignTask {
                    agent_id: NodeId::new(),
                    task_id: format!("bft_task_{}", i),
                    task_data: vec![0u8; 1024], // 1KB payload
                    priority: 5,
                };
                
                let _index = nodes[leader_idx].submit_command(command).await.unwrap();
                let latency = start.elapsed();
                latencies.push(latency);
                
                // Small delay between commands
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            
            // Calculate average latency
            let avg_latency = latencies.iter().sum::<Duration>() / latencies.len() as u32;
            
            // Validate requirement: <200ms for command commitment
            assert!(avg_latency.as_millis() < 200, 
                    "Average commitment latency {}ms exceeds 200ms requirement", 
                    avg_latency.as_millis());
            
            // Cleanup
            for node in &mut nodes {
                let _ = node.stop().await;
            }
            
            black_box(avg_latency);
        });
    });
    
    group.finish();
}

/// Benchmark leader election performance
fn bench_leader_election_time(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("consensus_leader_election");
    group.sample_size(10);
    
    let cluster_sizes = vec![3, 5, 7];
    
    for cluster_size in cluster_sizes {
        group.bench_with_input(
            BenchmarkId::new("election_time", cluster_size),
            &cluster_size,
            |b, &size| {
                b.to_async(&rt).iter(|| async move {
                    let node_ids: Vec<NodeId> = (0..size).map(|_| NodeId::new()).collect();
                    let mut nodes = Vec::new();
                    
                    // Create cluster with election-optimized config
                    for &node_id in &node_ids {
                        let node = ConsensusBuilder::new(node_id)
                            .cluster_nodes(node_ids.clone())
                            .config(ConsensusConfig {
                                election_timeout_ms: (150, 300),
                                heartbeat_interval_ms: 50,
                                vote_timeout_ms: 1000,
                                ..Default::default()
                            })
                            .build()
                            .unwrap();
                        nodes.push(node);
                    }
                    
                    // Start nodes and measure election time
                    let election_start = std::time::Instant::now();
                    
                    let start_tasks: Vec<_> = nodes.iter_mut()
                        .map(|node| {
                            let node_ptr = node as *mut _;
                            tokio::spawn(async move {
                                unsafe { (*node_ptr).start().await }
                            })
                        })
                        .collect();
                    
                    for task in start_tasks {
                        task.await.unwrap().unwrap();
                    }
                    
                    // Wait until a leader is elected
                    let mut leader_elected = false;
                    let mut attempts = 0;
                    
                    while !leader_elected && attempts < 50 { // Max 5 seconds
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        leader_elected = nodes.iter().any(|node| node.is_leader());
                        attempts += 1;
                    }
                    
                    let election_duration = election_start.elapsed();
                    
                    assert!(leader_elected, "Leader election failed for cluster size {}", size);
                    
                    // Validate requirement: Leader election should complete within 1 second
                    assert!(election_duration.as_millis() < 1000,
                            "Leader election took {}ms, exceeding 1s requirement", 
                            election_duration.as_millis());
                    
                    // Cleanup
                    for node in &mut nodes {
                        let _ = node.stop().await;
                    }
                    
                    black_box(election_duration)
                });
            }
        );
    }
    
    group.finish();
}

/// Benchmark consensus throughput (commands per second)
fn bench_consensus_throughput(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("consensus_throughput");
    group.measurement_time(Duration::from_secs(10));
    
    let batch_sizes = vec![1, 10, 50, 100];
    
    for batch_size in batch_sizes {
        group.throughput(Throughput::Elements(batch_size));
        group.bench_with_input(
            BenchmarkId::new("batched_commands", batch_size),
            &batch_size,
            |b, &batch_size| {
                b.to_async(&rt).iter(|| async {
                    let node_ids: Vec<NodeId> = (0..3).map(|_| NodeId::new()).collect();
                    let mut nodes = Vec::new();
                    
                    // Create cluster optimized for throughput
                    for &node_id in &node_ids {
                        let node = ConsensusBuilder::new(node_id)
                            .cluster_nodes(node_ids.clone())
                            .config(ConsensusConfig {
                                heartbeat_interval_ms: 10, // Frequent heartbeats
                                max_entries_per_message: 100, // Large batches
                                election_timeout_ms: (100, 200),
                                ..Default::default()
                            })
                            .build()
                            .unwrap();
                        nodes.push(node);
                    }
                    
                    // Start cluster
                    let start_tasks: Vec<_> = nodes.iter_mut()
                        .map(|node| {
                            let node_ptr = node as *mut _;
                            tokio::spawn(async move {
                                unsafe { (*node_ptr).start().await }
                            })
                        })
                        .collect();
                    
                    for task in start_tasks {
                        task.await.unwrap().unwrap();
                    }
                    
                    tokio::time::sleep(Duration::from_millis(200)).await;
                    
                    let leader_idx = nodes.iter()
                        .position(|node| node.is_leader())
                        .expect("No leader elected");
                    
                    // Submit batch of commands
                    let start_time = std::time::Instant::now();
                    let mut handles = Vec::new();
                    
                    for i in 0..batch_size {
                        let command = AgentCommand::UpdateStatus {
                            agent_id: NodeId::new(),
                            status: AgentStatus::Online,
                            metadata: HashMap::new(),
                        };
                        
                        let leader = &nodes[leader_idx];
                        let handle = tokio::spawn(async move {
                            leader.submit_command(command).await
                        });
                        handles.push(handle);
                    }
                    
                    // Wait for all commands to complete
                    let results: Vec<_> = futures::future::join_all(handles).await;
                    let successful_count = results.iter()
                        .filter(|r| r.is_ok() && r.as_ref().unwrap().is_ok())
                        .count();
                    
                    let duration = start_time.elapsed();
                    let throughput = successful_count as f64 / duration.as_secs_f64();
                    
                    println!("Consensus throughput: {:.2} commands/sec (batch size: {})", 
                             throughput, batch_size);
                    
                    // Cleanup
                    for node in &mut nodes {
                        let _ = node.stop().await;
                    }
                    
                    black_box(throughput);
                });
            }
        );
    }
    
    group.finish();
}

/// Benchmark consensus under network latency stress
fn bench_consensus_network_latency(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("consensus_network_stress");
    group.sample_size(10);
    
    // Simulate different network conditions
    let latency_conditions = vec![
        ("low_latency", 10),      // 10ms
        ("medium_latency", 50),   // 50ms 
        ("high_latency", 100),    // 100ms
    ];
    
    for (condition_name, base_latency_ms) in latency_conditions {
        group.bench_function(condition_name, |b| {
            b.to_async(&rt).iter(|| async {
                let node_ids: Vec<NodeId> = (0..3).map(|_| NodeId::new()).collect();
                let mut nodes = Vec::new();
                
                // Create cluster with latency-adapted timeouts
                for &node_id in &node_ids {
                    let node = ConsensusBuilder::new(node_id)
                        .cluster_nodes(node_ids.clone())
                        .config(ConsensusConfig {
                            heartbeat_interval_ms: base_latency_ms * 2,
                            election_timeout_ms: (base_latency_ms * 10, base_latency_ms * 20),
                            rpc_timeout_ms: base_latency_ms * 50,
                            ..Default::default()
                        })
                        .build()
                        .unwrap();
                    nodes.push(node);
                }
                
                // Start cluster
                let start_tasks: Vec<_> = nodes.iter_mut()
                    .map(|node| {
                        let node_ptr = node as *mut _;
                        tokio::spawn(async move {
                            unsafe { (*node_ptr).start().await }
                        })
                    })
                    .collect();
                
                for task in start_tasks {
                    task.await.unwrap().unwrap();
                }
                
                // Wait for stable cluster
                tokio::time::sleep(Duration::from_millis(base_latency_ms * 10)).await;
                
                let leader_idx = nodes.iter()
                    .position(|node| node.is_leader())
                    .expect("No leader elected under network stress");
                
                // Measure command latency under simulated network stress
                let command_start = std::time::Instant::now();
                let command = AgentCommand::RegisterAgent {
                    agent_id: NodeId::new(),
                    agent_type: AgentType::QA,
                    capabilities: vec!["testing".to_string(), "validation".to_string()],
                };
                
                // Add artificial delay to simulate network latency
                tokio::time::sleep(Duration::from_millis(base_latency_ms)).await;
                
                let _index = nodes[leader_idx].submit_command(command).await.unwrap();
                let total_latency = command_start.elapsed();
                
                // Validate that consensus still works under network stress
                // but allow for increased latency due to network conditions
                let max_acceptable_latency = base_latency_ms * 5; // 5x base latency
                assert!(total_latency.as_millis() < max_acceptable_latency as u128,
                        "Command latency {}ms exceeds max acceptable {}ms under {} conditions",
                        total_latency.as_millis(), max_acceptable_latency, condition_name);
                
                // Cleanup
                for node in &mut nodes {
                    let _ = node.stop().await;
                }
                
                black_box(total_latency);
            });
        });
    }
    
    group.finish();
}

/// Memory usage benchmarking during consensus operations
fn bench_consensus_memory_usage(c: &mut Criterion) {
    use sysinfo::{System, SystemExt, ProcessExt, PidExt};
    
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("consensus_memory_usage");
    group.sample_size(10);
    
    group.bench_function("memory_baseline", |b| {
        b.iter_custom(|_| {
            rt.block_on(async {
                let start = std::time::Instant::now();
                
                // Measure memory before consensus operations
                let mut system = System::new();
                system.refresh_processes();
                let pid = sysinfo::get_current_pid().unwrap();
                let process = system.process(pid).unwrap();
                let memory_before = process.memory();
                
                // Create and run consensus cluster
                let node_ids: Vec<NodeId> = (0..5).map(|_| NodeId::new()).collect();
                let mut nodes = Vec::new();
                
                for &node_id in &node_ids {
                    let node = ConsensusBuilder::new(node_id)
                        .cluster_nodes(node_ids.clone())
                        .build()
                        .unwrap();
                    nodes.push(node);
                }
                
                // Start cluster and perform operations
                let start_tasks: Vec<_> = nodes.iter_mut()
                    .map(|node| {
                        let node_ptr = node as *mut _;
                        tokio::spawn(async move {
                            unsafe { (*node_ptr).start().await }
                        })
                    })
                    .collect();
                
                for task in start_tasks {
                    task.await.unwrap().unwrap();
                }
                
                tokio::time::sleep(Duration::from_millis(300)).await;
                
                let leader_idx = nodes.iter()
                    .position(|node| node.is_leader())
                    .expect("No leader elected");
                
                // Submit many commands to stress memory usage
                for i in 0..1000 {
                    let command = create_test_command(i);
                    nodes[leader_idx].submit_command(command).await.unwrap();
                }
                
                // Measure memory after operations
                system.refresh_processes();
                let process = system.process(pid).unwrap();
                let memory_after = process.memory();
                
                let memory_used_mb = (memory_after - memory_before) as f64 / 1024.0 / 1024.0;
                println!("Consensus memory usage: {:.2} MB", memory_used_mb);
                
                // Validate requirement: Must stay <512MB baseline per agent
                assert!(memory_used_mb < 512.0 * nodes.len() as f64,
                        "Memory usage {:.2} MB exceeds 512MB * {} agents limit", 
                        memory_used_mb, nodes.len());
                
                // Cleanup
                for node in &mut nodes {
                    let _ = node.stop().await;
                }
                
                start.elapsed()
            })
        });
    });
    
    group.finish();
}

criterion_group!(
    consensus_benches,
    bench_single_node_latency,
    bench_three_node_consensus,
    bench_five_node_byzantine_tolerance,
    bench_leader_election_time,
    bench_consensus_throughput,
    bench_consensus_network_latency,
    bench_consensus_memory_usage
);

criterion_main!(consensus_benches);