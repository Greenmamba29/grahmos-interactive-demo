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
