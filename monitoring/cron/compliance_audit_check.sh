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
