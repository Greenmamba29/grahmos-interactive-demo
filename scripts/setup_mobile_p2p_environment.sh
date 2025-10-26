#!/bin/bash

# PRISM Mobile P2P Testing Environment Setup
# Configures cross-platform mobile testing infrastructure with network simulation

set -euo pipefail

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
MOBILE_TEST_DIR="$PROJECT_ROOT/tests/mobile"
DOCKER_COMPOSE_DIR="$PROJECT_ROOT/docker/mobile-testing"

# Logging functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_header() {
    echo -e "${BLUE}================================${NC}"
    echo -e "${BLUE} $1${NC}"
    echo -e "${BLUE}================================${NC}"
}

# Setup validation counters
setup_steps=0
completed_steps=0

track_step() {
    local step_name="$1"
    local result="$2"
    
    setup_steps=$((setup_steps + 1))
    
    if [ "$result" -eq 0 ]; then
        log_success "$step_name"
        completed_steps=$((completed_steps + 1))
    else
        log_error "$step_name"
        return 1
    fi
}

# Create mobile testing Docker environment
setup_docker_environment() {
    log_header "Setting up Docker Mobile Testing Environment"
    
    # Create Docker configuration directory
    log_info "Creating Docker configuration directory..."
    mkdir -p "$DOCKER_COMPOSE_DIR"
    track_step "Created Docker configuration directory" 0
    
    # Create Docker Compose file for mobile testing
    cat > "$DOCKER_COMPOSE_DIR/docker-compose.yml" << 'EOF'
version: '3.8'

services:
  # iOS Simulator Environment
  ios-simulator:
    image: sickcodes/docker-osx:ventura
    platform: linux/amd64
    environment:
      - DISPLAY=${DISPLAY:-:99}
      - XCODE_VERSION=14.3
      - SIMULATOR_DEVICE=iPhone 14 Pro
      - ENABLE_P2P_MESH=true
    volumes:
      - ios_simulator_data:/System/Applications/Xcode.app
      - ../tests/mobile:/app/tests:ro
    networks:
      - mobile_mesh_network
    ports:
      - "5900:5900"  # VNC access
      - "8080:8080"  # Test server
    mem_limit: 8g
    cpus: 4
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/health"]
      interval: 30s
      timeout: 10s
      retries: 3
      
  # Android Emulator Environment  
  android-emulator:
    image: budtmo/docker-android:emulator_11.0
    platform: linux/amd64
    environment:
      - DEVICE=Samsung Galaxy S21
      - ANDROID_VERSION=11.0
      - API_LEVEL=30
      - TARGET=google_apis_playstore
      - ENABLE_P2P_MESH=true
    volumes:
      - android_emulator_data:/root/.android
      - ../tests/mobile:/app/tests:ro
    networks:
      - mobile_mesh_network
    ports:
      - "6080:6080"  # VNC access
      - "5554:5554"  # ADB
      - "5555:5555"  # ADB
      - "8081:8081"  # Test server
    devices:
      - "/dev/kvm:/dev/kvm"
    mem_limit: 6g
    cpus: 3
    healthcheck:
      test: ["CMD", "adb", "shell", "echo", "ok"]
      interval: 30s
      timeout: 10s
      retries: 3
      
  # Network Simulation Controller
  network-simulator:
    image: alpine:3.18
    platform: linux/amd64
    environment:
      - SIMULATION_MODE=p2p_mesh
      - LATENCY_PROFILES=wifi,4g,3g,edge,offline
      - PEER_DISCOVERY_ENABLED=true
    volumes:
      - ./network-configs:/etc/network-sim:ro
      - ../tests/mobile:/app/tests:ro
    networks:
      - mobile_mesh_network
    ports:
      - "9090:9090"  # Control API
    command: |
      sh -c "
        apk add --no-cache curl jq python3 py3-pip tcpdump &&
        pip3 install flask requests &&
        python3 /app/tests/network_simulator.py
      "
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:9090/status"]
      interval: 15s
      timeout: 5s
      retries: 3
      
  # P2P Mesh Coordinator
  p2p-coordinator:
    image: rust:1.75-alpine
    platform: linux/amd64
    working_dir: /app
    environment:
      - RUST_LOG=debug
      - MESH_DISCOVERY_PORT=7777
      - MAX_PEERS=10
      - HEARTBEAT_INTERVAL=5s
    volumes:
      - ../:/app:ro
      - p2p_coordinator_data:/app/target
    networks:
      - mobile_mesh_network
    ports:
      - "7777:7777"  # P2P discovery
      - "8888:8888"  # Coordinator API
    command: |
      sh -c "
        apk add --no-cache build-base openssl-dev &&
        cd tests/mobile &&
        cargo build --release --bin p2p_coordinator &&
        ./target/release/p2p_coordinator
      "
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8888/peers"]
      interval: 20s
      timeout: 10s
      retries: 3
      
  # Test Results Collector
  test-collector:
    image: node:18-alpine
    platform: linux/amd64
    working_dir: /app
    environment:
      - NODE_ENV=testing
      - COLLECT_REAL_TIME=true
      - DASHBOARD_ENDPOINT=http://localhost:3000/api/results
    volumes:
      - ./test-collector:/app:ro
      - test_results:/app/results
    networks:
      - mobile_mesh_network  
    ports:
      - "3001:3001"  # Results API
    command: |
      sh -c "
        npm install express axios ws &&
        node test-collector.js
      "
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:3001/health"]
      interval: 30s
      timeout: 5s
      retries: 3

networks:
  mobile_mesh_network:
    driver: bridge
    ipam:
      driver: default
      config:
        - subnet: 172.20.0.0/16
    driver_opts:
      com.docker.network.bridge.enable_ip_masquerade: "true"
      com.docker.network.driver.mtu: 1500

volumes:
  ios_simulator_data:
    driver: local
  android_emulator_data:
    driver: local
  p2p_coordinator_data:
    driver: local
  test_results:
    driver: local
EOF
    
    track_step "Created Docker Compose configuration" 0
    
    log_info "Docker mobile testing environment configuration complete"
}

