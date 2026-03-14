# Multi-Machine Setup Tutorial

This tutorial walks you through setting up a 2-machine MClaw cluster with the dispatcher.

**Prerequisites:**
- Two machines (can be local + VPS, or any combination)
- A Telegram bot token
- SSH access to both machines

**Last verified:** March 14, 2026.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Telegram Bot                              │
└────────────────────────┬────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────┐
│  Machine 1 (Gateway/Dispatcher)                              │
│  - Public IP: 203.0.113.10                                   │
│  - Runs: mclaw-dispatcher (port 42619)                       │
│  - Runs: mclaw client (port 42618, machine_name: "gateway") │
└─────────────────────────────────────────────────────────────┘
                         │
                         ▼ (WebSocket)
┌─────────────────────────────────────────────────────────────┐
│  Machine 2 (Remote Client)                                   │
│  - IP: 198.51.100.20                                         │
│  - Runs: mclaw gateway (port 42618)                          │
│  - machine_name: "remote"                                    │
└─────────────────────────────────────────────────────────────┘
```

## Step 1: Prepare Machine 1 (Gateway/Dispatcher)

### 1.1 Install MClaw

```bash
# Install MClaw
curl -fsSL https://raw.githubusercontent.com/zeroclaw-labs/mclaw/main/install.sh | bash

# Verify installation
mclaw --version
```

### 1.2 Configure MClaw Client

```bash
# Run onboarding (for the client running on gateway)
mclaw onboard --interactive

# Or edit config directly
nano ~/.mclaw/config.toml
```

```toml
# ~/.mclaw/config.toml on Machine 1

[network]
allowed_domains = ["api.anthropic.com", "api.openai.com"]

[gateway]
enabled = true
host = "0.0.0.0"
port = 42618

[channels_config.telegram]
# We'll set this on dispatcher, leave empty for now
bot_token = ""
enabled = false

[dispatcher]
# This machine is also a client
enabled = true
machine_name = "gateway"
dispatcher_url = "http://localhost:42619"
```

### 1.3 Generate Pairing Token

```bash
# Generate a pairing token for Machine 1
mclaw --pair-generate

# Save the output, e.g.:
# Pairing token: a1b2c3d4-e5f6-7890-abcd-ef1234567890
```

## Step 2: Install and Configure Dispatcher

### 2.1 Install Dispatcher

```bash
# From source
cargo install mclaw-dispatcher

# Or download prebuilt binary (when available)
wget https://github.com/zeroclaw-labs/mclaw/releases/latest/download/mclaw-dispatcher-linux-amd64
chmod +x mclaw-dispatcher-linux-amd64
sudo mv mclaw-dispatcher-linux-amd64 /usr/local/bin/mclaw-dispatcher
```

### 2.2 Create Dispatcher Config

```bash
# Create config directory
sudo mkdir -p /etc/mclaw
sudo mkdir -p /var/log/mclaw

# Create dispatcher config
sudo nano /etc/mclaw/dispatcher.toml
```

```toml
# /etc/mclaw/dispatcher.toml

[server]
host = "0.0.0.0"
port = 42619

[telegram]
bot_token = "1234567890:ABCdefGHIjklMNOpqrsTUVwxyz"  # Your bot token
webhook_url = "https://203.0.113.10/webhook"  # Update with your IP/domain
allowed_users = ["@your_username"]

[machines_file]
path = "/etc/mclaw/machines.toml"

[logging]
level = "info"
file = "/var/log/mclaw/dispatcher.log"

[security]
require_pairing = true
rate_limit_per_user = 60
```

### 2.3 Create Machines Registry

```bash
sudo nano /etc/mclaw/machines.toml
```

```toml
# /etc/mclaw/machines.toml

[[machines]]
name = "gateway"
url = "http://localhost:42618"
token = "a1b2c3d4-e5f6-7890-abcd-ef1234567890"  # From Step 1.3
default = true

# We'll add the remote machine after setting it up
```

### 2.4 Set Up systemd Service

```bash
sudo tee /etc/systemd/system/mclaw-dispatcher.service > /dev/null <<'EOF'
[Unit]
Description=MClaw Dispatcher Service
After=network.target

[Service]
Type=simple
User=$USER
ExecStart=/usr/local/bin/mclaw-dispatcher --config /etc/mclaw/dispatcher.toml
Restart=always
RestartSec=10
Environment="RUST_LOG=info"

[Install]
WantedBy=multi-user.target
EOF

sudo systemctl daemon-reload
sudo systemctl enable mclaw-dispatcher
sudo systemctl start mclaw-dispatcher
```

### 2.5 Verify Dispatcher

```bash
# Check service status
sudo systemctl status mclaw-dispatcher

# Check health endpoint
curl http://localhost:42619/health

# Should return: {"status":"ok"}
```

## Step 3: Prepare Machine 2 (Remote Client)

### 3.1 Install MClaw

```bash
# SSH into Machine 2
ssh user@198.51.100.20

# Install MClaw
curl -fsSL https://raw.githubusercontent.com/zeroclaw-labs/mclaw/main/install.sh | bash

