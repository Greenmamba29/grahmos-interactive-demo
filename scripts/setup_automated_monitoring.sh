#!/bin/bash

# PRISM Automated Monitoring Setup
# Configures 4-hour quality checks and real-time monitoring

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
MONITORING_DIR="$PROJECT_ROOT/monitoring"
CRON_DIR="$MONITORING_DIR/cron"
ALERTS_DIR="$MONITORING_DIR/alerts"
REPORTS_DIR="$MONITORING_DIR/reports"

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

# Setup directories
setup_monitoring_structure() {
    log_header "Setting up Monitoring Directory Structure"
    
    mkdir -p "$MONITORING_DIR"
    mkdir -p "$CRON_DIR"
    mkdir -p "$ALERTS_DIR"
    mkdir -p "$REPORTS_DIR"
    mkdir -p "$MONITORING_DIR/logs"
    mkdir -p "$MONITORING_DIR/tmp"
    mkdir -p "$MONITORING_DIR/dashboards"
    
    log_success "Created monitoring directory structure"
}

# Create API contract validation script
create_api_monitoring_script() {
    log_header "Creating API Contract Monitoring Script"
    
    cat > "$CRON_DIR/api_contract_check.sh" << 'EOF'
#!/bin/bash

# PRISM API Contract Validation (Every 4h)
# Validates API contracts, OpenAPI spec, and performance benchmarks

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$(dirname "$SCRIPT_DIR")")"
TIMESTAMP=$(date -u +"%Y%m%d_%H%M%S")
LOG_FILE="$PROJECT_ROOT/monitoring/logs/api_contract_$TIMESTAMP.log"
RESULTS_FILE="$PROJECT_ROOT/monitoring/tmp/api_results_$TIMESTAMP.json"

echo "$(date -u): Starting API contract validation..." >> "$LOG_FILE"

# Change to project root
cd "$PROJECT_ROOT"

# API contract tests
echo "Running API contract tests..." >> "$LOG_FILE"
if cd tests/api && cargo test contract_tests --release --json > "$RESULTS_FILE" 2>> "$LOG_FILE"; then
    echo "✅ API contract tests passed" >> "$LOG_FILE"
    CONTRACT_STATUS="PASS"
else
    echo "❌ API contract tests failed" >> "$LOG_FILE"
    CONTRACT_STATUS="FAIL"
    ./monitoring/alerts/send_alert.sh "API contract tests failed" "critical"
fi

# OpenAPI validation
echo "Validating OpenAPI specification..." >> "$LOG_FILE"
if command -v swagger-parser &> /dev/null; then
    if swagger-parser validate tests/api/openapi.yaml >> "$LOG_FILE" 2>&1; then
        echo "✅ OpenAPI specification valid" >> "$LOG_FILE"
        OPENAPI_STATUS="PASS"
    else
        echo "❌ OpenAPI specification validation failed" >> "$LOG_FILE"
        OPENAPI_STATUS="FAIL"
        ./monitoring/alerts/send_alert.sh "OpenAPI specification invalid" "high"
    fi
else
    echo "⚠️ swagger-parser not available, skipping OpenAPI validation" >> "$LOG_FILE"
    OPENAPI_STATUS="SKIP"
fi

# Performance benchmark
echo "Running API performance benchmarks..." >> "$LOG_FILE"
if timeout 300s bash -c '
    cd tests/api
    cargo test --release test_performance_load --quiet
'; then
    echo "✅ API performance benchmarks passed" >> "$LOG_FILE"
    PERF_STATUS="PASS"
else
    echo "❌ API performance benchmarks failed or timed out" >> "$LOG_FILE"
    PERF_STATUS="FAIL"
    ./monitoring/alerts/send_alert.sh "API performance degraded" "high"
fi

# Generate summary report
cat > "$PROJECT_ROOT/monitoring/reports/api_summary_$TIMESTAMP.json" << EOJ
{
  "timestamp": "$(date -u --iso-8601)",
  "check_type": "api_contract",
  "status": {
    "contract_tests": "$CONTRACT_STATUS",
    "openapi_validation": "$OPENAPI_STATUS", 
    "performance_benchmark": "$PERF_STATUS"
  },
  "overall_status": "$([ "$CONTRACT_STATUS" = "PASS" ] && [ "$PERF_STATUS" = "PASS" ] && echo "PASS" || echo "FAIL")",
  "results_file": "$RESULTS_FILE",
  "log_file": "$LOG_FILE"
}
EOJ

echo "$(date -u): API contract validation completed" >> "$LOG_FILE"
EOF

    chmod +x "$CRON_DIR/api_contract_check.sh"
    log_success "Created API contract monitoring script"
}

