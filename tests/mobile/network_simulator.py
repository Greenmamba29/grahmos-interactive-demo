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
