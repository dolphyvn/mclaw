# MClaw Dispatcher Guide

The dispatcher is a centralized routing service that enables one Telegram bot to manage multiple MClaw client instances across different machines.

**Last verified:** March 14, 2026.

## What is the Dispatcher?

The dispatcher acts as a command router between your Telegram bot and multiple MClaw instances running on different machines. Instead of running a separate bot for each machine, you:

1. Run one dispatcher service with your Telegram bot
2. Connect multiple MClaw clients to the dispatcher
3. Route commands to specific machines using the `@machine` syntax

## When to Use It

| Scenario | Use Dispatcher? |
|----------|-----------------|
| Single machine managing commands | No (standard MClaw) |
| 2-3 machines needing occasional commands | Yes |
| Managing a fleet of servers | Yes |
| Running commands on multiple machines simultaneously | Yes (`@all`) |
| Need centralized logging and monitoring | Yes |

## Architecture Overview

```
┌─────────────────┐     ┌──────────────────────────────────────┐
│  Telegram Bot   │────▶│  Dispatcher Service                  │
│  (one bot)      │     │  Host: your-gateway.com              │
└─────────────────┘     │  Port: 42619                         │
                        └──────────────────────────────────────┘
                                          │
                    ┌─────────────────────┼─────────────────────┐
                    │                     │                     │
                    ▼                     ▼                     ▼
           ┌────────────────┐    ┌────────────────┐    ┌────────────────┐
           │  Client 1      │    │  Client 2      │    │  Client N      │
           │  (local)       │    │  51.255.93.22  │    │  ...           │
           │  Port: 42618   │    │  Port: 42618   │    │  Port: 42618   │
           │  machine_name: │    │  machine_name: │    │  machine_name: │
           │  "home-server" │    │  "vps-prod"    │    │  "backup-node" │
           └────────────────┘    └────────────────┘    └────────────────┘
```

## Command Syntax Reference

| Format | Target | Example |
|--------|--------|---------|
| `@machine_name command` | Specific machine | `@home-server uptime` |
| `@all command` | All machines | `@all df -h` |
| `command` (no prefix) | Default machine | `uptime` |
| `@list` | List all machines | `@list` |

### Examples

```bash
# Check uptime on specific machine
@vps-prod uptime

# Run command on all machines
@all systemctl status mclaw

# List registered machines
@list

# Run on default machine (no prefix)
tail -f /var/log/syslog
```

## Configuration

### Dispatcher Config (`/etc/mclaw/dispatcher.toml`)

```toml
[server]
host = "0.0.0.0"
port = 42619

[telegram]
bot_token = "YOUR_BOT_TOKEN"
webhook_url = "https://your-domain.com/webhook"
allowed_users = ["@username1", "@username2"]

[machines_file]
path = "/etc/mclaw/machines.toml"

[logging]
level = "info"
file = "/var/log/mclaw/dispatcher.log"

[security]
# Require pairing tokens from clients
require_pairing = true

# Rate limiting (requests per minute)
rate_limit_per_user = 60
rate_limit_per_machine = 120
```

### Machines Registry (`/etc/mclaw/machines.toml`)

```toml
# Define your MClaw client machines here
[[machines]]
name = "home-server"
url = "http://192.168.1.100:42618"
token = "pairing_token_from_client"
default = true

[[machines]]
name = "vps-prod"
url = "http://51.255.93.22:42618"
token = "pairing_token_from_client"
default = false

[[machines]]
name = "backup-node"
url = "http://10.0.0.50:42618"
token = "pairing_token_from_client"
default = false
```

### Client Config Changes

Each MClaw client needs to be configured to connect to the dispatcher:

```toml
# Disable direct Telegram (handled by dispatcher)
[channels_config.telegram]
bot_token = ""
enabled = false

# Enable dispatcher mode
[dispatcher]
enabled = true
machine_name = "home-server"  # Must match name in machines.toml
dispatcher_url = "http://your-gateway.com:42619"
pairing_token = "generated_pairing_token"
```

## Installation

### On Gateway Server

```bash
# Install dispatcher
cargo install mclaw-dispatcher

# Create config directory
sudo mkdir -p /etc/mclaw
sudo mkdir -p /var/log/mclaw

# Copy configs
sudo cp dispatcher.toml /etc/mclaw/
sudo cp machines.toml /etc/mclaw/

# Set permissions
sudo chmod 600 /etc/mclaw/dispatcher.toml
sudo chmod 600 /etc/mclaw/machines.toml

# Create systemd service
sudo tee /etc/systemd/system/mclaw-dispatcher.service > /dev/null <<'EOF'
[Unit]
Description=MClaw Dispatcher Service
After=network.target

[Service]
Type=simple
User=mclaw
Group=mclaw
ExecStart=/usr/local/bin/mclaw-dispatcher --config /etc/mclaw/dispatcher.toml
Restart=always
RestartSec=10

# Logging
StandardOutput=append:/var/log/mclaw/dispatcher.log
StandardError=append:/var/log/mclaw/dispatcher.error.log

# Security
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/etc/mclaw /var/log/mclaw

[Install]
WantedBy=multi-user.target
EOF

# Create user
sudo useradd -r -s /bin/false mclaw

# Enable and start
sudo systemctl daemon-reload
sudo systemctl enable mclaw-dispatcher
sudo systemctl start mclaw-dispatcher

# Check status
sudo systemctl status mclaw-dispatcher
```