# Verify
mclaw --version
```

### 3.2 Configure Remote Client

```bash
# Generate pairing token for Machine 2
mclaw --pair-generate

# Save this token: b2c3d4e5-f6g7-8901-bcde-f12345678901
```

```bash
# Edit config
nano ~/.mclaw/config.toml
```

```toml
# ~/.mclaw/config.toml on Machine 2

[network]
allowed_domains = ["api.anthropic.com", "api.openai.com"]

[gateway]
enabled = true
host = "0.0.0.0"
port = 42618

[channels_config.telegram]
# Empty - handled by dispatcher
bot_token = ""
enabled = false

[dispatcher]
enabled = true
machine_name = "remote"
dispatcher_url = "http://203.0.113.10:42619"  # Gateway IP
pairing_token = "b2c3d4e5-f6g7-8901-bcde-f12345678901"  # Generated above
```

### 3.3 Start MClaw Client

```bash
# Start the MClaw client
mclaw daemon

# Or as a service
mclaw service install
mclaw service start
```

## Step 4: Register Machine 2 with Dispatcher

### 4.1 Add Machine to Registry

Back on Machine 1 (Gateway):

```bash
sudo nano /etc/mclaw/machines.toml
```

```toml
[[machines]]
name = "gateway"
url = "http://localhost:42618"
token = "a1b2c3d4-e5f6-7890-abcd-ef1234567890"
default = true

[[machines]]
name = "remote"
url = "http://198.51.100.20:42618"  # Or use domain name
token = "b2c3d4e5-f6g7-8901-bcde-f12345678901"
default = false
```

### 4.2 Reload Dispatcher

```bash
sudo systemctl reload mclaw-dispatcher
# Or restart if reload doesn't work
sudo systemctl restart mclaw-dispatcher
```

## Step 5: Configure Telegram Webhook

### 5.1 Set Webhook

```bash
# Replace BOT_TOKEN with your actual token
curl -X POST https://api.telegram.org/bot<BOT_TOKEN>/setWebhook \
  -H "Content-Type: application/json" \
  -d '{"url": "https://203.0.113.10/webhook"}'
```

### 5.2 Verify Webhook

```bash
curl https://api.telegram.org/bot<BOT_TOKEN>/getWebhookInfo
```

## Step 6: Test Your Setup

### 6.1 List Machines

In Telegram, send to your bot:

```
@list
```

Expected response:
```
Available machines:
• gateway (default)
• remote
```

### 6.2 Test Single Machine

```
@gateway uptime
```

```
@remote uname -a
```

### 6.3 Test All Machines

```
@all echo "Hello from $(hostname)"
```

Expected responses from both machines.

## Step 7: Add More Machines

To add more machines, repeat Steps 3-4:

1. Install MClaw on new machine
2. Generate pairing token
3. Configure dispatcher settings
4. Add entry to `/etc/mclaw/machines.toml`
5. Reload dispatcher

```toml
[[machines]]
name = "machine3"
url = "http://10.0.0.100:42618"
token = "new_pairing_token"
default = false
```

## Security Checklist

- [ ] Firewall configured to only allow necessary ports
- [ ] HTTPS/TLS enabled for webhook (production)
- [ ] Pairing tokens are strong and unique per machine
- [ ] `allowed_users` restricted to trusted usernames
- [ ] Logs monitored for suspicious activity
- [ ] Regular backups of `/etc/mclaw/` configs

### Firewall Example

```bash
# On gateway (Machine 1)
sudo ufw allow 22/tcp      # SSH
sudo ufw allow 443/tcp     # HTTPS for webhook
sudo ufw allow 42619/tcp   # Dispatcher API (restrict if needed)
sudo ufw enable

# On clients
sudo ufw allow 22/tcp      # SSH
sudo ufw allow 42618/tcp   # MClaw gateway (from dispatcher IP only)
sudo ufw enable
```

## Troubleshooting

### Machine not responding

```bash
# On dispatcher: test connectivity to client
curl -H "Authorization: Bearer <token>" http://198.51.100.20:42618/health

# On client: verify gateway is running
mclaw status
netstat -tlnp | grep 42618
```

### Webhook not receiving messages

```bash
# Check webhook status
curl https://api.telegram.org/bot<BOT_TOKEN>/getWebhookInfo

# Check dispatcher is listening
curl http://localhost:42619/health

# Check logs
sudo journalctl -u mclaw-dispatcher -f
```

### Pairing token mismatch

```bash
# Regenerate token on client
mclaw --pair-generate

# Update token in /etc/mclaw/machines.toml on gateway

# Reload dispatcher
sudo systemctl reload mclaw-dispatcher
```

## Next Steps

- [ ] Set up TLS/HTTPS for webhook
- [ ] Configure log rotation
- [ ] Set up monitoring alerts
- [ ] Document your machine inventory
- [ ] Create runbooks for common operations

## Related Docs

- [dispatcher-guide.md](dispatcher-guide.md) - Complete reference
- [operations-runbook.md](ops/operations-runbook.md) - Day-2 operations
- [network-deployment.md](ops/network-deployment.md) - Network setup