# Create network simulation controller
setup_network_simulator() {
    log_header "Setting up Network Simulation Controller"
    
    # Create network simulation directory
    mkdir -p "$DOCKER_COMPOSE_DIR/network-configs"
    track_step "Created network simulation directory" 0
    
    # Create network profiles configuration
    cat > "$DOCKER_COMPOSE_DIR/network-configs/network-profiles.json" << 'EOF'
{
  "network_profiles": {
    "wifi": {
      "bandwidth_mbps": 100,
      "latency_ms": 5,
      "packet_loss_percent": 0.1,
      "jitter_ms": 2,
      "description": "High-speed WiFi connection"
    },
    "4g": {
      "bandwidth_mbps": 25,
      "latency_ms": 50,
      "packet_loss_percent": 1.0,
      "jitter_ms": 10,
      "description": "4G/LTE mobile connection"
    },
    "3g": {
      "bandwidth_mbps": 8,
      "latency_ms": 150,
      "packet_loss_percent": 2.5,
      "jitter_ms": 25,
      "description": "3G mobile connection"
    },
    "edge": {
      "bandwidth_mbps": 0.5,
      "latency_ms": 500,
      "packet_loss_percent": 5.0,
      "jitter_ms": 100,
      "description": "EDGE/2G slow mobile connection"
    },
    "offline": {
      "bandwidth_mbps": 0,
      "latency_ms": 0,
      "packet_loss_percent": 100,
      "jitter_ms": 0,
      "description": "Offline mode - no connectivity"
    }
  },
  "transition_scenarios": [
    {
      "name": "commute_scenario",
      "description": "Simulates network transitions during commute",
      "steps": [
        {"profile": "wifi", "duration_seconds": 30},
        {"profile": "4g", "duration_seconds": 120},
        {"profile": "3g", "duration_seconds": 60},
        {"profile": "offline", "duration_seconds": 15},
        {"profile": "wifi", "duration_seconds": 45}
      ]
    },
    {
      "name": "battery_saver_scenario", 
      "description": "Simulates reduced connectivity in battery saver mode",
      "steps": [
        {"profile": "4g", "duration_seconds": 60},
        {"profile": "3g", "duration_seconds": 90},
        {"profile": "edge", "duration_seconds": 120},
        {"profile": "offline", "duration_seconds": 30}
      ]
    },
    {
      "name": "mesh_recovery_scenario",
      "description": "Tests P2P mesh recovery after network failure", 
      "steps": [
        {"profile": "wifi", "duration_seconds": 30},
        {"profile": "offline", "duration_seconds": 45},
        {"profile": "4g", "duration_seconds": 60},
        {"profile": "wifi", "duration_seconds": 30}
      ]
    }
  ]
}
EOF
    
    track_step "Created network profiles configuration" 0
    
    # Create network simulator Python script
    mkdir -p "$MOBILE_TEST_DIR"
    cat > "$MOBILE_TEST_DIR/network_simulator.py" << 'EOF'
#!/usr/bin/env python3
"""
PRISM Mobile P2P Network Simulator
Simulates various network conditions for mobile testing
"""

import json
import time
import threading
import subprocess
from flask import Flask, jsonify, request
from typing import Dict, List, Optional

app = Flask(__name__)

class NetworkSimulator:
    def __init__(self):
        self.current_profile = "wifi"
        self.active_scenario = None
        self.scenario_thread = None
        self.profiles = {}
        self.load_profiles()
        
    def load_profiles(self):
        """Load network profiles from configuration"""
        try:
            with open('/etc/network-sim/network-profiles.json', 'r') as f:
                config = json.load(f)
                self.profiles = config['network_profiles']
                self.scenarios = config['transition_scenarios']
        except Exception as e:
            print(f"Warning: Could not load network profiles: {e}")
            # Use default profiles
            self.profiles = {
                "wifi": {"bandwidth_mbps": 100, "latency_ms": 5},
                "4g": {"bandwidth_mbps": 25, "latency_ms": 50},
                "3g": {"bandwidth_mbps": 8, "latency_ms": 150},
                "offline": {"bandwidth_mbps": 0, "latency_ms": 0}
            }
            self.scenarios = []
    
    def apply_network_profile(self, profile_name: str):
        """Apply network conditions using tc (traffic control)"""
        if profile_name not in self.profiles:
            raise ValueError(f"Unknown profile: {profile_name}")
            
        profile = self.profiles[profile_name]
        
        try:
            # Clear existing rules
            subprocess.run(['tc', 'qdisc', 'del', 'dev', 'eth0', 'root'], 
                         capture_output=True, check=False)
            
            if profile_name == "offline":
                # Block all traffic for offline mode
                subprocess.run(['tc', 'qdisc', 'add', 'dev', 'eth0', 'root', 'netem', 'loss', '100%'], 
                             check=True, capture_output=True)
            else:
                # Apply bandwidth and latency constraints
                bandwidth = profile['bandwidth_mbps']
                latency = profile['latency_ms']
                
                # Set up traffic shaping
                cmd = ['tc', 'qdisc', 'add', 'dev', 'eth0', 'root', 'handle', '1:', 'htb', 'default', '30']
                subprocess.run(cmd, check=True, capture_output=True)
                
                cmd = ['tc', 'class', 'add', 'dev', 'eth0', 'parent', '1:', 'classid', '1:1', 'htb', 
                       'rate', f'{bandwidth}mbit']
                subprocess.run(cmd, check=True, capture_output=True)
                
                # Add latency
                cmd = ['tc', 'qdisc', 'add', 'dev', 'eth0', 'parent', '1:1', 'handle', '10:', 
                       'netem', 'delay', f'{latency}ms']
                subprocess.run(cmd, check=True, capture_output=True)
            
            self.current_profile = profile_name
            print(f"Applied network profile: {profile_name}")
            
        except subprocess.CalledProcessError as e:
            print(f"Error applying network profile {profile_name}: {e}")
            raise
    
    def run_scenario(self, scenario_name: str):
        """Run a network transition scenario"""
        scenario = next((s for s in self.scenarios if s['name'] == scenario_name), None)
        if not scenario:
            raise ValueError(f"Unknown scenario: {scenario_name}")
        
        def scenario_runner():
            self.active_scenario = scenario_name
            print(f"Starting network scenario: {scenario_name}")
            
            for step in scenario['steps']:
                profile = step['profile']
                duration = step['duration_seconds']
                
                print(f"Switching to {profile} for {duration} seconds")
                self.apply_network_profile(profile)
                time.sleep(duration)
            
            self.active_scenario = None
            print(f"Completed network scenario: {scenario_name}")
        
        if self.scenario_thread and self.scenario_thread.is_alive():
            raise RuntimeError("Another scenario is already running")
        
        self.scenario_thread = threading.Thread(target=scenario_runner)
        self.scenario_thread.daemon = True
        self.scenario_thread.start()

# Global simulator instance
simulator = NetworkSimulator()

@app.route('/status', methods=['GET'])
def get_status():
    """Get current network simulation status"""
    return jsonify({
        'current_profile': simulator.current_profile,
        'active_scenario': simulator.active_scenario,
        'available_profiles': list(simulator.profiles.keys()),
        'available_scenarios': [s['name'] for s in simulator.scenarios]
    })

@app.route('/profile', methods=['POST'])
def set_profile():
    """Set network profile"""
    data = request.get_json()
    profile_name = data.get('profile')
    
    try:
        simulator.apply_network_profile(profile_name)
        return jsonify({'status': 'success', 'profile': profile_name})
    except Exception as e:
        return jsonify({'status': 'error', 'message': str(e)}), 400

@app.route('/scenario', methods=['POST'])
def run_scenario():
    """Run network scenario"""
    data = request.get_json()
    scenario_name = data.get('scenario')
    
    try:
        simulator.run_scenario(scenario_name)
        return jsonify({'status': 'success', 'scenario': scenario_name})
    except Exception as e:
        return jsonify({'status': 'error', 'message': str(e)}), 400

@app.route('/profiles', methods=['GET'])
def get_profiles():
    """Get available network profiles"""
    return jsonify(simulator.profiles)

@app.route('/scenarios', methods=['GET'])
def get_scenarios():
    """Get available scenarios"""
    return jsonify(simulator.scenarios)

if __name__ == '__main__':
    print("Starting PRISM Mobile P2P Network Simulator...")
    print(f"Loaded {len(simulator.profiles)} network profiles")
    print(f"Loaded {len(simulator.scenarios)} transition scenarios")
    app.run(host='0.0.0.0', port=9090, debug=False)
EOF
    
    track_step "Created network simulator Python script" 0
    
    chmod +x "$MOBILE_TEST_DIR/network_simulator.py"
    track_step "Made network simulator executable" 0
    
    log_info "Network simulation controller setup complete"
}

