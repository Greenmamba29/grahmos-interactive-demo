use async_trait::async_trait;
use futures::stream::StreamExt;
use libp2p::{
    gossipsub, identify, kad, mdns, noise, ping, tcp, yamux, Multiaddr, PeerId, Swarm,
    SwarmBuilder, Transport,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::error::Error as StdError;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, error, info, instrument, warn};

pub mod discovery;
pub mod encryption;
pub mod transport;

// Re-export core types
pub use libp2p::{Multiaddr, PeerId};

/// P2P mesh network coordinator for PRISM agent swarm
/// 
/// Implements the networking layer specified in PRISM PRD:
/// - P2P mesh topology with full peer connectivity
/// - mDNS local discovery + DHT cross-site discovery  
/// - Encrypted connections using Noise protocol (ChaCha20-Poly1305)
/// - Gossip-based message propagation with adaptive fanout
/// - Sub-5-second peer discovery target
pub struct P2PMesh {
    /// libp2p swarm for network operations
    swarm: Swarm<PrismBehaviour>,
    
    /// Local peer ID
    local_peer_id: PeerId,
    
    /// Known peers and their connection status
    peers: HashMap<PeerId, PeerInfo>,
    
    /// Site identifier for multi-site deployments
    site_id: Option<String>,
    
    /// Network configuration
    config: NetworkConfig,
    
    /// Channels for network events
    event_tx: broadcast::Sender<NetworkEvent>,
    event_rx: broadcast::Receiver<NetworkEvent>,
    
    /// Command channel for network operations
    command_tx: mpsc::Sender<NetworkCommand>,
    command_rx: Option<mpsc::Receiver<NetworkCommand>>,
}

/// Configuration for P2P networking behavior
#[derive(Debug, Clone)]
pub struct NetworkConfig {
    /// Ports to listen on for incoming connections
    pub listen_ports: Vec<u16>,
    
    /// Enable mDNS for local peer discovery
    pub mdns_enabled: bool,
    
    /// DHT bootstrap nodes for cross-site discovery
    pub bootstrap_nodes: Vec<Multiaddr>,
    
    /// Maximum number of peers to maintain connections with
    pub max_peers: usize,
    
    /// Connection timeout duration
    pub connection_timeout: Duration,
    
    /// Heartbeat interval for peer liveness
    pub heartbeat_interval: Duration,
    
    /// Gossip message fanout (number of peers to forward to)
    pub gossip_fanout: usize,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            listen_ports: vec![30000, 30001], // Default ports from PRD
            mdns_enabled: true,
            bootstrap_nodes: Vec::new(),
            max_peers: 100, // Reasonable default for agent swarm
            connection_timeout: Duration::from_secs(10),
            heartbeat_interval: Duration::from_millis(50), // From PRD: 50ms heartbeat
            gossip_fanout: 3, // Conservative fanout to prevent network flooding
        }
    }
}

/// Information about a peer in the network
#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub peer_id: PeerId,
    pub addresses: Vec<Multiaddr>,
    pub site_id: Option<String>,
    pub agent_role: Option<String>,
    pub last_seen: std::time::Instant,
    pub connection_status: ConnectionStatus,
    pub network_metrics: NetworkMetrics,
}

/// Connection status with a peer
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Failed { reason: String },
}

/// Network performance metrics for a peer
#[derive(Debug, Clone)]
pub struct NetworkMetrics {
    pub latency_ms: Option<f64>,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub messages_sent: u64,
    pub messages_received: u64,
}

impl Default for NetworkMetrics {
    fn default() -> Self {
        Self {
            latency_ms: None,
            bytes_sent: 0,
            bytes_received: 0,
            messages_sent: 0,
            messages_received: 0,
        }
    }
}

/// Events emitted by the P2P network
#[derive(Debug, Clone)]
pub enum NetworkEvent {
    /// New peer discovered and connected
    PeerConnected {
        peer_id: PeerId,
        address: Multiaddr,
    },
    
    /// Peer disconnected
    PeerDisconnected {
        peer_id: PeerId,
        reason: String,
    },
    
