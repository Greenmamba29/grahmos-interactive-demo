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