# Create mobile P2P monitoring script
create_mobile_monitoring_script() {
    log_header "Creating Mobile P2P Monitoring Script"
    
    cat > "$CRON_DIR/mobile_p2p_check.sh" << 'EOF'
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
EOF

    chmod +x "$CRON_DIR/mobile_p2p_check.sh"
    log_success "Created mobile P2P monitoring script"
}

# Create performance SLA monitoring script
create_performance_monitoring_script() {
    log_header "Creating Performance SLA Monitoring Script"
    
    cat > "$CRON_DIR/performance_sla_check.sh" << 'EOF'
#!/bin/bash

# PRISM Performance SLA Monitoring (Every 4h)
# Validates performance against SLA requirements

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$(dirname "$SCRIPT_DIR")")"
TIMESTAMP=$(date -u +"%Y%m%d_%H%M%S")
LOG_FILE="$PROJECT_ROOT/monitoring/logs/performance_sla_$TIMESTAMP.log"
RESULTS_FILE="$PROJECT_ROOT/monitoring/tmp/sla_results_$TIMESTAMP.json"
VALIDATION_FILE="$PROJECT_ROOT/monitoring/tmp/sla_validation_$TIMESTAMP.json"

echo "$(date -u): Starting performance SLA validation..." >> "$LOG_FILE"

cd "$PROJECT_ROOT"

# Run performance SLA tests
echo "Running performance SLA validation tests..." >> "$LOG_FILE"
if cd tests/performance && cargo test sla_validation --release --json > "$RESULTS_FILE" 2>> "$LOG_FILE"; then
    echo "✅ Performance SLA tests executed successfully" >> "$LOG_FILE"
    TEST_EXECUTION_STATUS="PASS"
else
    echo "❌ Performance SLA test execution failed" >> "$LOG_FILE"
    ./monitoring/alerts/send_alert.sh "Performance SLA test execution failed" "critical"
    TEST_EXECUTION_STATUS="FAIL"
fi

cd "$PROJECT_ROOT"

# Validate against SLA thresholds
echo "Validating against SLA thresholds..." >> "$LOG_FILE"
if python3 tests/scripts/validate_sla_compliance.py "$RESULTS_FILE" \
    --max-storage-latency 50 \
    --min-storage-throughput 100 \
    --max-api-response 200 \
    --max-consensus-latency 200 \
    --max-memory 512 \
    --output-format json > "$VALIDATION_FILE" 2>> "$LOG_FILE"; then
    
    echo "✅ SLA validation completed" >> "$LOG_FILE"
    
    # Check for SLA violations
    VIOLATIONS=$(jq -r '.failed_metrics' "$VALIDATION_FILE" 2>/dev/null || echo "0")
    OVERALL_PASSED=$(jq -r '.overall_passed' "$VALIDATION_FILE" 2>/dev/null || echo "false")
    
    echo "SLA Violations: $VIOLATIONS, Overall Passed: $OVERALL_PASSED" >> "$LOG_FILE"
    
    if [ "$VIOLATIONS" -gt 0 ]; then
        echo "❌ $VIOLATIONS SLA violations detected" >> "$LOG_FILE"
        ./monitoring/alerts/send_alert.sh "$VIOLATIONS SLA violations detected" "critical"
        SLA_VALIDATION_STATUS="CRITICAL"
    elif [ "$OVERALL_PASSED" = "true" ]; then
        echo "✅ All SLA requirements met" >> "$LOG_FILE"
        SLA_VALIDATION_STATUS="PASS"
    else
        echo "⚠️ SLA validation completed with warnings" >> "$LOG_FILE"
        SLA_VALIDATION_STATUS="WARNING"
    fi
else
    echo "❌ SLA validation script failed" >> "$LOG_FILE"
    ./monitoring/alerts/send_alert.sh "SLA validation script failed" "high"
    SLA_VALIDATION_STATUS="FAIL"
    VIOLATIONS="unknown"
    OVERALL_PASSED="false"
fi

# Extract key metrics if available
if [ -f "$VALIDATION_FILE" ]; then
    STORAGE_LATENCY=$(jq -r '.results[] | select(.metric_name=="storage_latency_ms") | .measured_value' "$VALIDATION_FILE" 2>/dev/null || echo "0")
    STORAGE_THROUGHPUT=$(jq -r '.results[] | select(.metric_name=="storage_throughput_mbs") | .measured_value' "$VALIDATION_FILE" 2>/dev/null || echo "0")
    API_RESPONSE=$(jq -r '.results[] | select(.metric_name=="api_response_ms") | .measured_value' "$VALIDATION_FILE" 2>/dev/null || echo "0")
    MEMORY_USAGE=$(jq -r '.results[] | select(.metric_name=="memory_usage_mb") | .measured_value' "$VALIDATION_FILE" 2>/dev/null || echo "0")
else
    STORAGE_LATENCY="0"
    STORAGE_THROUGHPUT="0"
    API_RESPONSE="0"
    MEMORY_USAGE="0"
fi

# Generate summary report
cat > "$PROJECT_ROOT/monitoring/reports/performance_summary_$TIMESTAMP.json" << EOJ
{
  "timestamp": "$(date -u --iso-8601)",
  "check_type": "performance_sla",
  "status": {
    "test_execution": "$TEST_EXECUTION_STATUS",
    "sla_validation": "$SLA_VALIDATION_STATUS",
    "violations_count": $VIOLATIONS,
    "overall_passed": $OVERALL_PASSED
  },
  "metrics": {
    "storage_latency_ms": $STORAGE_LATENCY,
    "storage_throughput_mbs": $STORAGE_THROUGHPUT,
    "api_response_ms": $API_RESPONSE,
    "memory_usage_mb": $MEMORY_USAGE
  },
  "overall_status": "$([ "$SLA_VALIDATION_STATUS" = "PASS" ] && echo "PASS" || echo "FAIL")",
  "results_file": "$RESULTS_FILE",
  "validation_file": "$VALIDATION_FILE",
  "log_file": "$LOG_FILE"
}
EOJ

echo "$(date -u): Performance SLA validation completed" >> "$LOG_FILE"
EOF

    chmod +x "$CRON_DIR/performance_sla_check.sh"
    log_success "Created performance SLA monitoring script"
}