    /// Message received from a peer
    MessageReceived {
        peer_id: PeerId,
        message: PrismMessage,
    },
    
    /// Network partition detected
    NetworkPartition {
        isolated_peers: Vec<PeerId>,
    },
    
    /// Peer discovery completed
    DiscoveryCompleted {
        discovered_peers: usize,
        duration: Duration,
    },
}

/// Commands for network operations
#[derive(Debug)]
pub enum NetworkCommand {
    /// Send message to specific peer
    SendMessage {
        peer_id: PeerId,
        message: PrismMessage,
    },
    
    /// Broadcast message to all peers
    BroadcastMessage {
        message: PrismMessage,
        exclude: Vec<PeerId>,
    },
    
    /// Connect to a specific peer
    ConnectToPeer {
        address: Multiaddr,
    },
    
    /// Disconnect from a peer
    DisconnectFromPeer {
        peer_id: PeerId,
    },
    
    /// Update local peer information
    UpdatePeerInfo {
        site_id: Option<String>,
        agent_role: Option<String>,
    },
}

/// Messages exchanged over the P2P network
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PrismMessage {
    /// Agent heartbeat with status information
    Heartbeat {
        agent_id: String,
        timestamp: u64,
        site_id: Option<String>,
        load_metrics: Vec<u8>, // Serialized load metrics
    },
    
    /// Agent joining the network
    Join {
        agent_id: String,
        role: String,
        site_id: Option<String>,
        capabilities: Vec<String>,
    },
    
    /// Agent leaving the network
    Leave {
        agent_id: String,
        reason: String,
    },
    
    /// CRDT state synchronization
    StateSync {
        state_hash: String,
        operations: Vec<u8>, // Serialized CRDT operations
    },
    
    /// Consensus message (PBFT)
    Consensus {
        view_number: u64,
        sequence_number: u64,
        message_type: String, // "prepare", "commit", "view-change"
        payload: Vec<u8>,
    },
    
    /// Custom application message
    Custom {
        message_type: String,
        payload: Vec<u8>,
    },
}

/// libp2p network behavior combining multiple protocols
#[derive(libp2p::NetworkBehaviour)]
pub struct PrismBehaviour {
    /// Gossipsub for message propagation
    pub gossipsub: gossipsub::Behaviour,
    
    /// mDNS for local peer discovery
    pub mdns: mdns::tokio::Behaviour,
    
    /// Kademlia DHT for distributed peer discovery
    pub kademlia: kad::Behaviour<kad::store::MemoryStore>,
    
    /// Identify protocol for peer information exchange
    pub identify: identify::Behaviour,
    
    /// Ping for connection liveness
    pub ping: ping::Behaviour,
}

impl P2PMesh {
    /// Create a new P2P mesh with default configuration
    pub fn new(site_id: Option<String>) -> Result<Self, Box<dyn StdError + Send + Sync>> {
        Self::with_config(NetworkConfig::default(), site_id)
    }
    
