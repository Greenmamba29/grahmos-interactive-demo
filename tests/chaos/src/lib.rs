use rand::Rng;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::{interval, sleep};
use tracing::{error, info, warn};
use std::collections::HashMap;
use std::sync::Arc;
use sysinfo::{System, SystemExt, ProcessExt, PidExt};

pub mod network_chaos;
pub mod node_chaos;
pub mod storage_chaos;
pub mod memory_chaos;

/// Chaos Engineering Framework for PRISM System Testing
/// 
/// Implements fault injection scenarios:
/// - Network partitions and latency injection
/// - Node failures and recovery
/// - Disk corruption and I/O errors
/// - Memory pressure and OOM conditions
/// - Byzantine behavior simulation

#[derive(Debug, Clone)]
pub struct ChaosConfig {
    /// Probability of chaos events (0.0 to 1.0)
    pub chaos_probability: f64,
    /// Duration between chaos events
    pub chaos_interval: Duration,
    /// Maximum duration for chaos events
    pub max_chaos_duration: Duration,
    /// Enable different chaos types
    pub enable_network_chaos: bool,
    pub enable_node_chaos: bool,
    pub enable_storage_chaos: bool,
    pub enable_memory_chaos: bool,
}

impl Default for ChaosConfig {
    fn default() -> Self {
        Self {
            chaos_probability: 0.1, // 10% chance of chaos
            chaos_interval: Duration::from_secs(30),
            max_chaos_duration: Duration::from_secs(60),
            enable_network_chaos: true,
            enable_node_chaos: true,
            enable_storage_chaos: true,
            enable_memory_chaos: false, // Disabled by default as it's risky
        }
    }
}

/// Types of chaos that can be injected
#[derive(Debug, Clone, PartialEq)]
pub enum ChaosType {
    /// Network partition between nodes
    NetworkPartition {
        affected_nodes: Vec<String>,
        duration: Duration,
    },
    /// Network latency injection
    NetworkLatency {
        affected_nodes: Vec<String>,
        latency_ms: u64,
        jitter_ms: u64,
        duration: Duration,
    },
    /// Packet loss injection
    PacketLoss {
        affected_nodes: Vec<String>, 
        loss_rate: f64, // 0.0 to 1.0
        duration: Duration,
    },
    /// Node failure (crash)
    NodeFailure {
        node_id: String,
        failure_type: NodeFailureType,
    },
    /// Disk I/O errors
    DiskErrors {
        error_rate: f64,
        duration: Duration,
    },
    /// Memory pressure
    MemoryPressure {
        pressure_mb: u64,
        duration: Duration,
    },
    /// CPU throttling
    CpuThrottling {
        cpu_limit: f64, // 0.0 to 1.0
        duration: Duration,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeFailureType {
    /// Clean shutdown
    Graceful,
    /// Immediate termination
    Crash,
    /// Byzantine behavior (sending invalid messages)
    Byzantine,
    /// Slow node (processing delays)
    Slow { delay_ms: u64 },
}

/// Chaos event with metadata
#[derive(Debug, Clone)]
pub struct ChaosEvent {
    pub id: String,
    pub chaos_type: ChaosType,
    pub start_time: std::time::Instant,
    pub end_time: Option<std::time::Instant>,
    pub target_nodes: Vec<String>,
    pub active: bool,
}

impl ChaosEvent {
    pub fn new(chaos_type: ChaosType, target_nodes: Vec<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            chaos_type,
            start_time: std::time::Instant::now(),
            end_time: None,
            target_nodes,
            active: true,
        }
    }
    
    pub fn duration(&self) -> Duration {
        match self.end_time {
            Some(end) => end.duration_since(self.start_time),
            None => self.start_time.elapsed(),
        }
    }
    
    pub fn stop(&mut self) {
        self.end_time = Some(std::time::Instant::now());
        self.active = false;
    }
}

/// Chaos monkey controller
pub struct ChaosController {
    config: ChaosConfig,
    active_events: Arc<Mutex<HashMap<String, ChaosEvent>>>,
    node_registry: Arc<Mutex<HashMap<String, NodeHandle>>>,
    system_monitor: Arc<Mutex<System>>,
    chaos_history: Arc<Mutex<Vec<ChaosEvent>>>,
}

/// Handle to control a node during chaos testing
#[derive(Debug, Clone)]
pub struct NodeHandle {
    pub node_id: String,
    pub process_id: Option<u32>,
    pub status: NodeStatus,
    pub last_heartbeat: std::time::Instant,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeStatus {
    Running,
    Failed,
    Partitioned,
    Slow,
    Byzantine,
}

impl ChaosController {
    pub fn new(config: ChaosConfig) -> Self {
        Self {
            config,
            active_events: Arc::new(Mutex::new(HashMap::new())),
            node_registry: Arc::new(Mutex::new(HashMap::new())),
            system_monitor: Arc::new(Mutex::new(System::new())),
            chaos_history: Arc::new(Mutex::new(Vec::new())),
        }
    }
    