# Create compliance monitoring script
create_compliance_monitoring_script() {
    log_header "Creating Compliance Monitoring Script"
    
    cat > "$CRON_DIR/compliance_audit_check.sh" << 'EOF'
#!/bin/bash

# PRISM Compliance Audit (Every 4h)
# Validates RBAC, GDPR, and SOC 2 compliance requirements

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$(dirname "$SCRIPT_DIR")")"
TIMESTAMP=$(date -u +"%Y%m%d_%H%M%S")
LOG_FILE="$PROJECT_ROOT/monitoring/logs/compliance_audit_$TIMESTAMP.log"
RESULTS_FILE="$PROJECT_ROOT/monitoring/tmp/compliance_results_$TIMESTAMP.json"

echo "$(date -u): Starting compliance audit..." >> "$LOG_FILE"

cd "$PROJECT_ROOT"

# RBAC compliance tests
echo "Running RBAC compliance tests..." >> "$LOG_FILE"
if cd tests/compliance && cargo test rbac_tests --release --json > "$RESULTS_FILE" 2>> "$LOG_FILE"; then
    echo "✅ RBAC compliance tests passed" >> "$LOG_FILE"
    RBAC_STATUS="PASS"
else
    echo "❌ RBAC compliance tests failed" >> "$LOG_FILE"
    ./monitoring/alerts/send_alert.sh "RBAC compliance tests failed" "critical"
    RBAC_STATUS="FAIL"
fi

# GDPR compliance tests
echo "Running GDPR compliance tests..." >> "$LOG_FILE"
cd "$PROJECT_ROOT/tests/compliance"
if cargo test gdpr_compliance_tests --release --quiet >> "$LOG_FILE" 2>&1; then
    echo "✅ GDPR compliance tests passed" >> "$LOG_FILE"
    GDPR_STATUS="PASS"
else
    echo "❌ GDPR compliance tests failed" >> "$LOG_FILE"
    ./monitoring/alerts/send_alert.sh "GDPR compliance validation failed" "critical"
    GDPR_STATUS="FAIL"
fi

# SOC 2 compliance tests
echo "Running SOC 2 compliance tests..." >> "$LOG_FILE"
if cargo test test_soc2_compliance --release --quiet >> "$LOG_FILE" 2>&1; then
    echo "✅ SOC 2 compliance tests passed" >> "$LOG_FILE"
    SOC2_STATUS="PASS"
else
    echo "❌ SOC 2 compliance tests failed" >> "$LOG_FILE"
    ./monitoring/alerts/send_alert.sh "SOC 2 compliance validation failed" "critical"
    SOC2_STATUS="FAIL"
fi

# ISO 27001 compliance tests
echo "Running ISO 27001 compliance tests..." >> "$LOG_FILE"
if cargo test test_iso27001_requirements --release --quiet >> "$LOG_FILE" 2>&1; then
    echo "✅ ISO 27001 compliance tests passed" >> "$LOG_FILE"
    ISO27001_STATUS="PASS"
else
    echo "❌ ISO 27001 compliance tests failed" >> "$LOG_FILE"
    ./monitoring/alerts/send_alert.sh "ISO 27001 compliance validation failed" "high"
    ISO27001_STATUS="FAIL"
fi

# Encryption compliance tests
echo "Running encryption compliance tests..." >> "$LOG_FILE"
if cargo test test_encryption_compliance --release --quiet >> "$LOG_FILE" 2>&1; then
    echo "✅ Encryption compliance tests passed" >> "$LOG_FILE"
    ENCRYPTION_STATUS="PASS"
else
    echo "❌ Encryption compliance tests failed" >> "$LOG_FILE"
    ./monitoring/alerts/send_alert.sh "Encryption compliance validation failed" "high"
    ENCRYPTION_STATUS="FAIL"
fi

cd "$PROJECT_ROOT"

# Generate compliance report
echo "Generating compliance report..." >> "$LOG_FILE"
if python3 -c "
import json
import sys
from datetime import datetime

report_data = {
    'timestamp': datetime.utcnow().isoformat() + 'Z',
    'audit_type': 'comprehensive_compliance',
    'compliance_areas': {
        'rbac': '$RBAC_STATUS',
        'gdpr': '$GDPR_STATUS', 
        'soc2': '$SOC2_STATUS',
        'iso27001': '$ISO27001_STATUS',
        'encryption': '$ENCRYPTION_STATUS'
    },
    'overall_compliance': 'PASS' if all(status == 'PASS' for status in ['$RBAC_STATUS', '$GDPR_STATUS', '$SOC2_STATUS']) else 'FAIL',
    'critical_failures': sum(1 for status in ['$RBAC_STATUS', '$GDPR_STATUS', '$SOC2_STATUS'] if status == 'FAIL'),
    'warning_failures': sum(1 for status in ['$ISO27001_STATUS', '$ENCRYPTION_STATUS'] if status == 'FAIL')
}

with open('$PROJECT_ROOT/monitoring/reports/compliance_$TIMESTAMP.json', 'w') as f:
    json.dump(report_data, f, indent=2)

print('Compliance report generated successfully')
" >> "$LOG_FILE" 2>&1; then
    echo "✅ Compliance report generated" >> "$LOG_FILE"
    REPORT_STATUS="PASS"
else
    echo "❌ Failed to generate compliance report" >> "$LOG_FILE"
    REPORT_STATUS="FAIL"
fi

# Overall compliance status
CRITICAL_FAILURES=$(python3 -c "print(sum(1 for status in ['$RBAC_STATUS', '$GDPR_STATUS', '$SOC2_STATUS'] if status == 'FAIL'))")

if [ "$CRITICAL_FAILURES" -gt 0 ]; then
    echo "❌ $CRITICAL_FAILURES critical compliance failures detected" >> "$LOG_FILE"
    ./monitoring/alerts/send_alert.sh "$CRITICAL_FAILURES critical compliance failures detected" "critical"
    OVERALL_STATUS="CRITICAL"
else
    echo "✅ All critical compliance requirements met" >> "$LOG_FILE"
    OVERALL_STATUS="PASS"
fi

echo "$(date -u): Compliance audit completed" >> "$LOG_FILE"
EOF

    chmod +x "$CRON_DIR/compliance_audit_check.sh"
    log_success "Created compliance monitoring script"
}