    /// Create a new P2P mesh with custom configuration
    #[instrument(skip(config))]
    pub fn with_config(
        config: NetworkConfig,
        site_id: Option<String>,
    ) -> Result<Self, Box<dyn StdError + Send + Sync>> {
        info!("Initializing P2P mesh network");
        
        // Generate local peer identity
        let local_key = libp2p::identity::Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(local_key.public());
        
        info!(peer_id = %local_peer_id, "Generated local peer identity");
        
        // Create transport with noise encryption
        let transport = libp2p::tokio_development_transport(local_key.clone())?;
        
        // Configure gossipsub for message propagation
        let gossipsub_config = gossipsub::ConfigBuilder::default()
            .heartbeat_interval(config.heartbeat_interval)
            .validation_mode(gossipsub::ValidationMode::Strict)
            .fanout_ttl(Duration::from_secs(60))
            .build()
            .map_err(|e| format!("Gossipsub config error: {}", e))?;
        
        let gossipsub = gossipsub::Behaviour::new(
            gossipsub::MessageAuthenticity::Signed(local_key.clone()),
            gossipsub_config,
        )?;
        
        // Configure mDNS for local discovery
        let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), local_peer_id)?;
        
        // Configure Kademlia DHT for distributed discovery
        let store = kad::store::MemoryStore::new(local_peer_id);
        let kademlia = kad::Behaviour::new(local_peer_id, store);
        
        // Configure identify protocol
        let identify = identify::Behaviour::new(identify::Config::new(
            "/prism/1.0.0".to_string(),
            local_key.public(),
        ));
        
        // Configure ping for liveness
        let ping = ping::Behaviour::new(ping::Config::new());
        
        // Combine behaviors
        let behavior = PrismBehaviour {
            gossipsub,
            mdns,
            kademlia,
            identify,
            ping,
        };
        
        // Create swarm
        let mut swarm = SwarmBuilder::with_tokio_executor(transport, behavior, local_peer_id)
            .build();
        
        // Listen on configured ports
        for port in &config.listen_ports {
            let addr = format!("/ip4/0.0.0.0/tcp/{}", port)
                .parse::<Multiaddr>()
                .map_err(|e| format!("Invalid listen address: {}", e))?;
            
            swarm.listen_on(addr.clone())
                .map_err(|e| format!("Failed to listen on {}: {}", addr, e))?;
        }
        
        // Set up event channels
        let (event_tx, event_rx) = broadcast::channel(1024);
        let (command_tx, command_rx) = mpsc::channel(256);
        
        Ok(Self {
            swarm,
            local_peer_id,
            peers: HashMap::new(),
            site_id,
            config,
            event_tx,
            event_rx,
            command_tx,
            command_rx: Some(command_rx),
        })
    }
    
    /// Start the P2P mesh network
    #[instrument(skip(self))]
    pub async fn start(&mut self) -> Result<(), Box<dyn StdError + Send + Sync>> {
        info!("Starting P2P mesh network");
        
        // Subscribe to gossipsub topic for agent messages
        let topic = gossipsub::IdentTopic::new("prism-agents");
        self.swarm.behaviour_mut().gossipsub.subscribe(&topic)?;
        
        // Bootstrap DHT with configured nodes
        for addr in &self.config.bootstrap_nodes.clone() {
            if let Ok(peer_id) = addr.iter().find_map(|protocol| match protocol {
                libp2p::multiaddr::Protocol::P2p(hash) => PeerId::from_multihash(hash).ok(),
                _ => None,
            }) {
                self.swarm.behaviour_mut().kademlia.add_address(&peer_id, addr.clone());
                info!(peer_id = %peer_id, address = %addr, "Added bootstrap node");
            }
        }
        
        // Start bootstrap process
        self.swarm.behaviour_mut().kademlia.bootstrap().ok();
        
        // Start network event loop
        self.start_event_loop().await;
        
        info!("P2P mesh network started successfully");
        Ok(())
    }
    
    /// Send a message to a specific peer
    pub async fn send_message(
        &mut self,
        peer_id: PeerId,
        message: PrismMessage,
    ) -> Result<(), Box<dyn StdError + Send + Sync>> {
        self.command_tx
            .send(NetworkCommand::SendMessage { peer_id, message })
            .await
            .map_err(|e| format!("Failed to send message command: {}", e).into())
    }
    
    /// Broadcast a message to all connected peers
    pub async fn broadcast_message(
        &mut self,
        message: PrismMessage,
    ) -> Result<(), Box<dyn StdError + Send + Sync>> {
        self.command_tx
            .send(NetworkCommand::BroadcastMessage {
                message,
                exclude: Vec::new(),
            })
            .await
            .map_err(|e| format!("Failed to send broadcast command: {}", e).into())
    }
    
    /// Get information about all known peers
    pub fn peers(&self) -> &HashMap<PeerId, PeerInfo> {
        &self.peers
    }
    
    /// Get the local peer ID
    pub fn local_peer_id(&self) -> PeerId {
        self.local_peer_id
    }
    
    /// Get a channel for receiving network events
    pub fn event_receiver(&self) -> broadcast::Receiver<NetworkEvent> {
        self.event_tx.subscribe()
    }
    
    /// Start the main network event processing loop
    async fn start_event_loop(&mut self) {
        let mut command_rx = self.command_rx.take().expect("Command receiver should be available");
        let event_tx = self.event_tx.clone();
        
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    // Handle swarm events
                    event = self.swarm.select_next_some() => {
                        self.handle_swarm_event(event).await;
                    }
                    
                    // Handle network commands
                    command = command_rx.recv() => {
                        match command {
                            Some(cmd) => self.handle_network_command(cmd).await,
                            None => break, // Command channel closed
                        }
                    }
                }
            }
        });
    }
    
    /// Handle events from the libp2p swarm
    async fn handle_swarm_event(&mut self, event: libp2p::swarm::SwarmEvent<PrismBehaviourEvent>) {
        use libp2p::swarm::SwarmEvent;
        
        match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                info!(address = %address, "Listening on address");
            }
            
            SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                info!(peer_id = %peer_id, address = %endpoint.get_remote_address(), "Peer connected");
                
                // Update peer info
                let peer_info = PeerInfo {
                    peer_id,
                    addresses: vec![endpoint.get_remote_address().clone()],
                    site_id: None,
                    agent_role: None,
                    last_seen: std::time::Instant::now(),
                    connection_status: ConnectionStatus::Connected,
                    network_metrics: NetworkMetrics::default(),
                };
                
                self.peers.insert(peer_id, peer_info);
                
                // Notify event listeners
                let _ = self.event_tx.send(NetworkEvent::PeerConnected {
                    peer_id,
                    address: endpoint.get_remote_address().clone(),
                });
            }
            
            SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                warn!(peer_id = %peer_id, "Peer disconnected: {:?}", cause);
                
                // Update connection status
                if let Some(peer_info) = self.peers.get_mut(&peer_id) {
                    peer_info.connection_status = ConnectionStatus::Disconnected;
                }
                
                // Notify event listeners
                let _ = self.event_tx.send(NetworkEvent::PeerDisconnected {
                    peer_id,
                    reason: format!("{:?}", cause),
                });
            }
            
            SwarmEvent::Behaviour(event) => {
                self.handle_behavior_event(event).await;
            }
            
            _ => {}
        }
    }
    
    /// Handle events from specific network behaviors
    async fn handle_behavior_event(&mut self, event: PrismBehaviourEvent) {
        match event {
            PrismBehaviourEvent::Mdns(mdns::Event::Discovered(list)) => {
                for (peer_id, multiaddr) in list {
                    debug!(peer_id = %peer_id, address = %multiaddr, "mDNS peer discovered");
                    
                    // Add to Kademlia routing table
                    self.swarm
                        .behaviour_mut()
                        .kademlia
                        .add_address(&peer_id, multiaddr.clone());
                }
            }
            
            PrismBehaviourEvent::Mdns(mdns::Event::Expired(list)) => {
                for (peer_id, multiaddr) in list {
                    debug!(peer_id = %peer_id, address = %multiaddr, "mDNS peer expired");
                }
            }
            
            PrismBehaviourEvent::Gossipsub(gossipsub::Event::Message { 
                propagation_source: _,
                message_id: _,
                message,
            }) => {
                // Deserialize and handle PRISM message
                if let Ok(prism_message) = rmp_serde::from_slice::<PrismMessage>(&message.data) {
                    let _ = self.event_tx.send(NetworkEvent::MessageReceived {
                        peer_id: message.source.unwrap_or(self.local_peer_id),
                        message: prism_message,
                    });
                }
            }
            
            PrismBehaviourEvent::Kademlia(kad::Event::OutboundQueryProgressed { 
                id: _,
                result: kad::QueryResult::Bootstrap(Ok(kad::BootstrapOk { peer, .. })),
                ..
            }) => {
                debug!(peer_id = %peer, "DHT bootstrap completed");
            }
            
            PrismBehaviourEvent::Ping(ping::Event { 
                peer,
                result: Ok(ping::Success::Ping { rtt }),
                ..
            }) => {
                // Update network metrics
                if let Some(peer_info) = self.peers.get_mut(&peer) {
                    peer_info.network_metrics.latency_ms = Some(rtt.as_millis() as f64);
                    peer_info.last_seen = std::time::Instant::now();
                }
            }
            
            _ => {}
        }
    }
    
    /// Handle network commands
    async fn handle_network_command(&mut self, command: NetworkCommand) {
        match command {
            NetworkCommand::BroadcastMessage { message, exclude } => {
                // Serialize message
                if let Ok(data) = rmp_serde::to_vec(&message) {
                    let topic = gossipsub::IdentTopic::new("prism-agents");
                    if let Err(e) = self.swarm.behaviour_mut().gossipsub.publish(topic, data) {
                        error!(error = %e, "Failed to broadcast message");
                    }
                }
            }
            
            NetworkCommand::ConnectToPeer { address } => {
                if let Err(e) = self.swarm.dial(address.clone()) {
                    error!(address = %address, error = %e, "Failed to dial peer");
                }
            }
            
            NetworkCommand::UpdatePeerInfo { site_id, agent_role } => {
                // Update local site and role information
                // This would be used by the identify protocol
            }
            
            _ => {
                // Handle other commands
            }
        }
    }
}

