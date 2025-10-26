#!/bin/bash

# PRISM Mobile P2P Health Check (Every 4h)
# Monitors P2P mesh health, network simulation, and battery impact

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$(dirname "$SCRIPT_DIR")")"
TIMESTAMP=$(date -u +"%Y%m%d_%H%M%S")
LOG_FILE="$PROJECT_ROOT/monitoring/logs/mobile_p2p_$TIMESTAMP.log"

echo "$(date -u): Starting mobile P2P health check..." >> "$LOG_FILE"

# Check if mobile testing environment is running
if ! curl -s http://localhost:8888/peers > /dev/null; then
    echo "⚠️ Mobile P2P environment not running, attempting to start..." >> "$LOG_FILE"
    cd "$PROJECT_ROOT"
    if ./scripts/mobile_test_control.sh start >> "$LOG_FILE" 2>&1; then
        echo "✅ Mobile P2P environment started" >> "$LOG_FILE"
        sleep 30  # Wait for services to stabilize
    else
        echo "❌ Failed to start mobile P2P environment" >> "$LOG_FILE"
        ./monitoring/alerts/send_alert.sh "Mobile P2P environment startup failed" "critical"
        exit 1
    fi
fi

# P2P mesh health check
echo "Checking P2P mesh health..." >> "$LOG_FILE"
MESH_STATUS=$(curl -s http://localhost:8888/status)
P2P_HEALTH=$(echo "$MESH_STATUS" | jq -r '.mesh_health')
CONNECTED_PEERS=$(echo "$MESH_STATUS" | jq -r '.connected_peers')

echo "P2P Mesh Health: $P2P_HEALTH, Connected Peers: $CONNECTED_PEERS" >> "$LOG_FILE"

if (( $(echo "$P2P_HEALTH < 0.7" | bc -l) )); then
    echo "❌ P2P mesh health below threshold: $P2P_HEALTH" >> "$LOG_FILE"
    ./monitoring/alerts/send_alert.sh "P2P mesh health degraded: $P2P_HEALTH" "critical"
    MESH_STATUS_RESULT="CRITICAL"
elif (( $(echo "$P2P_HEALTH < 0.9" | bc -l) )); then
    echo "⚠️ P2P mesh health suboptimal: $P2P_HEALTH" >> "$LOG_FILE"
    MESH_STATUS_RESULT="WARNING"
else
    echo "✅ P2P mesh health good: $P2P_HEALTH" >> "$LOG_FILE"
    MESH_STATUS_RESULT="PASS"
fi

# Network switching test
echo "Running network switching test scenario..." >> "$LOG_FILE"
cd "$PROJECT_ROOT"
if ./scripts/mobile_test_control.sh scenario network_switching >> "$LOG_FILE" 2>&1; then
    sleep 300  # Wait for scenario to complete
    
    # Check mobile metrics
    ./scripts/mobile_test_control.sh metrics > "$PROJECT_ROOT/monitoring/tmp/mobile_metrics_$TIMESTAMP.json" 2>> "$LOG_FILE"
    
    # Validate battery impact
    if [ -f "$PROJECT_ROOT/monitoring/tmp/mobile_metrics_$TIMESTAMP.json" ]; then
        IOS_BATTERY=$(jq -r '.iosTests.averageBatteryImpact // 0' "$PROJECT_ROOT/monitoring/tmp/mobile_metrics_$TIMESTAMP.json")
        ANDROID_BATTERY=$(jq -r '.androidTests.averageBatteryImpact // 0' "$PROJECT_ROOT/monitoring/tmp/mobile_metrics_$TIMESTAMP.json")
        
        echo "Battery Impact - iOS: $IOS_BATTERY%, Android: $ANDROID_BATTERY%" >> "$LOG_FILE"
        
        if (( $(echo "$IOS_BATTERY > 5.0" | bc -l) )); then
            echo "⚠️ iOS battery impact exceeds threshold: $IOS_BATTERY%" >> "$LOG_FILE"
            ./monitoring/alerts/send_alert.sh "iOS battery impact threshold exceeded: $IOS_BATTERY%" "medium"
            BATTERY_STATUS="WARNING"
        elif (( $(echo "$ANDROID_BATTERY > 5.0" | bc -l) )); then
            echo "⚠️ Android battery impact exceeds threshold: $ANDROID_BATTERY%" >> "$LOG_FILE"
            ./monitoring/alerts/send_alert.sh "Android battery impact threshold exceeded: $ANDROID_BATTERY%" "medium"
            BATTERY_STATUS="WARNING"
        else
            echo "✅ Battery impact within acceptable limits" >> "$LOG_FILE"
            BATTERY_STATUS="PASS"
        fi
    else
        echo "❌ Could not retrieve mobile metrics" >> "$LOG_FILE"
        BATTERY_STATUS="FAIL"
    fi
    
    NETWORK_TEST_STATUS="PASS"
else
    echo "❌ Network switching test failed" >> "$LOG_FILE"
    ./monitoring/alerts/send_alert.sh "Mobile network switching test failed" "high"
    NETWORK_TEST_STATUS="FAIL"
    BATTERY_STATUS="FAIL"
fi

# Generate summary report
cat > "$PROJECT_ROOT/monitoring/reports/mobile_summary_$TIMESTAMP.json" << EOJ
{
  "timestamp": "$(date -u --iso-8601)",
  "check_type": "mobile_p2p",
  "status": {
    "mesh_health": "$MESH_STATUS_RESULT",
    "connected_peers": $CONNECTED_PEERS,
    "mesh_health_score": $P2P_HEALTH,
    "network_switching_test": "$NETWORK_TEST_STATUS",
    "battery_impact": "$BATTERY_STATUS",
    "ios_battery_impact": ${IOS_BATTERY:-0},
    "android_battery_impact": ${ANDROID_BATTERY:-0}
  },
  "overall_status": "$([ "$MESH_STATUS_RESULT" != "CRITICAL" ] && [ "$NETWORK_TEST_STATUS" = "PASS" ] && echo "PASS" || echo "FAIL")",
  "log_file": "$LOG_FILE"
}
EOJ

echo "$(date -u): Mobile P2P health check completed" >> "$LOG_FILE"
