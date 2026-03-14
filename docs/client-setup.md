# MClaw Client Setup for Multi-Machine Dispatcher

This guide explains how to set up a MClaw client machine to register with the dispatcher.

## Prerequisites

- Linux server (Ubuntu/Debian recommended)
- Root or sudo access
- Network connectivity to the dispatcher server

## Quick Setup (client2 example)

### Step 1: Install Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source ~/.cargo/env
```

### Step 2: Copy and Build MClaw

```bash
# Copy source (from development machine)
rsync -av --exclude=target --exclude='.git' /opt/works/personal/github/mclaw/ root@YOUR_CLIENT:/opt/mclaw/

# On client machine
cd /opt/mclaw
cargo build --release
cp target/release/mclaw /usr/local/bin/mclaw
chmod +x /usr/local/bin/mclaw
```

### Step 3: Configure MClaw

Edit `/root/.mclaw/config.toml`:

```toml
# Dispatcher configuration
[dispatcher]
enabled = true
machine_name = "client2"  # Unique name for this machine
endpoint = "http://ns3366383.ip-37-187-77.eu:42619"
auth_token = "YOUR_BOT_TOKEN"  # Use same token as dispatcher
description = "Remote production server"
default = false
registration_interval_secs = 30

# Gateway configuration
[gateway]
enabled = true
host = "0.0.0.0"
port = 42618
allow_public_bind = true  # Required for external access

# Security
[autonomy]
level = "full"
allowed_commands = ["*"]
```

### Step 4: Create Systemd Services

**Gateway Service** (`/etc/systemd/system/mclaw-gateway.service`):

```ini
[Unit]
Description=MClaw Gateway - Multi-machine client
After=network.target

[Service]
Type=simple
User=root
WorkingDirectory=/root
Environment="RUST_LOG=info"
ExecStart=/usr/local/bin/mclaw gateway start
Restart=on-failure
RestartSec=10s

[Install]
WantedBy=multi-user.target
```

**Heartbeat Service** (`/etc/systemd/system/mclaw-heartbeat.service`):

```ini
[Unit]
Description=MClaw Dispatcher Heartbeat
After=network.target

[Service]
Type=simple
User=root
ExecStart=/bin/bash -c 'while true; do sleep 30; curl -s -X POST http://ns3366383.ip-37-187-77.eu:42619/heartbeat -H "Content-Type: application/json" -d "{\"machine_name\": \"client2\", \"url\": \"http://51.255.93.22:42618\"}"; done'
Restart=on-failure

[Install]
WantedBy=multi-user.target
```

### Step 5: Enable and Start Services

```bash
systemctl daemon-reload
systemctl enable mclaw-gateway
systemctl enable mclaw-heartbeat
systemctl start mclaw-gateway
systemctl start mclaw-heartbeat
```

### Step 6: Register with Dispatcher

```bash
curl -X POST https://ml.ovh139.aliases.me/register \
  -H "Content-Type: application/json" \
  -d '{
    "machine_name": "client2",
    "url": "http://51.255.93.22:42618",
    "auth_token": "YOUR_BOT_TOKEN",
    "description": "Remote production server",
    "default": false
  }'
```

### Step 7: Verify Registration

```bash
# Check dispatcher machine list
curl https://ml.ovh139.aliases.me/admin/machines | jq .
```

## Using the Dispatcher

Once your client is registered, send commands via Telegram:

```
@client2 uptime              # Check uptime on client2
@client2 df -h               # Check disk space
@all systemctl status mclaw  # Run on all machines
@list                        # List all machines
```

## Troubleshooting

### Gateway not starting

```bash
journalctl -u mclaw-gateway -n 50
# Check if port 42618 is available
ss -tlnp | grep 42618
```

### Not registered with dispatcher

```bash
# Check heartbeat logs
journalctl -u mclaw-heartbeat -n 50

# Manually send heartbeat
curl -X POST http://ns3366383.ip-37-187-77.eu:42619/heartbeat \
  -H "Content-Type: application/json" \
  -d '{"machine_name": "client2", "url": "http://51.255.93.22:42618"}'
```

### Commands not routing

```bash
# Verify machine is in dispatcher registry
curl https://ml.ovh139.aliases.me/admin/machines | jq '.machines[] | select(.name == "client2")'

# Check dispatcher logs
ssh root@ns3366383.ip-37-187-77.eu journalctl -u mclaw-dispatcher -f | grep client2
```
