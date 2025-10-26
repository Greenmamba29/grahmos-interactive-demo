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
