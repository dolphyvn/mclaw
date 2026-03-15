# MClaw Dispatcher - Multi-Machine Router Design

## Overview

The dispatcher is a centralized routing service that enables one Telegram bot to manage multiple MClaw client instances across different machines.

## Architecture

### WebSocket Mode (Recommended - NAT-Friendly)

```
┌─────────────────┐     ┌──────────────────────────────────────┐
│  Telegram Bot   │────▶│  Dispatcher Service                  │
│  (one bot)      │     │  Host: gateway.example.com           │
└─────────────────┘     │  Port: 42619                         │
                        │  WebSocket: /ws/connect              │
                        └──────────────────────────────────────┘
                                          │
                    WebSocket connections (outbound from clients)
                    ┌─────────────────────┼─────────────────────┐
                    │                     │                     │
                    ▼                     ▼                     ▼
           ┌────────────────┐    ┌────────────────┐    ┌────────────────┐
           │  Client 1      │    │  Client 2      │    │  Client N      │
           │  (local)       │    │  Behind NAT   │    │  ...           │
           │  Connects TO   │    │  Connects TO   │    │  Connects TO   │
           │  Dispatcher    │    │  Dispatcher    │    │  Dispatcher    │
           │  via WS        │    │  via WS        │    │  via WS        │
           │  machine_name: │    │  machine_name: │    │  machine_name: │
           │  "client1"     │    │  "client2"     │    │  "clientN"     │
           └────────────────┘    └────────────────┘    └────────────────┘
```

**Key advantages of WebSocket mode:**
- Clients connect TO dispatcher (no public IP needed)
- Works behind NAT/firewall
- Automatic reconnection on disconnect
- No inbound ports needed on clients

## Command Syntax

| Format | Target | Example |
|--------|--------|---------|
| `@machine_name command` | Specific machine | `@client1 uptime` |
| `@all command` | All machines | `@all df -h` |
| `command` (no prefix) | Default machine | `uptime` |
| `@list` | List all machines | `@list` |

## Dispatcher API

### POST /dispatch
Routes a command to target machine(s).

**Request:**
```json
{
  "source": "telegram",
  "chat_id": "-100123456",
  "user": "@POs3id0nn",
  "message": "@client1 uptime"
}
```

**Response (single machine):**
```json
{
  "target": "client1",
  "response": " 14:32:05 up 10 days..."
}
```

**Response (@all):**
```json
{
  "responses": [
    {"machine": "client1", "response": "..."},
    {"machine": "client2", "response": "..."}
  ]
}
```

### GET /machines
List all registered machines.

### GET /health
Health check endpoint.

## Machine Registry

**Note:** With WebSocket mode, machines auto-register when they connect. The `machines.toml` file is optional.

```toml
# /etc/mclaw/machines.toml - Optional pre-configuration
# WebSocket clients auto-register; this file provides:
# - Persistence across restarts
# - Token-based authentication
# - Default machine designation

[[machines]]
name = "client1"
url = "http://localhost:42618"  # Optional for WebSocket clients
token = "pairing_token_if_required"  # Auth token (optional)
default = true
description = "Local machine"

[[machines]]
name = "client2"
# url = "http://51.255.93.22:42618"  # Not needed for WebSocket
token = "pairing_token_if_required"
default = false
description = "Remote NAT client"
```

## Message Flow

### WebSocket Mode

1. User sends message to Telegram bot
2. Telegram webhook → Dispatcher `/webhook`
3. Dispatcher parses message for `@machine` prefix
4. Dispatcher sends command via WebSocket to connected client
5. MClaw client processes command
6. Client sends result back via WebSocket
7. Dispatcher sends reply to Telegram

### Connection Flow

1. Dispatcher starts listening on `/ws/connect`
2. MClaw client connects with: `ws://dispatcher:42619/ws/connect?machine_name=X&token=Y`
3. Dispatcher authenticates client (token optional for auto-registration)
4. If machine unknown, dispatcher auto-registers it
5. Client stays connected, ready to receive commands

## Configuration