# Create alerting system
create_alerting_system() {
    log_header "Creating Alerting System"
    
    # Alert sender script
    cat > "$ALERTS_DIR/send_alert.sh" << 'EOF'
#!/bin/bash

# PRISM Alert Sender
# Sends alerts via multiple channels based on severity

ALERT_MESSAGE="$1"
SEVERITY="${2:-medium}"
TIMESTAMP=$(date -u --iso-8601)

# Configuration
SLACK_WEBHOOK="${PRISM_SLACK_WEBHOOK:-}"
EMAIL_RECIPIENTS="${PRISM_EMAIL_ALERTS:-}"
SMS_WEBHOOK="${PRISM_SMS_WEBHOOK:-}"

# Color codes for severity
case "$SEVERITY" in
    "critical")
        COLOR="#FF0000"
        EMOJI="🚨"
        CHANNELS="slack,email,sms"
        ;;
    "high")
        COLOR="#FFA500" 
        EMOJI="⚠️"
        CHANNELS="slack,email"
        ;;
    "medium")
        COLOR="#FFFF00"
        EMOJI="⚠️"
        CHANNELS="slack"
        ;;
    "low")
        COLOR="#00FF00"
        EMOJI="ℹ️"
        CHANNELS="slack"
        ;;
    *)
        COLOR="#808080"
        EMOJI="📋"
        CHANNELS="slack"
        ;;