/// Event types from PrismBehaviour
#[derive(Debug)]
pub enum PrismBehaviourEvent {
    Gossipsub(gossipsub::Event),
    Mdns(mdns::Event),
    Kademlia(kad::Event),
    Identify(identify::Event),
    Ping(ping::Event),
}

impl From<gossipsub::Event> for PrismBehaviourEvent {
    fn from(event: gossipsub::Event) -> Self {
        PrismBehaviourEvent::Gossipsub(event)
    }
}

impl From<mdns::Event> for PrismBehaviourEvent {
    fn from(event: mdns::Event) -> Self {
        PrismBehaviourEvent::Mdns(event)
    }
}

impl From<kad::Event> for PrismBehaviourEvent {
    fn from(event: kad::Event) -> Self {
        PrismBehaviourEvent::Kademlia(event)
    }
}

impl From<identify::Event> for PrismBehaviourEvent {
    fn from(event: identify::Event) -> Self {
        PrismBehaviourEvent::Identify(event)
    }
}

impl From<ping::Event> for PrismBehaviourEvent {
    fn from(event: ping::Event) -> Self {
        PrismBehaviourEvent::Ping(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_test;
    
    #[tokio::test]
    async fn test_p2p_mesh_creation() {
        let mesh = P2PMesh::new(Some("site-a".to_string()));
        assert!(mesh.is_ok());
        
        let mesh = mesh.unwrap();
        assert_eq!(mesh.site_id, Some("site-a".to_string()));
        assert_eq!(mesh.peers.len(), 0);
    }
    
    #[tokio::test]
    async fn test_network_config_defaults() {
        let config = NetworkConfig::default();
        assert_eq!(config.listen_ports, vec![30000, 30001]);
        assert!(config.mdns_enabled);
        assert_eq!(config.gossip_fanout, 3);
        assert_eq!(config.heartbeat_interval, Duration::from_millis(50));
    }
    
    #[test]
    fn test_prism_message_serialization() {
        let message = PrismMessage::Heartbeat {
            agent_id: "test-agent".to_string(),
            timestamp: 12345,
            site_id: Some("site-a".to_string()),
            load_metrics: vec![1, 2, 3],
        };
        
        let serialized = rmp_serde::to_vec(&message).unwrap();
        let deserialized: PrismMessage = rmp_serde::from_slice(&serialized).unwrap();
        
        match deserialized {
            PrismMessage::Heartbeat { agent_id, timestamp, .. } => {
                assert_eq!(agent_id, "test-agent");
                assert_eq!(timestamp, 12345);
            }
            _ => panic!("Wrong message type"),
        }
    }
}