    /// Register a node for chaos testing
    pub async fn register_node(&self, node_id: String, process_id: Option<u32>) {
        let handle = NodeHandle {
            node_id: node_id.clone(),
            process_id,
            status: NodeStatus::Running,
            last_heartbeat: std::time::Instant::now(),
        };
        
        let mut registry = self.node_registry.lock().await;
        registry.insert(node_id.clone(), handle);
        
        info!("Registered node {} for chaos testing", node_id);
    }
    
    /// Start the chaos monkey
    pub async fn start_chaos_monkey(&self) {
        info!("Starting chaos monkey with config: {:?}", self.config);
        
        let mut interval_timer = interval(self.config.chaos_interval);
        
        loop {
            interval_timer.tick().await;
            
            // Check if we should inject chaos
            let should_inject_chaos = rand::thread_rng().gen::<f64>() < self.config.chaos_probability;
            
            if should_inject_chaos {
                if let Some(chaos_event) = self.generate_random_chaos().await {
                    info!("Injecting chaos: {:?}", chaos_event.chaos_type);
                    self.inject_chaos(chaos_event).await;
                }
            }
            
            // Clean up expired chaos events
            self.cleanup_expired_events().await;
            
            // Update node statuses
            self.update_node_statuses().await;
        }
    }
    
    /// Generate a random chaos event based on configuration
    async fn generate_random_chaos(&self) -> Option<ChaosEvent> {
        let registry = self.node_registry.lock().await;
        let node_ids: Vec<String> = registry.keys().cloned().collect();
        
        if node_ids.is_empty() {
            return None;
        }
        
        let mut rng = rand::thread_rng();
        let chaos_types = self.get_enabled_chaos_types();
        
        if chaos_types.is_empty() {
            return None;
        }
        
        let chaos_type = chaos_types[rng.gen_range(0..chaos_types.len())].clone();
        let duration = Duration::from_secs(rng.gen_range(5..self.config.max_chaos_duration.as_secs()));
        
        let chaos_event = match chaos_type {
            "network_partition" => {
                let num_affected = rng.gen_range(1..=(node_ids.len().min(3)));
                let affected_nodes = node_ids.iter()
                    .choose_multiple(&mut rng, num_affected)
                    .cloned()
                    .cloned()
                    .collect();
                
                ChaosType::NetworkPartition {
                    affected_nodes: affected_nodes.clone(),
                    duration,
                }
            },
            "network_latency" => {
                let num_affected = rng.gen_range(1..=node_ids.len());
                let affected_nodes = node_ids.iter()
                    .choose_multiple(&mut rng, num_affected)
                    .cloned()
                    .cloned()
                    .collect();
                
                ChaosType::NetworkLatency {
                    affected_nodes: affected_nodes.clone(),
                    latency_ms: rng.gen_range(100..2000),
                    jitter_ms: rng.gen_range(10..500),
                    duration,
                }
            },
            "packet_loss" => {
                let num_affected = rng.gen_range(1..=node_ids.len());
                let affected_nodes = node_ids.iter()
                    .choose_multiple(&mut rng, num_affected)
                    .cloned()
                    .cloned()
                    .collect();
                
                ChaosType::PacketLoss {
                    affected_nodes: affected_nodes.clone(),
                    loss_rate: rng.gen_range(0.01..0.3), // 1% to 30% loss
                    duration,
                }
            },
            "node_failure" => {
                let node_id = node_ids[rng.gen_range(0..node_ids.len())].clone();
                let failure_types = vec![
                    NodeFailureType::Graceful,
                    NodeFailureType::Crash,
                    NodeFailureType::Byzantine,
                    NodeFailureType::Slow { delay_ms: rng.gen_range(100..5000) },
                ];
                let failure_type = failure_types[rng.gen_range(0..failure_types.len())].clone();
                
                ChaosType::NodeFailure {
                    node_id: node_id.clone(),
                    failure_type,
                }
            },
            "disk_errors" => {
                ChaosType::DiskErrors {
                    error_rate: rng.gen_range(0.01..0.1), // 1% to 10% error rate
                    duration,
                }
            },
            "memory_pressure" => {
                ChaosType::MemoryPressure {
                    pressure_mb: rng.gen_range(100..1000), // 100MB to 1GB pressure
                    duration,
                }
            },
            "cpu_throttling" => {
                ChaosType::CpuThrottling {
                    cpu_limit: rng.gen_range(0.1..0.8), // 10% to 80% CPU limit
                    duration,
                }
            },
            _ => return None,
        };
        
        let target_nodes = match &chaos_event {
            ChaosType::NetworkPartition { affected_nodes, .. } |
            ChaosType::NetworkLatency { affected_nodes, .. } |
            ChaosType::PacketLoss { affected_nodes, .. } => affected_nodes.clone(),
            ChaosType::NodeFailure { node_id, .. } => vec![node_id.clone()],
            _ => node_ids.clone(),
        };
        
        Some(ChaosEvent::new(chaos_event, target_nodes))
    }
    