esac

# Log alert
echo "[$TIMESTAMP] $SEVERITY: $ALERT_MESSAGE" >> "$(dirname "$0")/../logs/alerts.log"

# Send Slack notification
if [[ "$CHANNELS" == *"slack"* ]] && [ -n "$SLACK_WEBHOOK" ]; then
    curl -X POST "$SLACK_WEBHOOK" \
        -H 'Content-type: application/json' \
        --data "{
            \"attachments\": [{
                \"color\": \"$COLOR\",
                \"blocks\": [{
                    \"type\": \"section\",
                    \"text\": {
                        \"type\": \"mrkdwn\",
                        \"text\": \"$EMOJI *PRISM Alert - $SEVERITY*\\n$ALERT_MESSAGE\\n\\n*Timestamp:* $TIMESTAMP\"
                    }
                }]
            }]
        }" 2>/dev/null || echo "Failed to send Slack notification"
fi

# Send email notification
if [[ "$CHANNELS" == *"email"* ]] && [ -n "$EMAIL_RECIPIENTS" ] && command -v mail &> /dev/null; then
    echo -e "PRISM Alert - $SEVERITY\n\nMessage: $ALERT_MESSAGE\nTimestamp: $TIMESTAMP\n\nView Dashboard: http://localhost:3001/dashboard" | \
        mail -s "PRISM Alert: $ALERT_MESSAGE" "$EMAIL_RECIPIENTS" 2>/dev/null || echo "Failed to send email notification"
fi

# Send SMS notification (for critical alerts only)
if [[ "$CHANNELS" == *"sms"* ]] && [ -n "$SMS_WEBHOOK" ] && [ "$SEVERITY" = "critical" ]; then
    curl -X POST "$SMS_WEBHOOK" \
        -H 'Content-type: application/json' \
        --data "{\"message\": \"PRISM CRITICAL: $ALERT_MESSAGE at $TIMESTAMP\"}" 2>/dev/null || echo "Failed to send SMS notification"
fi

echo "Alert sent: $SEVERITY - $ALERT_MESSAGE"
EOF

    chmod +x "$ALERTS_DIR/send_alert.sh"
    log_success "Created alert sender script"
    
    # Create alert configuration template
    cat > "$ALERTS_DIR/config.env.example" << 'EOF'
# PRISM Alert Configuration
# Copy this file to config.env and update with your webhook URLs

# Slack webhook URL for alerts
PRISM_SLACK_WEBHOOK=https://hooks.slack.com/services/YOUR/SLACK/WEBHOOK

# Email recipients for high/critical alerts (comma-separated)
PRISM_EMAIL_ALERTS=alerts@yourcompany.com,cto@yourcompany.com

# SMS webhook URL for critical alerts only
PRISM_SMS_WEBHOOK=https://api.sms-service.com/send

# Dashboard URL (customize if different)
PRISM_DASHBOARD_URL=http://localhost:3001/dashboard
EOF

    log_success "Created alert configuration template"
}

