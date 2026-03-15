# Multi-Machine Setup Tutorial

This tutorial walks you through setting up a 2-machine MClaw cluster with the dispatcher.

**Prerequisites:**
- Two machines (can be local + VPS, or any combination)
- A Telegram bot token
- SSH access to both machines

**Last verified:** March 15, 2026.

## Choosing a Connection Mode

MClaw dispatcher supports two connection modes:

| Mode | How It Works | Best For |
|------|--------------|----------|
| **WebSocket** | Clients connect TO dispatcher | NAT/firewall scenarios, no public IP needed |
| **HTTP** | Dispatcher connects TO clients | All machines have public IPs |

**WebSocket mode (recommended)** works for machines behind NAT/firewall without requiring public IP addresses or inbound ports. This tutorial uses WebSocket mode.

## Architecture (WebSocket Mode)

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
│  - Runs: mclaw client (machine_name: "gateway")              │
│  - Accepts WebSocket connections                             │
└─────────────────────────────────────────────────────────────┘
                         │
                         ▼ (WebSocket - outbound from client)
┌─────────────────────────────────────────────────────────────┐
│  Machine 2 (Remote Client - behind NAT)                      │
│  - Private IP: 192.168.1.20 (no public IP needed)           │
│  - Runs: mclaw daemon (connects TO dispatcher)              │
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

[channels_config.telegram]
# We'll set this on dispatcher, leave empty for now
bot_token = ""
enabled = false

[dispatcher]
# This machine is also a client
enabled = true
mode = "ws"  # WebSocket mode
machine_name = "gateway"
endpoint = "ws://localhost:42619"
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
```

### 2.3 Create Machines Registry (Optional)

**Note:** With WebSocket mode, machines auto-register when they connect. The machines.toml file is optional and primarily useful for:
- Pre-configuring authentication tokens
- Setting a default machine
- Adding descriptions

```bash
sudo nano /etc/mclaw/machines.toml
```

```toml
# /etc/mclaw/machines.toml
# Optional: Pre-configure machines (WebSocket clients auto-register)

[[machines]]
name = "gateway"
url = "http://localhost:42618"
default = true
description = "Local gateway machine"

# Remote machines will auto-register via WebSocket
# You can optionally pre-configure them with auth tokens:
# [[machines]]
# name = "remote"
# token = "your_token_here"
# default = false
# description = "Remote client"
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

### 3.2 Configure Remote Client (WebSocket Mode)

```bash
# Edit config
nano ~/.mclaw/config.toml
```

```toml
# ~/.mclaw/config.toml on Machine 2

[network]
allowed_domains = ["api.anthropic.com", "api.openai.com"]

[channels_config.telegram]
# Empty - handled by dispatcher
bot_token = ""
enabled = false

[dispatcher]
enabled = true
mode = "ws"  # WebSocket mode
machine_name = "remote"
endpoint = "ws://203.0.113.10:42619"  # Dispatcher WebSocket URL
auth_token = "your_token_here"  # Optional - if you pre-configure in machines.toml
reconnect_interval_secs = 5
```

### 3.3 Start MClaw Client

```bash
# Start the MClaw client
mclaw daemon

# Or as a service
mclaw service install
mclaw service start
```

### 3.4 Verify Connection

On Machine 2, check logs for successful connection:
```bash
journalctl -u mclaw-daemon -f
```

Look for: `WebSocket client connected` or similar message.

On Machine 1 (dispatcher), check the connected machines:
```bash
curl http://localhost:42619/machines
```

## Step 4: Register Machine 2 with Dispatcher

### WebSocket Mode: Auto-Registration

With WebSocket mode, machines automatically register when they connect! No manual registration needed. Check the dispatcher logs:

```bash
sudo journalctl -u mclaw-dispatcher -f | grep "Auto-registering"
```

You should see:
```
Auto-registering new WebSocket client: remote
WebSocket client connected: remote
```

### Optional: Pre-Configure Machine in machines.toml

Pre-configuring machines in `/etc/mclaw/machines.toml` provides:
- Persistence across dispatcher restarts
- Token-based authentication
- Custom description