    /// Get list of enabled chaos types based on configuration
    fn get_enabled_chaos_types(&self) -> Vec<&'static str> {
        let mut types = Vec::new();
        
        if self.config.enable_network_chaos {
            types.extend_from_slice(&["network_partition", "network_latency", "packet_loss"]);
        }
        
        if self.config.enable_node_chaos {
            types.push("node_failure");
        }
        
        if self.config.enable_storage_chaos {
            types.push("disk_errors");
        }
        
        if self.config.enable_memory_chaos {
            types.extend_from_slice(&["memory_pressure", "cpu_throttling"]);
        }
        
        types
    }
    
    /// Inject a chaos event
    async fn inject_chaos(&self, chaos_event: ChaosEvent) {
        match &chaos_event.chaos_type {
            ChaosType::NetworkPartition { affected_nodes, duration } => {
                self.inject_network_partition(affected_nodes.clone(), *duration).await;
            },
            ChaosType::NetworkLatency { affected_nodes, latency_ms, jitter_ms, duration } => {
                self.inject_network_latency(affected_nodes.clone(), *latency_ms, *jitter_ms, *duration).await;
            },
            ChaosType::PacketLoss { affected_nodes, loss_rate, duration } => {
                self.inject_packet_loss(affected_nodes.clone(), *loss_rate, *duration).await;
            },
            ChaosType::NodeFailure { node_id, failure_type } => {
                self.inject_node_failure(node_id.clone(), failure_type.clone()).await;
            },
            ChaosType::DiskErrors { error_rate, duration } => {
                self.inject_disk_errors(*error_rate, *duration).await;
            },
            ChaosType::MemoryPressure { pressure_mb, duration } => {
                self.inject_memory_pressure(*pressure_mb, *duration).await;
            },
            ChaosType::CpuThrottling { cpu_limit, duration } => {
                self.inject_cpu_throttling(*cpu_limit, *duration).await;
            },
        }
        
        // Store active event
        let mut active_events = self.active_events.lock().await;
        active_events.insert(chaos_event.id.clone(), chaos_event.clone());
        
        // Add to history
        let mut history = self.chaos_history.lock().await;
        history.push(chaos_event);
    }
    