# Create monitoring dashboard generator
create_dashboard_generator() {
    log_header "Creating Dashboard Generator"
    
    cat > "$MONITORING_DIR/generate_dashboard_data.sh" << 'EOF'
#!/bin/bash

# PRISM Monitoring Dashboard Data Generator
# Aggregates monitoring data for real-time dashboard

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TIMESTAMP=$(date -u --iso-8601)
OUTPUT_FILE="$PROJECT_ROOT/monitoring/dashboards/dashboard_data.json"

# Collect latest reports
LATEST_API_REPORT=$(find "$PROJECT_ROOT/monitoring/reports" -name "api_summary_*.json" -type f | sort -r | head -n 1)
LATEST_MOBILE_REPORT=$(find "$PROJECT_ROOT/monitoring/reports" -name "mobile_summary_*.json" -type f | sort -r | head -n 1)
LATEST_PERFORMANCE_REPORT=$(find "$PROJECT_ROOT/monitoring/reports" -name "performance_summary_*.json" -type f | sort -r | head -n 1)
LATEST_COMPLIANCE_REPORT=$(find "$PROJECT_ROOT/monitoring/reports" -name "compliance_*.json" -type f | sort -r | head -n 1)

# Generate dashboard data
python3 -c "
import json
import sys
from datetime import datetime

dashboard_data = {
    'last_updated': '$TIMESTAMP',
    'status': 'operational',
    'components': {}
}

# API Component
if '$LATEST_API_REPORT' and '$LATEST_API_REPORT' != '':
    try:
        with open('$LATEST_API_REPORT', 'r') as f:
            api_data = json.load(f)
        dashboard_data['components']['api'] = {
            'status': api_data.get('overall_status', 'unknown'),
            'last_check': api_data.get('timestamp', '$TIMESTAMP'),
            'details': api_data.get('status', {})
        }
    except:
        pass

# Mobile Component  
if '$LATEST_MOBILE_REPORT' and '$LATEST_MOBILE_REPORT' != '':
    try:
        with open('$LATEST_MOBILE_REPORT', 'r') as f:
            mobile_data = json.load(f)
        dashboard_data['components']['mobile'] = {
            'status': mobile_data.get('overall_status', 'unknown'),
            'last_check': mobile_data.get('timestamp', '$TIMESTAMP'),
            'mesh_health': mobile_data.get('status', {}).get('mesh_health_score', 0),
            'connected_peers': mobile_data.get('status', {}).get('connected_peers', 0)
        }
    except:
        pass

# Performance Component
if '$LATEST_PERFORMANCE_REPORT' and '$LATEST_PERFORMANCE_REPORT' != '':
    try:
        with open('$LATEST_PERFORMANCE_REPORT', 'r') as f:
            perf_data = json.load(f)
        dashboard_data['components']['performance'] = {
            'status': perf_data.get('overall_status', 'unknown'),
            'last_check': perf_data.get('timestamp', '$TIMESTAMP'),
            'metrics': perf_data.get('metrics', {}),
            'violations': perf_data.get('status', {}).get('violations_count', 0)
        }
    except:
        pass

# Compliance Component
if '$LATEST_COMPLIANCE_REPORT' and '$LATEST_COMPLIANCE_REPORT' != '':
    try:
        with open('$LATEST_COMPLIANCE_REPORT', 'r') as f:
            compliance_data = json.load(f)
        dashboard_data['components']['compliance'] = {
            'status': compliance_data.get('overall_compliance', 'unknown'),
            'last_check': compliance_data.get('timestamp', '$TIMESTAMP'),
            'areas': compliance_data.get('compliance_areas', {}),
            'critical_failures': compliance_data.get('critical_failures', 0)
        }
    except:
        pass

# Overall system status
component_statuses = [comp.get('status', 'unknown') for comp in dashboard_data['components'].values()]
if 'CRITICAL' in component_statuses or 'FAIL' in component_statuses:
    dashboard_data['status'] = 'degraded'
elif 'WARNING' in component_statuses:
    dashboard_data['status'] = 'partial_outage'
else:
    dashboard_data['status'] = 'operational'

# Save dashboard data
with open('$OUTPUT_FILE', 'w') as f:
    json.dump(dashboard_data, f, indent=2)

print(f'Dashboard data generated: {dashboard_data[\"status\"]}')
"

echo "Dashboard data generated at: $OUTPUT_FILE"
EOF

    chmod +x "$MONITORING_DIR/generate_dashboard_data.sh"
    log_success "Created dashboard data generator"
}