```bash
sudo nano /etc/mclaw/machines.toml
```

```toml
# Pre-configured machines (optional for WebSocket clients)

[[machines]]
name = "gateway"
url = "http://localhost:42618"
default = true
description = "Gateway machine"

[[machines]]
name = "remote"
token = "your_auth_token_here"  # Must match auth_token in client config
default = false
description = "Remote client behind NAT"
```

### HTTP Mode Alternative

If you need HTTP mode (dispatcher connects TO clients):

```toml
[[machines]]
name = "remote"
url = "http://198.51.100.20:42618"  # Client's reachable URL
token = "pairing_token"
default = false
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

### WebSocket Mode (Recommended)

1. Install MClaw on new machine
2. Configure dispatcher mode in `~/.mclaw/config.toml`:
   ```toml
   [dispatcher]
   enabled = true
   mode = "ws"
   machine_name = "machine3"
   endpoint = "ws://203.0.113.10:42619"
   ```
3. Start MClaw: `mclaw daemon`
4. Machine auto-registers on connection!

### HTTP Mode (Alternative)

1. Install MClaw on new machine
2. Ensure machine has accessible URL (public IP or port forward)
3. Add entry to `/etc/mclaw/machines.toml`:
   ```toml
   [[machines]]
   name = "machine3"
   url = "http://10.0.0.100:42618"
   token = "pairing_token"
   default = false
   ```
4. Reload dispatcher: `sudo systemctl reload mclaw-dispatcher`

## Security Checklist

- [ ] Firewall configured to only allow necessary ports
- [ ] HTTPS/TLS enabled for webhook (production)
- [ ] WSS (secure WebSocket) enabled for production
- [ ] `allowed_users` restricted to trusted usernames
- [ ] Logs monitored for suspicious activity
- [ ] Regular backups of `/etc/mclaw/` configs

### Firewall Example

#### WebSocket Mode (Recommended)

```bash
# On gateway (Machine 1) - dispatcher
sudo ufw allow 22/tcp      # SSH
sudo ufw allow 443/tcp     # HTTPS for webhook
sudo ufw allow 42619/tcp   # Dispatcher WebSocket port
sudo ufw enable

# On clients - NO inbound ports needed!
# Clients connect OUT to dispatcher via WebSocket
sudo ufw allow 22/tcp      # SSH only
sudo ufw enable
```

#### HTTP Mode

```bash
# On clients - need inbound gateway port
sudo ufw allow 22/tcp      # SSH
sudo ufw allow 42618/tcp   # MClaw gateway (from dispatcher IP only)
sudo ufw enable
```

## Troubleshooting

### WebSocket client not connecting

```bash
# On client: check dispatcher mode logs
journalctl -u mclaw-daemon -f

# On dispatcher: check WebSocket port is listening
sudo ss -tlnp | grep 42619

# Check dispatcher logs for connection attempts
sudo journalctl -u mclaw-dispatcher -f | grep "WebSocket"

# Test WebSocket connection manually
curl -i -N \
  -H "Connection: Upgrade" \
  -H "Upgrade: websocket" \
  -H "Sec-WebSocket-Version: 13" \
  -H "Sec-WebSocket-Key: test123" \
  "http://203.0.113.10:42619/ws/connect?machine_name=test&token=test"
```

### Machine not responding (HTTP mode)

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

### Authentication token mismatch

```bash
# Verify token in client config matches machines.toml
grep auth_token ~/.mclaw/config.toml
grep token /etc/mclaw/machines.toml

# Check dispatcher logs for auth errors
sudo journalctl -u mclaw-dispatcher -f | grep "authentication"
```

## Next Steps

- [ ] Set up TLS/HTTPS for webhook
- [ ] Use WSS (secure WebSocket) for production
- [ ] Configure log rotation
- [ ] Set up monitoring alerts
- [ ] Document your machine inventory
- [ ] Create runbooks for common operations

## Related Docs

- [dispatcher-guide.md](dispatcher-guide.md) - Complete reference with WebSocket details
- [operations-runbook.md](ops/operations-runbook.md) - Day-2 operations
- [network-deployment.md](ops/network-deployment.md) - Network setup