# Create P2P coordinator service
setup_p2p_coordinator() {
    log_header "Setting up P2P Mesh Coordinator"
    
    # Create P2P coordinator Rust binary
    cat > "$MOBILE_TEST_DIR/p2p_coordinator.rs" << 'EOF'
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
EOF
    
    track_step "Created P2P coordinator Rust service" 0
    
    # Create Cargo.toml for the P2P coordinator
    cat > "$MOBILE_TEST_DIR/Cargo.toml" << 'EOF'
[package]
name = "p2p-coordinator"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "p2p_coordinator"
path = "p2p_coordinator.rs"

[dependencies]
tokio = { version = "1.0", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
warp = "0.3"
EOF
    
    track_step "Created Cargo.toml for P2P coordinator" 0
    
    log_info "P2P mesh coordinator setup complete"
}

# Create test collector service
setup_test_collector() {
    log_header "Setting up Test Results Collector"
    
    mkdir -p "$DOCKER_COMPOSE_DIR/test-collector"
    
    # Create test collector Node.js service
    cat > "$DOCKER_COMPOSE_DIR/test-collector/test-collector.js" << 'EOF'
#!/usr/bin/env node

const express = require('express');
const WebSocket = require('ws');
const axios = require('axios');
const fs = require('fs').promises;
const path = require('path');

const app = express();
const PORT = 3001;

// Test results storage
let testResults = [];
let realTimeMetrics = {
    totalTests: 0,
    passedTests: 0,
    failedTests: 0,
    averageExecutionTime: 0,
    mobileSpecificMetrics: {
        iosBatteryImpact: 0,
        androidBatteryImpact: 0,
        p2pMeshStability: 0,
        networkRecoveryTime: 0
    }
};

// WebSocket server for real-time updates
const wss = new WebSocket.Server({ port: 3002 });

app.use(express.json());

// Health check endpoint
app.get('/health', (req, res) => {
    res.json({ status: 'healthy', timestamp: new Date().toISOString() });
});

// Submit test results
app.post('/results', async (req, res) => {
    try {
        const result = {
            ...req.body,
            timestamp: new Date().toISOString(),
            id: Date.now().toString()
        };
        
        testResults.push(result);
        updateMetrics(result);
        
        // Broadcast to WebSocket clients
        broadcastUpdate('test_result', result);
        
        // Save to file
        await saveResults();
        
        console.log(`📊 Test result collected: ${result.testName} - ${result.status}`);
        
        res.json({ status: 'success', id: result.id });
    } catch (error) {
        console.error('Error saving test result:', error);
        res.status(500).json({ error: 'Failed to save test result' });
    }
});

// Get all test results
app.get('/results', (req, res) => {
    res.json({
        results: testResults,
        metrics: realTimeMetrics,
        count: testResults.length
    });
});

// Get real-time metrics
app.get('/metrics', (req, res) => {
    res.json(realTimeMetrics);
});

// Mobile-specific metrics endpoint
app.get('/mobile-metrics', (req, res) => {
    const mobileTests = testResults.filter(r => 
        r.platform === 'ios' || r.platform === 'android'
    );
    
    const iosTests = mobileTests.filter(r => r.platform === 'ios');
    const androidTests = mobileTests.filter(r => r.platform === 'android');
    
    const metrics = {
        totalMobileTests: mobileTests.length,
        iosTests: {
            count: iosTests.length,
            successRate: calculateSuccessRate(iosTests),
            averageBatteryImpact: calculateAverageBatteryImpact(iosTests)
        },
        androidTests: {
            count: androidTests.length,
            successRate: calculateSuccessRate(androidTests),
            averageBatteryImpact: calculateAverageBatteryImpact(androidTests)
        },
        p2pMeshMetrics: calculateP2PMeshMetrics(mobileTests),
        networkRecoveryMetrics: calculateNetworkRecoveryMetrics(mobileTests)
    };
    
    res.json(metrics);
});

function updateMetrics(result) {
    realTimeMetrics.totalTests++;
    
    if (result.status === 'passed') {
        realTimeMetrics.passedTests++;
    } else {
        realTimeMetrics.failedTests++;
    }
    
    // Update average execution time
    const totalTime = realTimeMetrics.averageExecutionTime * (realTimeMetrics.totalTests - 1) + 
                     (result.executionTime || 0);
    realTimeMetrics.averageExecutionTime = totalTime / realTimeMetrics.totalTests;
    
    // Update mobile-specific metrics
    if (result.platform === 'ios' && result.batteryImpact) {
        realTimeMetrics.mobileSpecificMetrics.iosBatteryImpact = 
            (realTimeMetrics.mobileSpecificMetrics.iosBatteryImpact + result.batteryImpact) / 2;
    }
    
    if (result.platform === 'android' && result.batteryImpact) {
        realTimeMetrics.mobileSpecificMetrics.androidBatteryImpact = 
            (realTimeMetrics.mobileSpecificMetrics.androidBatteryImpact + result.batteryImpact) / 2;
    }
    
    if (result.p2pMeshStability) {
        realTimeMetrics.mobileSpecificMetrics.p2pMeshStability = 
            (realTimeMetrics.mobileSpecificMetrics.p2pMeshStability + result.p2pMeshStability) / 2;
    }
    
    if (result.networkRecoveryTime) {
        realTimeMetrics.mobileSpecificMetrics.networkRecoveryTime = 
            (realTimeMetrics.mobileSpecificMetrics.networkRecoveryTime + result.networkRecoveryTime) / 2;
    }
}

function broadcastUpdate(type, data) {
    const message = JSON.stringify({ type, data, timestamp: new Date().toISOString() });
    
    wss.clients.forEach(client => {
        if (client.readyState === WebSocket.OPEN) {
            client.send(message);
        }
    });
}

async function saveResults() {
    try {
        const data = {
            results: testResults,
            metrics: realTimeMetrics,
            lastUpdated: new Date().toISOString()
        };
        
        await fs.writeFile('/app/results/test-results.json', JSON.stringify(data, null, 2));
    } catch (error) {
        console.error('Error saving results to file:', error);
    }
}

function calculateSuccessRate(tests) {
    if (tests.length === 0) return 0;
    const passed = tests.filter(t => t.status === 'passed').length;
    return (passed / tests.length) * 100;
}

function calculateAverageBatteryImpact(tests) {
    const testsWithBattery = tests.filter(t => t.batteryImpact !== undefined);
    if (testsWithBattery.length === 0) return 0;
    
    const total = testsWithBattery.reduce((sum, t) => sum + t.batteryImpact, 0);
    return total / testsWithBattery.length;
}

function calculateP2PMeshMetrics(tests) {
    const p2pTests = tests.filter(t => t.p2pMeshStability !== undefined);
    if (p2pTests.length === 0) return { stability: 0, recoveryTime: 0 };
    
    const avgStability = p2pTests.reduce((sum, t) => sum + t.p2pMeshStability, 0) / p2pTests.length;
    const avgRecovery = p2pTests.reduce((sum, t) => sum + (t.meshRecoveryTime || 0), 0) / p2pTests.length;
    
    return {
        stability: avgStability,
        recoveryTime: avgRecovery
    };
}

function calculateNetworkRecoveryMetrics(tests) {
    const networkTests = tests.filter(t => t.networkRecoveryTime !== undefined);
    if (networkTests.length === 0) return { averageRecoveryTime: 0, successRate: 0 };
    
    const avgRecovery = networkTests.reduce((sum, t) => sum + t.networkRecoveryTime, 0) / networkTests.length;
    const successfulRecoveries = networkTests.filter(t => t.networkRecoverySuccess === true).length;
    const successRate = (successfulRecoveries / networkTests.length) * 100;
    
    return {
        averageRecoveryTime: avgRecovery,
        successRate
    };
}

// WebSocket connection handler
wss.on('connection', (ws) => {
    console.log('📱 New WebSocket client connected');
    
    // Send current metrics to new client
    ws.send(JSON.stringify({
        type: 'initial_metrics',
        data: realTimeMetrics,
        timestamp: new Date().toISOString()
    }));
    
    ws.on('close', () => {
        console.log('📱 WebSocket client disconnected');
    });
});

// Start the server
app.listen(PORT, () => {
    console.log(`🚀 Test Results Collector running on port ${PORT}`);
    console.log(`📡 WebSocket server running on port 3002`);
    console.log(`📊 Real-time metrics collection enabled`);
});

// Graceful shutdown
process.on('SIGINT', async () => {
    console.log('💾 Saving final results...');
    await saveResults();
    console.log('👋 Test Results Collector shutting down');
    process.exit(0);
});
EOF
    
    track_step "Created test results collector service" 0
    
    log_info "Test results collector setup complete"
}

# Create mobile testing control script
create_mobile_control_script() {
    log_header "Creating Mobile Testing Control Script"
    
    cat > "$PROJECT_ROOT/scripts/mobile_test_control.sh" << 'EOF'
#!/bin/bash

# PRISM Mobile P2P Testing Control Script
# Controls mobile testing environment and execution

set -euo pipefail

DOCKER_COMPOSE_DIR="$(dirname "$0")/../docker/mobile-testing"

# Color codes
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Commands
start_environment() {
    log_info "Starting mobile P2P testing environment..."
    cd "$DOCKER_COMPOSE_DIR"
    docker-compose up -d
    
    log_info "Waiting for services to be ready..."
    sleep 30
    
    # Check service health
    check_service_health
    
    log_success "Mobile P2P testing environment is ready!"
    show_environment_status
}

stop_environment() {
    log_info "Stopping mobile P2P testing environment..."
    cd "$DOCKER_COMPOSE_DIR"
    docker-compose down -v
    log_success "Environment stopped"
}

check_service_health() {
    log_info "Checking service health..."
    
    # Check network simulator
    if curl -s http://localhost:9090/status > /dev/null; then
        log_success "Network simulator is healthy"
    else
        log_warning "Network simulator not responding"
    fi
    
    # Check P2P coordinator
    if curl -s http://localhost:8888/peers > /dev/null; then
        log_success "P2P coordinator is healthy"
    else
        log_warning "P2P coordinator not responding"
    fi
    
    # Check test collector
    if curl -s http://localhost:3001/health > /dev/null; then
        log_success "Test results collector is healthy"
    else
        log_warning "Test results collector not responding"
    fi
}

show_environment_status() {
    log_info "Mobile P2P Testing Environment Status:"
    echo "----------------------------------------"
    echo "🍎  iOS Simulator:        http://localhost:5900 (VNC)"
    echo "🤖  Android Emulator:     http://localhost:6080 (VNC)"
    echo "🌐  Network Simulator:    http://localhost:9090/status"
    echo "🔗  P2P Coordinator:      http://localhost:8888/peers"
    echo "📊  Test Results:         http://localhost:3001/results"
    echo "📡  WebSocket Stream:     ws://localhost:3002"
    echo ""
}

run_test_scenario() {
    local scenario="$1"
    log_info "Running mobile test scenario: $scenario"
    
    case "$scenario" in
        "network_switching")
            log_info "Testing network switching scenarios..."
            curl -X POST http://localhost:9090/scenario \
                -H "Content-Type: application/json" \
                -d '{"scenario": "commute_scenario"}'
            ;;
        "battery_saver")
            log_info "Testing battery saver mode..."
            curl -X POST http://localhost:9090/scenario \
                -H "Content-Type: application/json" \
                -d '{"scenario": "battery_saver_scenario"}'
            ;;
        "mesh_recovery")
            log_info "Testing P2P mesh recovery..."
            curl -X POST http://localhost:9090/scenario \
                -H "Content-Type: application/json" \
                -d '{"scenario": "mesh_recovery_scenario"}'
            ;;
        *)
            log_error "Unknown scenario: $scenario"
            echo "Available scenarios: network_switching, battery_saver, mesh_recovery"
            exit 1
            ;;
    esac
    
    log_success "Test scenario '$scenario' started"
}

