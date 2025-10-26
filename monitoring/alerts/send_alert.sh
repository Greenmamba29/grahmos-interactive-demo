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