### On Each Client Machine

```bash
# Enable gateway (if not already enabled)
mclaw --gateway

# Or configure in config.toml
# [gateway]
# enabled = true
# host = "0.0.0.0"
# port = 42618

# Generate pairing token
mclaw --pair-generate

# Add the generated token to the dispatcher's machines.toml
# Then restart the dispatcher
```

## Troubleshooting

### Dispatcher not starting

```bash
# Check logs
sudo journalctl -u mclaw-dispatcher -n 50

# Verify config
mclaw-dispatcher --config /etc/mclaw/dispatcher.toml --verify

# Check port availability
sudo ss -tlnp | grep 42619
```

### Client not connecting to dispatcher

```bash
# On client: verify gateway is running
mclaw status

# On client: check dispatcher connectivity
curl http://your-gateway.com:42619/health

# On dispatcher: check client in machines.toml
cat /etc/mclaw/machines.toml

# Verify pairing token matches
# Token must be identical on client and dispatcher
```

### Commands not routing to correct machine

```bash
# List registered machines
@list

# Check dispatcher logs for routing decisions
sudo journalctl -u mclaw-dispatcher -f | grep "Routing"

# Verify machine_name in client config matches machines.toml
grep machine_name ~/.mclaw/config.toml
grep name /etc/mclaw/machines.toml
```

### Telegram webhook not receiving messages

```bash
# Verify webhook URL is set
curl https://api.telegram.org/bot<BOT_TOKEN>/getWebhookInfo

# Test webhook endpoint
curl -X POST https://your-domain.com/webhook \
  -H "Content-Type: application/json" \
  -d '{"update_id": 1, "message": {"chat": {"id": "123"}, "text": "test"}}'

# Check firewall allows inbound on webhook port
sudo ufw status
```

### All commands timing out

```bash
# Check if dispatcher service is running
sudo systemctl status mclaw-dispatcher

# Check if clients' gateway ports are accessible
# From dispatcher server:
nc -zv 192.168.1.100 42618
nc -zv 51.255.93.22 42618

# Check rate limits (may be throttling)
grep rate_limit /etc/mclaw/dispatcher.toml
```

## Monitoring and Health Checks

### Dispatcher Health

```bash
# Health endpoint
curl http://localhost:42619/health

# List connected machines
curl http://localhost:42619/machines

# Service status
sudo systemctl status mclaw-dispatcher
```

### Logs

```bash
# Live logs
sudo journalctl -u mclaw-dispatcher -f

# Last 100 lines
sudo journalctl -u mclaw-dispatcher -n 100

# Log file
tail -f /var/log/mclaw/dispatcher.log
```

## Security Best Practices

1. **Use pairing tokens** - Never leave `token = ""` in machines.toml
2. **Restrict allowed_users** - Only add trusted Telegram usernames
3. **Enable TLS** - Use HTTPS for webhook URL in production
4. **Firewall rules** - Only expose necessary ports
5. **Log rotation** - Configure logrotate for dispatcher logs
6. **Secrets management** - Consider using a secret store for bot tokens

### Example systemd hardening

```toml
[Service]
# ... existing config ...

# Additional security
ProtectClock=yes
ProtectProc=invisible
ProcSubset=pid
RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6
RestrictRealtime=yes
SystemCallFilter=@system-service
SystemCallErrorNumber=EPERM
```

## Limitations and Known Issues

| Issue | Workaround |
|-------|------------|
| Concurrent commands to same machine may queue | Commands execute sequentially per client |
| Large output may be truncated | Use file output and `@all cat /path/to/file` |
| No command history persistence | Use shell history on each machine |
| WebSocket connections may drop on unstable networks | Dispatcher auto-reconnects; check logs |
| No command cancellation once sent | Let command complete or restart client |

## Related Docs

- [multi-machine-setup.md](multi-machine-setup.md) - Step-by-step tutorial
- [operations-runbook.md](ops/operations-runbook.md) - Runtime operations
- [troubleshooting.md](ops/troubleshooting.md) - Common issues
- [network-deployment.md](ops/network-deployment.md) - Network considerations