# Create cron configuration
setup_cron_schedule() {
    log_header "Setting up Cron Schedule"
    
    # Create crontab entries
    cat > "$MONITORING_DIR/prism-monitoring.crontab" << EOF
# PRISM Automated Monitoring Schedule
# 4-hour quality checks as specified in coordination protocol

# API Contract validation (every 4 hours)
0 2,6,10,14,18,22 * * * $CRON_DIR/api_contract_check.sh

# Mobile P2P health check (every 4 hours, offset by 1 hour)
0 3,7,11,15,19,23 * * * $CRON_DIR/mobile_p2p_check.sh

# Performance SLA monitoring (every 4 hours, offset by 2 hours)  
0 4,8,12,16,20,0 * * * $CRON_DIR/performance_sla_check.sh

# Compliance audit (every 4 hours, offset by 3 hours)
0 5,9,13,17,21,1 * * * $CRON_DIR/compliance_audit_check.sh

# Dashboard data generation (every 30 minutes)
*/30 * * * * $MONITORING_DIR/generate_dashboard_data.sh

# Log cleanup (daily at 01:00 UTC)
0 1 * * * find $MONITORING_DIR/logs -name "*.log" -mtime +7 -delete
0 1 * * * find $MONITORING_DIR/tmp -name "*.json" -mtime +3 -delete

# Weekly summary report (Sundays at 23:00 UTC)
0 23 * * 0 $MONITORING_DIR/generate_weekly_summary.sh
EOF

    log_success "Created cron schedule configuration"
    
    # Instructions for cron installation
    cat > "$MONITORING_DIR/INSTALL_CRON.md" << 'EOF'
# Installing PRISM Monitoring Cron Jobs

## Automatic Installation
Run the following command to install the monitoring cron jobs:

```bash
crontab monitoring/prism-monitoring.crontab
```

## Manual Installation
1. Edit your crontab:
   ```bash
   crontab -e
   ```

2. Add the contents of `monitoring/prism-monitoring.crontab` to your crontab.

3. Save and exit.

## Verify Installation
Check that the cron jobs are installed:
```bash
crontab -l
```

## View Cron Logs
Monitor cron job execution:
```bash
tail -f /var/log/cron
# or on macOS:
tail -f /var/log/system.log | grep cron
```

## Environment Variables
Make sure the following environment variables are set for the cron jobs:

- `PRISM_SLACK_WEBHOOK` - Slack webhook URL for alerts
- `PRISM_EMAIL_ALERTS` - Email addresses for alerts
- `PRISM_SMS_WEBHOOK` - SMS webhook URL for critical alerts (optional)

You can set these in `/etc/environment` or in your cron environment.
EOF

    log_success "Created cron installation instructions"
}

# Create status checker script
create_status_checker() {
    log_header "Creating Status Checker"
    
    cat > "$PROJECT_ROOT/scripts/check_monitoring_status.sh" << 'EOF'
#!/bin/bash

# PRISM Monitoring Status Checker
# Provides quick overview of monitoring system health

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MONITORING_DIR="$PROJECT_ROOT/monitoring"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}PRISM Monitoring Status Check${NC}"
echo "==============================="
echo

# Check if monitoring directories exist
if [ -d "$MONITORING_DIR" ]; then
    echo -e "${GREEN}✅ Monitoring system installed${NC}"
else
    echo -e "${RED}❌ Monitoring system not installed${NC}"
    exit 1
fi

# Check cron jobs
echo "Checking cron jobs..."
if crontab -l 2>/dev/null | grep -q "prism.*check"; then
    echo -e "${GREEN}✅ Monitoring cron jobs installed${NC}"
    CRON_COUNT=$(crontab -l 2>/dev/null | grep -c "prism.*check" || echo "0")
    echo "   Found $CRON_COUNT monitoring jobs"
else
    echo -e "${YELLOW}⚠️ Monitoring cron jobs not found${NC}"
fi

# Check recent activity
echo
echo "Recent monitoring activity:"

# API monitoring
LATEST_API_LOG=$(find "$MONITORING_DIR/logs" -name "api_contract_*.log" -type f 2>/dev/null | sort -r | head -n 1)
if [ -n "$LATEST_API_LOG" ]; then
    API_TIME=$(stat -c %y "$LATEST_API_LOG" 2>/dev/null | cut -d' ' -f1,2 | cut -d'.' -f1)
    echo -e "${GREEN}✅ API Contract Check:${NC} $API_TIME"
else
    echo -e "${YELLOW}⚠️ API Contract Check:${NC} No recent logs"
fi

# Mobile monitoring
LATEST_MOBILE_LOG=$(find "$MONITORING_DIR/logs" -name "mobile_p2p_*.log" -type f 2>/dev/null | sort -r | head -n 1)
if [ -n "$LATEST_MOBILE_LOG" ]; then
    MOBILE_TIME=$(stat -c %y "$LATEST_MOBILE_LOG" 2>/dev/null | cut -d' ' -f1,2 | cut -d'.' -f1)
    echo -e "${GREEN}✅ Mobile P2P Check:${NC} $MOBILE_TIME"
else
    echo -e "${YELLOW}⚠️ Mobile P2P Check:${NC} No recent logs"
fi

# Performance monitoring
LATEST_PERF_LOG=$(find "$MONITORING_DIR/logs" -name "performance_sla_*.log" -type f 2>/dev/null | sort -r | head -n 1)
if [ -n "$LATEST_PERF_LOG" ]; then
    PERF_TIME=$(stat -c %y "$LATEST_PERF_LOG" 2>/dev/null | cut -d' ' -f1,2 | cut -d'.' -f1)
    echo -e "${GREEN}✅ Performance SLA Check:${NC} $PERF_TIME"
else
    echo -e "${YELLOW}⚠️ Performance SLA Check:${NC} No recent logs"
fi

