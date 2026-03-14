# MClaw Dispatcher - Multi-Machine Router Design

## Overview

The dispatcher is a centralized routing service that enables one Telegram bot to manage multiple MClaw client instances across different machines.

## Architecture

```
┌─────────────────┐     ┌──────────────────────────────────────┐
│  Telegram Bot   │────▶│  Dispatcher Service                  │
│  (one bot)      │     │  Host: ns3366383.ip-37-187-77.eu     │
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
           │  "client1"     │    │  "client2"     │    │  "clientN"     │
           └────────────────┘    └────────────────┘    └────────────────┘
```

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

```toml
[machines]
  [machines.client1]
  name = "client1"
  url = "http://localhost:42618"
  token = "pairing_token_if_required"
  default = true

  [machines.client2]
  name = "client2"
  url = "http://51.255.93.22:42618"
  token = "pairing_token_if_required"
  default = false
```

## Message Flow

1. User sends message to Telegram bot
2. Telegram webhook → Dispatcher `/webhook`
3. Dispatcher parses message for `@machine` prefix
4. Dispatcher forwards to MClaw client via WebSocket `/ws/chat`
5. MClaw client processes command
6. Dispatcher receives response
7. Dispatcher sends reply to Telegram

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

Each MClaw client needs:

```toml
[channels_config.telegram]
bot_token = ""  # Empty - handled by dispatcher
enabled = false  # Direct Telegram disabled

[dispatcher]
enabled = true
machine_name = "client1"
```

## Security

1. **Auth**: Dispatcher uses pairing tokens to connect to clients
2. **Rate limiting**: Per-user and per-machine limits
3. **Allowed users**: Same as Telegram channel config
4. **TLS**: Use HTTPS for webhook in production

## Deployment

### On Gateway Server (ns3366383.ip-37-187-77.eu)

```bash
# Install dispatcher
cargo install mclaw-dispatcher

# Create config
mkdir -p /etc/mclaw
cp dispatcher.toml /etc/mclaw/

# Create systemd service
cp mclaw-dispatcher.service /etc/systemd/system/
systemctl daemon-reload
systemctl enable mclaw-dispatcher
systemctl start mclaw-dispatcher
```

### On Each Client Machine

```bash
# Enable gateway server (if not already)
mclaw --gateway

# Pair with dispatcher (if required)
mclaw --pair <pairing_code>
```

## Implementation Components

1. **dispatcher crate** (`/src/dispatcher/`)
   - `mod.rs` - Main entry point
   - `config.rs` - Configuration parsing
   - `router.rs` - Command routing logic
   - `telegram.rs` - Telegram webhook handler
   - `client.rs` - WebSocket client for MClaw connection
   - `machines.rs` - Machine registry

2. **MClaw client changes**
   - Add `machine_name` to config schema
   - Add dispatcher config section
   - Allow empty Telegram config when dispatcher enabled

3. **Documentation**
   - Setup guide
   - Configuration reference
   - Troubleshooting