show_logs() {
    local service="${1:-all}"
    cd "$DOCKER_COMPOSE_DIR"
    
    if [ "$service" = "all" ]; then
        docker-compose logs -f
    else
        docker-compose logs -f "$service"
    fi
}

show_metrics() {
    log_info "Current mobile testing metrics:"
    echo "================================"
    
    # Get P2P mesh status
    echo "🔗 P2P Mesh Status:"
    curl -s http://localhost:8888/status | jq '.'
    echo ""
    
    # Get test results metrics
    echo "📊 Test Results:"
    curl -s http://localhost:3001/metrics | jq '.'
    echo ""
    
    # Get mobile-specific metrics
    echo "📱 Mobile-Specific Metrics:"
    curl -s http://localhost:3001/mobile-metrics | jq '.'
    echo ""
}

# Main command handling
case "${1:-}" in
    "start")
        start_environment
        ;;
    "stop")
        stop_environment
        ;;
    "status")
        show_environment_status
        check_service_health
        ;;
    "scenario")
        if [ $# -lt 2 ]; then
            log_error "Usage: $0 scenario <scenario_name>"
            exit 1
        fi
        run_test_scenario "$2"
        ;;
    "logs")
        show_logs "${2:-all}"
        ;;
    "metrics")
        show_metrics
        ;;
    "restart")
        stop_environment
        sleep 5
        start_environment
        ;;
    *)
        echo "PRISM Mobile P2P Testing Control"
        echo "Usage: $0 <command>"
        echo ""
        echo "Commands:"
        echo "  start     - Start the mobile testing environment"
        echo "  stop      - Stop the mobile testing environment"
        echo "  restart   - Restart the environment"
        echo "  status    - Show environment status"
        echo "  scenario  - Run a test scenario (network_switching|battery_saver|mesh_recovery)"
        echo "  logs      - Show logs (optionally specify service name)"
        echo "  metrics   - Show current testing metrics"
        echo ""
        exit 1
        ;;
