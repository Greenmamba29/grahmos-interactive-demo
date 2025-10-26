use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tokio::time::{interval, Duration};
use warp::Filter;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub peer_id: String,
    pub platform: String, // "ios" or "android"
    pub ip_address: String,
    pub port: u16,
    pub last_seen: u64,
    pub battery_level: Option<u8>,
    pub network_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshStatus {
    pub connected_peers: usize,
    pub mesh_health: f64, // 0.0 to 1.0
    pub partition_count: usize,
    pub message_latency_ms: f64,
}

type PeerRegistry = Arc<Mutex<HashMap<String, PeerInfo>>>;

pub struct P2PCoordinator {
    peers: PeerRegistry,
    discovery_port: u16,
    api_port: u16,
}

impl P2PCoordinator {
    pub fn new(discovery_port: u16, api_port: u16) -> Self {
        Self {
            peers: Arc::new(Mutex::new(HashMap::new())),
            discovery_port,
            api_port,
        }
    }
    
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("🚀 Starting P2P Mesh Coordinator...");
        
        // Start peer discovery service
        let peers_discovery = Arc::clone(&self.peers);
        let discovery_port = self.discovery_port;
        
        tokio::spawn(async move {
            if let Err(e) = Self::run_discovery_service(peers_discovery, discovery_port).await {
                eprintln!("Discovery service error: {}", e);
            }
        });
        
        // Start heartbeat cleanup
        let peers_heartbeat = Arc::clone(&self.peers);
        tokio::spawn(async move {
            Self::run_heartbeat_cleanup(peers_heartbeat).await;
        });
        
        // Start REST API
        self.run_api_service().await
    }
    
    async fn run_discovery_service(peers: PeerRegistry, port: u16) -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
        println!("📡 P2P discovery service listening on port {}", port);
        
        loop {
            match listener.accept().await {
                Ok((mut socket, addr)) => {
                    let peers = Arc::clone(&peers);
                    tokio::spawn(async move {
                        // Handle peer discovery protocol
                        println!("🔗 New peer discovery connection from {}", addr);
                        
                        // In a real implementation, this would handle the P2P discovery protocol
                        // For testing purposes, we'll simulate peer registration
                    });
                },
                Err(e) => {
                    eprintln!("Failed to accept discovery connection: {}", e);
                }
            }
        }
    }
    
    async fn run_heartbeat_cleanup(peers: PeerRegistry) {
        let mut interval = interval(Duration::from_secs(30));
        
        loop {
            interval.tick().await;
            
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            
            let mut peers_guard = peers.lock().unwrap();
            
            // Remove peers that haven't been seen for 2 minutes
            peers_guard.retain(|peer_id, peer| {
                let is_active = now - peer.last_seen < 120;
                if !is_active {
                    println!("🚪 Removing inactive peer: {}", peer_id);
                }
                is_active
            });
        }
    }
    
    async fn run_api_service(&self) -> Result<(), Box<dyn std::error::Error>> {
        let peers = Arc::clone(&self.peers);
        
        let peers_route = warp::path("peers")
            .and(warp::get())
            .and(warp::any().map(move || Arc::clone(&peers)))
            .and_then(Self::get_peers);
            
        let register_route = warp::path("register")
            .and(warp::post())
            .and(warp::body::json())
            .and(warp::any().map(move || Arc::clone(&self.peers)))
            .and_then(Self::register_peer);
            
        let status_route = warp::path("status")
            .and(warp::get())
            .and(warp::any().map(move || Arc::clone(&self.peers)))
            .and_then(Self::get_mesh_status);
        
        let health_route = warp::path("health")
            .and(warp::get())
            .map(|| warp::reply::json(&serde_json::json!({"status": "healthy"})));
        
        let routes = peers_route
            .or(register_route)
            .or(status_route)
            .or(health_route)
            .with(warp::cors().allow_any_origin().allow_headers(vec!["content-type"]).allow_methods(vec!["GET", "POST"]));
        
        println!("🌐 P2P Coordinator API listening on port {}", self.api_port);
        
        warp::serve(routes)
            .run(([0, 0, 0, 0], self.api_port))
            .await;
            
        Ok(())
    }
    
    async fn get_peers(peers: PeerRegistry) -> Result<impl warp::Reply, warp::Rejection> {
        let peers_guard = peers.lock().unwrap();
        let peer_list: Vec<&PeerInfo> = peers_guard.values().collect();
        Ok(warp::reply::json(&peer_list))
    }
    
    async fn register_peer(peer: PeerInfo, peers: PeerRegistry) -> Result<impl warp::Reply, warp::Rejection> {
        println!("📱 Registering peer: {} ({})", peer.peer_id, peer.platform);
        
        let mut peers_guard = peers.lock().unwrap();
        peers_guard.insert(peer.peer_id.clone(), peer);
        
        Ok(warp::reply::json(&serde_json::json!({"status": "registered"})))
    }
    
    async fn get_mesh_status(peers: PeerRegistry) -> Result<impl warp::Reply, warp::Rejection> {
        let peers_guard = peers.lock().unwrap();
        let peer_count = peers_guard.len();
        
        // Calculate mesh health based on peer connectivity
        let mesh_health = if peer_count >= 3 {
            0.9 // Good mesh with 3+ peers
        } else if peer_count >= 2 {
            0.7 // Acceptable with 2 peers
        } else if peer_count >= 1 {
            0.4 // Poor with only 1 peer
        } else {
            0.0 // No mesh
        };
        
        let status = MeshStatus {
            connected_peers: peer_count,
            mesh_health,
            partition_count: if peer_count > 0 { 1 } else { 0 },
            message_latency_ms: 25.0 + (peer_count as f64 * 5.0), // Simulated latency
        };
        
        Ok(warp::reply::json(&status))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let coordinator = P2PCoordinator::new(7777, 8888);
    coordinator.start().await
}
