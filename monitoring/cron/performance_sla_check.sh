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