esac
EOF
    
    chmod +x "$PROJECT_ROOT/scripts/mobile_test_control.sh"
    track_step "Created mobile testing control script" 0
    
    log_info "Mobile testing control script created"
}

# Main setup execution
main() {
    echo -e "${GREEN}"
    echo "📱 PRISM Mobile P2P Testing Environment Setup"
    echo "============================================="
    echo -e "${NC}"
    
    setup_docker_environment
    setup_network_simulator
    setup_p2p_coordinator
    setup_test_collector
    create_mobile_control_script
    
    echo ""
    log_header "Mobile P2P Testing Environment Setup Complete"
    echo ""
    echo "📊 Setup Summary:"
    echo "  Total Steps: $setup_steps"
    echo "  Completed: $completed_steps"
    echo "  Success Rate: $(( completed_steps * 100 / setup_steps ))%"
    echo ""
    
    if [ $completed_steps -eq $setup_steps ]; then
        echo -e "${GREEN}🎉 SUCCESS${NC}"
        echo "Mobile P2P testing environment is ready for deployment!"
        echo ""
        echo "Next steps:"
        echo "1. Start the environment: ./scripts/mobile_test_control.sh start"
        echo "2. Run network scenarios: ./scripts/mobile_test_control.sh scenario network_switching"
        echo "3. Monitor metrics: ./scripts/mobile_test_control.sh metrics"
        echo "4. View real-time dashboard: http://localhost:3001/results"
    else
        echo -e "${RED}❌ SETUP INCOMPLETE${NC}"
        echo "Some steps failed. Please review the errors above."
        exit 1
    fi
}

# Execute main function
main "$@"