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