Dispatcher config: `/etc/mclaw/dispatcher.toml`

```toml
[server]
host = "0.0.0.0"
port = 42619

[telegram]
bot_token = "YOUR_BOT_TOKEN"
webhook_url = "https://your-domain.com/webhook"

[machines_file]
path = "/etc/mclaw/machines.toml"

[logging]
level = "info"
file = "/var/log/mclaw/dispatcher.log"
```

## Client Configuration

### WebSocket Mode (Recommended)

Each MClaw client connects TO the dispatcher:

```toml
[channels_config.telegram]
bot_token = ""  # Empty - handled by dispatcher
enabled = false  # Direct Telegram disabled

[dispatcher]
enabled = true
mode = "ws"  # WebSocket mode
machine_name = "client1"  # Unique name for this machine
endpoint = "ws://dispatcher.example.com:42619"  # Dispatcher URL
auth_token = "optional_token"  # If pre-configured in machines.toml
reconnect_interval_secs = 5
```

### HTTP Mode (Alternative)

For machines with public IP (dispatcher connects TO client):

```toml
[channels_config.telegram]
bot_token = ""
enabled = false

[dispatcher]
enabled = true
mode = "register"  # HTTP polling mode
machine_name = "client1"
dispatcher_url = "http://dispatcher:42619"
pairing_token = "generated_token"
```

## Security

1. **Auth**: Dispatcher uses pairing tokens to connect to clients
2. **Rate limiting**: Per-user and per-machine limits
3. **Allowed users**: Same as Telegram channel config
4. **TLS**: Use HTTPS for webhook in production

## Deployment

### On Gateway Server (with public IP)

```bash
# Install dispatcher
cargo install mclaw-dispatcher

# Create config
mkdir -p /etc/mclaw
cp dispatcher.toml /etc/mclaw/

# Create systemd service
sudo tee /etc/systemd/system/mclaw-dispatcher.service > /dev/null <<'EOF'
[Unit]
Description=MClaw Dispatcher Service
After=network.target

[Service]
Type=simple
User=mclaw
ExecStart=/usr/local/bin/mclaw-dispatcher --config /etc/mclaw/dispatcher.toml
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
EOF

sudo systemctl daemon-reload
sudo systemctl enable mclaw-dispatcher
sudo systemctl start mclaw-dispatcher
```

### On Each Client Machine

```bash
# Install MClaw
curl -fsSL https://raw.githubusercontent.com/zeroclaw-labs/mclaw/main/install.sh | bash

# Configure to connect to dispatcher
mclaw connect-dispatcher \
  --endpoint ws://dispatcher.example.com:42619 \
  --machine-name $(hostname) \
  --token optional_token

# Or edit config manually
nano ~/.mclaw/config.toml

# Start MClaw daemon
mclaw daemon

# Or as a service
mclaw service install
mclaw service start
```

## Implementation Components

1. **dispatcher crate** (`/src/dispatcher/`)
   - `mod.rs` - Main entry point, WebSocket upgrade handler
   - `config.rs` - Configuration parsing
   - `router.rs` - Command routing logic (WebSocket + HTTP)
   - `telegram.rs` - Telegram webhook handler
   - `ws_server.rs` - WebSocket server for client connections
   - `connector.rs` - HTTP client for fallback
   - `machines.rs` - Machine registry with auto-registration
   - `client_register.rs` - Client registration types

2. **gateway/dispatcher_mode.rs** - Client-side WebSocket connector
   - Connects TO dispatcher via WebSocket
   - Handles command execution and responses
   - Automatic reconnection

3. **MClaw client config**
   - `dispatcher.mode` - "ws" or "register"
   - `dispatcher.endpoint` - WebSocket URL
   - `dispatcher.machine_name` - Unique identifier
   - `dispatcher.auth_token` - Optional authentication

4. **Documentation**
   - [dispatcher-guide.md](dispatcher-guide.md) - User guide
   - [multi-machine-setup.md](multi-machine-setup.md) - Tutorial
   - [dispatcher-design.md](dispatcher-design.md) - This file