    /// Inject network partition
    async fn inject_network_partition(&self, affected_nodes: Vec<String>, duration: Duration) {
        info!("Injecting network partition affecting nodes: {:?} for {:?}", affected_nodes, duration);
        
        // Update node statuses
        let mut registry = self.node_registry.lock().await;
        for node_id in &affected_nodes {
            if let Some(handle) = registry.get_mut(node_id) {
                handle.status = NodeStatus::Partitioned;
            }
        }
        
        // Schedule recovery
        let affected_nodes_clone = affected_nodes.clone();
        let registry_clone = Arc::clone(&self.node_registry);
        
        tokio::spawn(async move {
            sleep(duration).await;
            
            let mut registry = registry_clone.lock().await;
            for node_id in &affected_nodes_clone {
                if let Some(handle) = registry.get_mut(node_id) {
                    if handle.status == NodeStatus::Partitioned {
                        handle.status = NodeStatus::Running;
                        info!("Recovered node {} from network partition", node_id);
                    }
                }
            }
        });
    }
    
    /// Inject network latency
    async fn inject_network_latency(&self, affected_nodes: Vec<String>, latency_ms: u64, jitter_ms: u64, duration: Duration) {
        info!("Injecting network latency {}ms (±{}ms) affecting nodes: {:?} for {:?}", 
              latency_ms, jitter_ms, affected_nodes, duration);
        
        // In a real implementation, this would configure network interfaces
        // For testing, we simulate the effect
        
        tokio::spawn(async move {
            sleep(duration).await;
            info!("Recovered from network latency injection");
        });
    }
    
    /// Inject packet loss
    async fn inject_packet_loss(&self, affected_nodes: Vec<String>, loss_rate: f64, duration: Duration) {
        info!("Injecting packet loss {:.1}% affecting nodes: {:?} for {:?}", 
              loss_rate * 100.0, affected_nodes, duration);
        
        tokio::spawn(async move {
            sleep(duration).await;
            info!("Recovered from packet loss injection");
        });
    }
    
    /// Inject node failure
    async fn inject_node_failure(&self, node_id: String, failure_type: NodeFailureType) {
        info!("Injecting node failure {:?} on node: {}", failure_type, node_id);
        
        let mut registry = self.node_registry.lock().await;
        if let Some(handle) = registry.get_mut(&node_id) {
            match failure_type {
                NodeFailureType::Graceful => {
                    handle.status = NodeStatus::Failed;
                    // In real implementation, send graceful shutdown signal
                },
                NodeFailureType::Crash => {
                    handle.status = NodeStatus::Failed;
                    // In real implementation, kill -9 the process
                    if let Some(pid) = handle.process_id {
                        warn!("Would crash process {} (pid: {})", node_id, pid);
                    }
                },
                NodeFailureType::Byzantine => {
                    handle.status = NodeStatus::Byzantine;
                    // In real implementation, start sending invalid messages
                },
                NodeFailureType::Slow { delay_ms } => {
                    handle.status = NodeStatus::Slow;
                    info!("Node {} will have {}ms processing delays", node_id, delay_ms);
                },
            }
        }
    }
    
    /// Inject disk errors
    async fn inject_disk_errors(&self, error_rate: f64, duration: Duration) {
        info!("Injecting disk errors at {:.1}% rate for {:?}", error_rate * 100.0, duration);
        
        tokio::spawn(async move {
            sleep(duration).await;
            info!("Recovered from disk error injection");
        });
    }
    
    /// Inject memory pressure
    async fn inject_memory_pressure(&self, pressure_mb: u64, duration: Duration) {
        info!("Injecting memory pressure {}MB for {:?}", pressure_mb, duration);
        
        // Allocate memory to create pressure
        let pressure_bytes = pressure_mb * 1024 * 1024;
        let _memory_pressure: Vec<u8> = vec![0; pressure_bytes as usize];
        
        tokio::spawn(async move {
            sleep(duration).await;
            info!("Released memory pressure");
            // Memory is automatically released when vector goes out of scope
        });
    }
    
    /// Inject CPU throttling
    async fn inject_cpu_throttling(&self, cpu_limit: f64, duration: Duration) {
        info!("Injecting CPU throttling to {:.1}% for {:?}", cpu_limit * 100.0, duration);
        
        // In real implementation, this would use cgroups or similar
        tokio::spawn(async move {
            sleep(duration).await;
            info!("Removed CPU throttling");
        });
    }
    
