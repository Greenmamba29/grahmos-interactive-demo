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