# Compliance monitoring
LATEST_COMPLIANCE_LOG=$(find "$MONITORING_DIR/logs" -name "compliance_audit_*.log" -type f 2>/dev/null | sort -r | head -n 1)
if [ -n "$LATEST_COMPLIANCE_LOG" ]; then
    COMPLIANCE_TIME=$(stat -c %y "$LATEST_COMPLIANCE_LOG" 2>/dev/null | cut -d' ' -f1,2 | cut -d'.' -f1)
    echo -e "${GREEN}✅ Compliance Audit:${NC} $COMPLIANCE_TIME"
else
    echo -e "${YELLOW}⚠️ Compliance Audit:${NC} No recent logs"
fi

# Check dashboard data
echo
if [ -f "$MONITORING_DIR/dashboards/dashboard_data.json" ]; then
    DASHBOARD_STATUS=$(jq -r '.status' "$MONITORING_DIR/dashboards/dashboard_data.json" 2>/dev/null || echo "unknown")
    case "$DASHBOARD_STATUS" in
        "operational")
            echo -e "${GREEN}✅ System Status: $DASHBOARD_STATUS${NC}"
            ;;
        "degraded"|"partial_outage")
            echo -e "${YELLOW}⚠️ System Status: $DASHBOARD_STATUS${NC}"
            ;;
        *)
            echo -e "${RED}❌ System Status: $DASHBOARD_STATUS${NC}"
            ;;
    esac
else
    echo -e "${YELLOW}⚠️ Dashboard data not available${NC}"
fi

# Recent alerts
echo
ALERT_LOG="$MONITORING_DIR/logs/alerts.log"
if [ -f "$ALERT_LOG" ]; then
    RECENT_ALERTS=$(tail -5 "$ALERT_LOG" 2>/dev/null | wc -l)
    echo "Recent alerts (last 5): $RECENT_ALERTS"
    if [ "$RECENT_ALERTS" -gt 0 ]; then
        echo "Latest alerts:"
        tail -5 "$ALERT_LOG" | sed 's/^/  /'
    fi
else
    echo "No alert log found"
fi

echo
echo "=== Quick Actions ==="
echo "View dashboard data:    cat $MONITORING_DIR/dashboards/dashboard_data.json | jq"
echo "Check cron logs:        tail -f /var/log/cron"
echo "View recent alerts:     tail -f $MONITORING_DIR/logs/alerts.log"
echo "Run manual API check:   $MONITORING_DIR/cron/api_contract_check.sh"
EOF

    chmod +x "$PROJECT_ROOT/scripts/check_monitoring_status.sh"
    log_success "Created monitoring status checker"
}

# Main setup execution
main() {
    echo -e "${GREEN}"
    echo "🔍 PRISM Automated Monitoring Setup"
    echo "====================================="
    echo -e "${NC}"
    
    setup_monitoring_structure
    create_api_monitoring_script
    create_mobile_monitoring_script
    create_performance_monitoring_script
    create_compliance_monitoring_script
    create_alerting_system
    create_dashboard_generator
    setup_cron_schedule
    create_status_checker
    
    echo
    log_header "Automated Monitoring Setup Complete"
    echo
    echo "📊 Setup Summary:"
    echo "  ✅ Monitoring directory structure created"
    echo "  ✅ 4-hour validation scripts created:"
    echo "      - API contract monitoring"
    echo "      - Mobile P2P health checks"  
    echo "      - Performance SLA validation"
    echo "      - Compliance auditing"
    echo "  ✅ Alert system configured"
    echo "  ✅ Dashboard data generator created"
    echo "  ✅ Cron schedule configured"
    echo "  ✅ Status checker utility created"
    echo
    
    echo -e "${GREEN}🎉 SUCCESS${NC}"
    echo "Automated monitoring system is ready for activation!"
    echo
    echo -e "${BLUE}Next steps:${NC}"
    echo "1. Install cron jobs: crontab monitoring/prism-monitoring.crontab"
    echo "2. Configure alerts: cp monitoring/alerts/config.env.example monitoring/alerts/config.env"
    echo "3. Edit alert configuration with your webhook URLs"
    echo "4. Test monitoring: ./scripts/check_monitoring_status.sh"
    echo "5. Start mobile P2P environment if not running: ./scripts/mobile_test_control.sh start"
    echo
    echo -e "${YELLOW}Configuration files:${NC}"
    echo "- Cron schedule: monitoring/prism-monitoring.crontab"
    echo "- Alert config: monitoring/alerts/config.env.example"
    echo "- Installation guide: monitoring/INSTALL_CRON.md"
}

# Execute main function
main "$@"