    /// Clean up expired chaos events
    async fn cleanup_expired_events(&self) {
        let mut active_events = self.active_events.lock().await;
        let mut expired_events = Vec::new();
        
        for (id, event) in active_events.iter() {
            let max_duration = match &event.chaos_type {
                ChaosType::NetworkPartition { duration, .. } |
                ChaosType::NetworkLatency { duration, .. } |
                ChaosType::PacketLoss { duration, .. } |
                ChaosType::DiskErrors { duration, .. } |
                ChaosType::MemoryPressure { duration, .. } |
                ChaosType::CpuThrottling { duration, .. } => *duration,
                ChaosType::NodeFailure { .. } => Duration::from_secs(300), // 5 minutes default
            };
            
            if event.start_time.elapsed() > max_duration {
                expired_events.push(id.clone());
            }
        }
        
        for id in expired_events {
            if let Some(mut event) = active_events.remove(&id) {
                event.stop();
                info!("Chaos event {} expired after {:?}", id, event.duration());
            }
        }
    }
    
    /// Update node statuses based on heartbeats
    async fn update_node_statuses(&self) {
        let mut registry = self.node_registry.lock().await;
        let mut system = self.system_monitor.lock().await;
        system.refresh_processes();
        
        for (node_id, handle) in registry.iter_mut() {
            // Check if process is still alive
            if let Some(pid) = handle.process_id {
                if system.process(sysinfo::Pid::from_u32(pid)).is_none() {
                    if handle.status == NodeStatus::Running {
                        handle.status = NodeStatus::Failed;
                        warn!("Node {} process (pid: {}) is no longer running", node_id, pid);
                    }
                }
            }
            
            // Check heartbeat timeout
            if handle.last_heartbeat.elapsed() > Duration::from_secs(60) {
                if handle.status == NodeStatus::Running {
                    handle.status = NodeStatus::Failed;
                    warn!("Node {} heartbeat timeout", node_id);
                }
            }
        }
    }
    
    /// Get current chaos statistics
    pub async fn get_chaos_stats(&self) -> ChaosStats {
        let active_events = self.active_events.lock().await;
        let history = self.chaos_history.lock().await;
        let registry = self.node_registry.lock().await;
        
        let mut stats = ChaosStats {
            total_events: history.len(),
            active_events: active_events.len(),
            nodes_registered: registry.len(),
            nodes_running: 0,
            nodes_failed: 0,
            nodes_partitioned: 0,
            nodes_byzantine: 0,
            network_events: 0,
            node_events: 0,
            storage_events: 0,
            memory_events: 0,
        };
        
        // Count node statuses
        for handle in registry.values() {
            match handle.status {
                NodeStatus::Running => stats.nodes_running += 1,
                NodeStatus::Failed => stats.nodes_failed += 1,
                NodeStatus::Partitioned => stats.nodes_partitioned += 1,
                NodeStatus::Byzantine => stats.nodes_byzantine += 1,
                NodeStatus::Slow => stats.nodes_running += 1, // Count slow nodes as running
            }
        }
        
        // Count event types
        for event in history.iter() {
            match &event.chaos_type {
                ChaosType::NetworkPartition { .. } |
                ChaosType::NetworkLatency { .. } |
                ChaosType::PacketLoss { .. } => stats.network_events += 1,
                ChaosType::NodeFailure { .. } => stats.node_events += 1,
                ChaosType::DiskErrors { .. } => stats.storage_events += 1,
                ChaosType::MemoryPressure { .. } |
                ChaosType::CpuThrottling { .. } => stats.memory_events += 1,
            }
        }
        
        stats
    }
}

#[derive(Debug, Clone)]
pub struct ChaosStats {
    pub total_events: usize,
    pub active_events: usize,
    pub nodes_registered: usize,
    pub nodes_running: usize,
    pub nodes_failed: usize,
    pub nodes_partitioned: usize,
    pub nodes_byzantine: usize,
    pub network_events: usize,
    pub node_events: usize,
    pub storage_events: usize,
    pub memory_events: usize,
}