# MClaw - Multi-Tenant AI Agent Runtime

**MClaw** is a multi-tenant fork of [ZeroClaw](https://github.com/zeroclaw-labs/zeroclaw) with centralized LLM gateway and multi-machine dispatcher capabilities.

## What is MClaw?

MClaw is a Rust-first autonomous agent runtime that adds **multi-machine management** and **multi-tenant LLM gateway** functionality to ZeroClaw. It allows you to:

- **Manage multiple machines** from a single Telegram bot interface
- **Centralize LLM API keys** on one secure gateway server
- **Route requests by client identity** - different groups use different providers
- **Run distributed agent workloads** across multiple servers

### Key Features

- **All ZeroClaw features**: Agent orchestration, channels (Telegram, Discord, Slack, etc.), tools, memory, cron, hardware peripherals
- **Multi-Machine Dispatcher**: Route commands to multiple MClaw instances via Telegram
- **Dynamic machine registration**: Clients auto-register and send heartbeats
- **Multi-tenant LLM gateway**: Centralize LLM API keys and route requests by client identity
- **Client authentication**: Secure Bearer token or Basic auth per client group
- **Provider routing**: Different groups use different LLM providers (OpenRouter, OpenAI, Anthropic, GLM, Ollama, etc.)

## Architecture Overview

```
┌───────────────────────────────────────────────────────────────────────────┐
│                         MClaw Complete Architecture                       │
├───────────────────────────────────────────────────────────────────────────┤
│                                                                           │
│  ┌─────────────────────────────────────────────────────────────────────┐ │
│  │                      Telegram Bot Interface                         │ │
│  │                    (@your_bot commands)                             │ │
│  └───────────────────────────────┬─────────────────────────────────────┘ │
│                                  │                                        │
│  ┌───────────────────────────────▼─────────────────────────────────────┐ │
│  │                     MClaw Dispatcher (Port 42619)                   │ │
│  │  • Routes @machine commands to registered clients                  │ │
│  │  • Dynamic machine registration & heartbeat monitoring             │ │
│  │  • WebSocket connections to MClaw gateways                         │ │
│  └───────────────┬─────────────────────────────────┬───────────────────┘ │
│                  │                                 │                      │
│         ┌────────▼────────┐              ┌────────▼────────┐             │
│         │  Machine:       │              │  Machine:       │             │
│         │  client1        │              │  client2        │             │
│         │  (localhost)    │              │  (remote)       │             │
│         │  Port: 42618    │              │  Port: 42618    │             │
│         └────────┬────────┘              └────────┬────────┘             │
│                  │                                 │                      │
│  ┌───────────────▼─────────────────────────────────▼───────────────────┐ │
│  │              Multi-Tenant Gateway (Port 42620)                      │ │
│  │  • Centralized LLM API management                                   │ │
│  │  • Client authentication & rate limiting                            │ │
│  │  • Provider routing (OpenRouter, OpenAI, GLM, etc.)                │ │
│  └───────────────────────────┬───────────────────────────────────────┘ │
│                              │                                            │
│         ┌────────────────────┼────────────────────┐                      │
│         ▼                    ▼                    ▼                      │
│  ┌──────────┐         ┌──────────┐         ┌──────────┐                 │
│  │ Group A  │         │ Group B  │         │ Group C  │                 │
│  │OpenRouter│         │ ChatGPT  │         │   GLM    │                 │
│  └──────────┘         └──────────┘         └──────────┘                 │
│                                                                          │
│  Access via: https://ml.ovh139.aliases.me (Nginx HTTPS Reverse Proxy)    │
└──────────────────────────────────────────────────────────────────────────┘
```

## Component Comparison

| Component | Port | Purpose |
|-----------|------|---------|
| **Dispatcher** | 42619 | Routes Telegram commands to multiple machines |
| **Gateway** | 42618 | Executes commands on a single machine |
| **Multi-Tenant Gateway** | 42620 | Centralized LLM API for multiple clients |

## Complete Setup Guide

This guide walks you through setting up the complete MClaw infrastructure from scratch, including:

1. Building MClaw
2. Setting up the Dispatcher (multi-machine management)
3. Setting up the Multi-Tenant Gateway (centralized LLM API)
4. Configuring client machines
5. Setting up Nginx with HTTPS
6. Running as systemd services

---

## Part 1: Build MClaw

### Prerequisites

- Rust 1.70+ (install via [rustup](https://rustup.rs/))
- Git
- Linux server (Ubuntu/Debian recommended)

### Build from source

```bash
# Clone the repository
git clone https://github.com/dolphyvn/mclaw.git
cd mclaw

# Build release binary
cargo build --release

# The binary will be at: ./target/release/mclaw
sudo cp target/release/mclaw /usr/local/bin/mclaw
sudo chmod +x /usr/local/bin/mclaw
```

---

## Part 2: Dispatcher Setup (Multi-Machine Management)

The Dispatcher allows you to manage multiple MClaw machines from a single Telegram bot.

### Step 1: Create Telegram Bot

1. Open Telegram and search for `@BotFather`
2. Send `/newbot` and follow the prompts
3. Save the bot token (format: `123456789:ABCdefGHIjklMNOpqrsTUVwxyz`)
4. Note your Telegram username for authorization

### Step 2: Configure Dispatcher

Create `/etc/mclaw/dispatcher.toml`:

```toml
[server]
host = "127.0.0.1"
port = 42619

[telegram]
bot_token = "YOUR_BOT_TOKEN_HERE"
allowed_users = ["@YOUR_TELEGRAM_USERNAME"]
webhook_url = ""  # Set later after nginx is configured

[machines]
path = "/etc/mclaw/machines.toml"
```

Create `/etc/mclaw/machines.toml`:

```toml
# Static machine configurations
# Machines can also register dynamically via /register endpoint

[[machines]]
name = "client1"
url = "http://localhost:42618"
default = true
description = "Local gateway machine"
```

### Step 3: Start Dispatcher (for testing)

```bash
# Create config directory
sudo mkdir -p /etc/mclaw

# Test run
/usr/local/bin/mclaw dispatcher --config /etc/mclaw/dispatcher.toml
```

---

## Part 3: Multi-Tenant Gateway Setup

The Multi-Tenant Gateway centralizes all LLM API keys.

### Step 1: Generate Client Secrets

```bash
# Generate secret for each client group
mclaw generate-secret client1
mclaw generate-secret client2
mclaw generate-secret group-a
mclaw generate-secret group-b
```

### Step 2: Configure Gateway

Create `/etc/mclaw/multi_tenant.toml`:

```toml
# Client 1 - Uses OpenRouter
[[groups]]
client_id = "client1"
client_secret = "mc_client1_..."  # From generate-secret output
provider = "openrouter"
model = "anthropic/claude-sonnet-4.6"
api_key = "sk-or-v1-YOUR_OPENROUTER_KEY"

# Client 2 - Uses OpenRouter
[[groups]]
client_id = "client2"
client_secret = "mc_client2_..."
provider = "openrouter"
model = "anthropic/claude-sonnet-4.6"
api_key = "sk-or-v1-YOUR_OPENROUTER_KEY"

# Group A - Free model
[[groups]]
client_id = "group-a"
client_secret = "mc_group-a_..."
provider = "openrouter"
model = "nvidia/nemotron-3-nano-30b-a3b:free"
api_key = "sk-or-v1-YOUR_OPENROUTER_KEY"

# Group B - OpenAI
[[groups]]
client_id = "group-b"
client_secret = "mc_group-b_..."
provider = "openai"
model = "gpt-4o"
api_key = "sk-proj-YOUR_OPENAI_KEY"

# Group C - GLM
[[groups]]
client_id = "group-c"
client_secret = "mc_group-c_..."
provider = "glm"
model = "glm-4"
api_key = "your-id.secret"

# ChatGPT Plus (OAuth)
[[groups]]
client_id = "chatgpt-plus"
client_secret = "mc_chatgpt-plus_..."
provider = "openai-codex"
model = "gpt-4o"
auth_profile = "chatgpt-plus"
```

### Step 3: Setup OAuth for ChatGPT Plus (Optional)

```bash
# Login to OpenAI OAuth on the gateway server
mclaw auth login openai --profile chatgpt-plus
```

Follow the browser prompts to authenticate with your ChatGPT account.

### Step 4: Test Multi-Tenant Gateway

```bash
# Start the gateway server
mclaw gateway-server --config-dir /etc/mclaw --port 42620 --host 0.0.0.0
```

Test the endpoints:
```bash
# Health check
curl http://localhost:42620/health

# List clients
curl http://localhost:42620/api/v1/clients

# Chat completion (for client1)
curl -X POST http://localhost:42620/api/v1/chat \
  -H "Content-Type: application/json" \
  -d '{
    "client_id": "client1",
    "messages": [{"role": "user", "content": "Hello!"}]
  }'
```

---

## Part 4: Client Machine Setup

Each client machine runs a MClaw Gateway that executes commands.

### Option A: Local Client (client1 on dispatcher host)

Create `/root/.mclaw/config.toml`:

```toml
[dispatcher]
enabled = true
machine_name = "client1"
endpoint = "http://127.0.0.1:42619"
auth_token = "YOUR_BOT_TOKEN"
description = "Local gateway machine"
default = true

[gateway]
enabled = true
host = "127.0.0.1"
port = 42618
allow_public_bind = false
```

### Option B: Remote Client (client2 on separate server)

1. Build and install MClaw on the remote server:
```bash
# On remote server (51.255.93.22 example)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source ~/.cargo/env

git clone https://github.com/dolphyvn/mclaw.git /opt/mclaw
cd /opt/mclaw
cargo build --release
sudo cp target/release/mclaw /usr/local/bin/mclaw
sudo chmod +x /usr/local/bin/mclaw
```

2. Create `/root/.mclaw/config.toml`:
```toml
[dispatcher]
enabled = true
machine_name = "client2"
endpoint = "http://ns3366383.ip-37-187-77.eu:42619"
auth_token = "YOUR_BOT_TOKEN"
description = "Remote production server"
default = false

[gateway]
enabled = true
host = "0.0.0.0"  # Bind to all interfaces for external access
port = 42618
allow_public_bind = true
```

3. Create heartbeat script `/usr/local/bin/mclaw-heartbeat.sh`:
```bash
#!/bin/bash
while true; do
  sleep 30
  curl -s -X POST http://ns3366383.ip-37-187-77.eu:42619/heartbeat \
    -H 'Content-Type: application/json' \
    -d '{"machine_name": "client2", "url": "http://51.255.93.22:42618"}'
done
```

```bash
sudo chmod +x /usr/local/bin/mclaw-heartbeat.sh
```

4. Register with dispatcher:
```bash
curl -X POST http://ns3366383.ip-37-187-77.eu:42619/register \
  -H "Content-Type: application/json" \
  -d '{
    "machine_name": "client2",
    "url": "http://51.255.93.22:42618",
    "auth_token": "YOUR_BOT_TOKEN",
    "description": "Remote production server",
    "default": false
  }'
```

### Step 5: Configure Client to Use Multi-Tenant Gateway

**IMPORTANT**: By default, clients use their own LLM provider configuration. To use the centralized Multi-Tenant Gateway instead, add this to the client's `/root/.mclaw/config.toml`:

#### For client1 (local machine):

```toml
[dispatcher]
enabled = true
machine_name = "client1"
endpoint = "http://127.0.0.1:42619"
auth_token = "YOUR_BOT_TOKEN"
description = "Local gateway machine"
default = true

[gateway]
enabled = true
host = "127.0.0.1"
port = 42618
allow_public_bind = false

# === Configure to use Multi-Tenant Gateway ===
# This tells the agent to use the centralized gateway instead of local provider
[providers.mclaw]
type = "mclaw"
gateway_url = "http://127.0.0.1:42620"  # Multi-Tenant Gateway URL
client_id = "client1"                   # Your assigned group/client ID
client_secret = "mc_client1_..."         # Your secret from generate-secret

# Use mclaw as the default model
[default_model]
name = "mclaw"
type = "mclaw"
```

#### For client2 (remote machine):

```toml
[dispatcher]
enabled = true
machine_name = "client2"
endpoint = "http://ns3366383.ip-37-187-77.eu:42619"
auth_token = "YOUR_BOT_TOKEN"
description = "Remote production server"
default = false

[gateway]
enabled = true
host = "0.0.0.0"
port = 42618
allow_public_bind = true

# === Configure to use Multi-Tenant Gateway ===
# Point to the remote Multi-Tenant Gateway
[providers.mclaw]
type = "mclaw"
gateway_url = "http://ns3366383.ip-37-187-77.eu:42620"
client_id = "client2"
client_secret = "mc_client2_..."

[default_model]
name = "mclaw"
type = "mclaw"
```

### How It Works

```
┌─────────────────────────────────────────────────────────────┐
│                    Client Machine (client2)                  │
│                                                             │
│  MClaw Agent Daemon                                         │
│     │                                                       │
│     │  Needs LLM for reasoning                              │
│     ▼                                                       │
│  [providers.mclaw]                                          │
│     │                                                       │
│     │  HTTP POST with client_id + client_secret            │
│     ▼                                                       │
│  Multi-Tenant Gateway (ns3366383:42620)                     │
│     │                                                       │
│     │  Validates credentials, routes to provider            │
│     ▼                                                       │
│  OpenRouter / OpenAI / GLM (as configured for client2)      │
└─────────────────────────────────────────────────────────────┘
```

### Client-Provider Mapping

Each client can be assigned to different provider groups:

| Client | client_id | Provider | Model |
|--------|-----------|----------|-------|
| client1 | `client1` | OpenRouter | Claude Sonnet 4.6 |
| client2 | `client2` | OpenRouter | Claude Sonnet 4.6 |
| group-a | `group-a` | OpenRouter | Nemotron (free) |
| group-b | `group-b` | OpenAI | GPT-4o |
| chatgpt-plus | `chatgpt-plus` | OpenAI Codex (OAuth) | GPT-4o |

### Alternative: Direct Provider Configuration

If you want a client to use a provider directly (bypassing the Multi-Tenant Gateway):

```toml
# Direct OpenRouter configuration (no gateway needed)
[providers.openrouter]
type = "openrouter"
api_key = "sk-or-v1-YOUR_KEY"

[default_model]
name = "anthropic/claude-sonnet-4.6"
type = "openrouter"
```

---

## Part 5: Systemd Services

Create systemd services for all components.

### Dispatcher Service

`/etc/systemd/system/mclaw-dispatcher.service`:

```ini
[Unit]
Description=MClaw Dispatcher - Multi-machine command router
After=network.target

[Service]
Type=simple
User=root
WorkingDirectory=/root
Environment="RUST_LOG=info"
ExecStart=/usr/local/bin/mclaw dispatcher --config /etc/mclaw/dispatcher.toml
Restart=on-failure
RestartSec=10s

[Install]
WantedBy=multi-user.target
```

### Gateway Service (client1)

`/etc/systemd/system/mclaw-gateway.service`:

```ini
[Unit]
Description=MClaw Gateway - Command executor
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

### Multi-Tenant Gateway Service

`/etc/systemd/system/mclaw-multi-tenant-gateway.service`:

```ini
[Unit]
Description=MClaw Multi-Tenant Gateway - Centralized LLM API
After=network.target

[Service]
Type=simple
User=root
WorkingDirectory=/root
Environment="RUST_LOG=info"
ExecStart=/usr/local/bin/mclaw gateway-server --config-dir /etc/mclaw --port 42620 --host 0.0.0.0
Restart=on-failure
RestartSec=10s

[Install]
WantedBy=multi-user.target
```

### Heartbeat Service (for remote clients)

`/etc/systemd/system/mclaw-heartbeat.service`:

```ini
[Unit]
Description=MClaw Dispatcher Heartbeat
After=network.target

[Service]
Type=simple
User=root
ExecStart=/usr/local/bin/mclaw-heartbeat.sh
Restart=on-failure

[Install]
WantedBy=multi-user.target
```

### Enable and Start Services

```bash
# Reload systemd
sudo systemctl daemon-reload

# Enable services (start on boot)
sudo systemctl enable mclaw-dispatcher
sudo systemctl enable mclaw-gateway
sudo systemctl enable mclaw-multi-tenant-gateway
sudo systemctl enable mclaw-heartbeat  # On remote clients only

# Start services now
sudo systemctl start mclaw-dispatcher
sudo systemctl start mclaw-gateway
sudo systemctl start mclaw-multi-tenant-gateway
sudo systemctl start mclaw-heartbeat  # On remote clients only

# Check status
sudo systemctl status mclaw-dispatcher
sudo systemctl status mclaw-gateway
sudo systemctl status mclaw-multi-tenant-gateway
```

---

## Part 6: Nginx HTTPS Reverse Proxy

Set up Nginx for HTTPS access to all services.

### Step 1: Install Nginx and Get SSL Certificate

```bash
# Install nginx
sudo apt update
sudo apt install nginx certbot python3-certbot-nginx

# Get SSL certificate (replace with your domain)
sudo certbot --nginx -d ml.ovh139.aliases.me
```

### Step 2: Configure Nginx

`/etc/nginx/sites-available/mclaw.conf`:

```nginx
# Upstream definitions
upstream dispatcher_backend {
    server 127.0.0.1:42619 max_fails=1 fail_timeout=2s;
}

upstream multi_tenant_backend {
    server 127.0.0.1:42620 max_fails=1 fail_timeout=2s;
}

server {
    server_name ml.ovh139.aliases.me;

    # === Dispatcher Routes ===

    location /webhook {
        proxy_pass http://dispatcher_backend;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }

    location /dispatch {
        proxy_pass http://dispatcher_backend;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    }

    location /dispatcher-health {
        proxy_pass http://dispatcher_backend/health;
        proxy_set_header Host $host;
        access_log off;
    }

    location /admin {
        proxy_pass http://dispatcher_backend;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    }

    location /register {
        proxy_pass http://dispatcher_backend;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    }

    location /unregister {
        proxy_pass http://dispatcher_backend;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    }

    location /heartbeat {
        proxy_pass http://dispatcher_backend;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    }

    location /machines {
        proxy_pass http://dispatcher_backend;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    }

    # === Multi-Tenant Gateway Routes ===

    location /api/v1/chat {
        proxy_pass http://multi_tenant_backend;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_connect_timeout 120s;
        proxy_send_timeout 120s;
        proxy_read_timeout 120s;
    }

    location /api/v1/clients {
        proxy_pass http://multi_tenant_backend;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    }

    location /mt-health {
        proxy_pass http://multi_tenant_backend/health;
        proxy_set_header Host $host;
        access_log off;
    }

    listen [::]:443 ssl;
    listen 443 ssl;
    ssl_certificate /etc/letsencrypt/live/ml.ovh139.aliases.me/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/ml.ovh139.aliases.me/privkey.pem;
    include /etc/letsencrypt/options-ssl-nginx.conf;
    ssl_dhparam /etc/letsencrypt/ssl-dhparams.pem;
}

# HTTP to HTTPS redirect
server {
    if ($host = ml.ovh139.aliases.me) {
        return 301 https://$host$request_uri;
    }
    listen 80;
    listen [::]:80;
    server_name ml.ovh139.aliases.me;
    return 404;
}
```

### Step 3: Enable and Reload Nginx

```bash
# Enable site
sudo ln -s /etc/nginx/sites-available/mclaw.conf /etc/nginx/sites-enabled/

# Test configuration
sudo nginx -t

# Reload nginx
sudo systemctl reload nginx
```

### Step 4: Set Telegram Webhook

```bash
curl "https://api.telegram.org/botYOUR_BOT_TOKEN/setWebhook?url=https://ml.ovh139.aliases.me/webhook"
```

Update `/etc/mclaw/dispatcher.toml`:
```toml
[telegram]
webhook_url = "https://ml.ovh139.aliases.me/webhook"
```

Restart dispatcher:
```bash
sudo systemctl restart mclaw-dispatcher
```

---

## Part 7: Usage

### Telegram Bot Commands

Send these commands to your Telegram bot:

```
@list                           # List all machines
@client1 uptime                 # Run on client1
@client2 df -h                  # Run on client2
@all systemctl status mclaw     # Run on all machines
uptime                          # Run on default machine
```

### Multi-Tenant Gateway API

```bash
# Check health
curl https://ml.ovh139.aliases.me/mt-health

# List configured clients
curl https://ml.ovh139.aliases.me/api/v1/clients | jq .

# Send chat request
curl -X POST https://ml.ovh139.aliases.me/api/v1/chat \
  -H "Content-Type: application/json" \
  -d '{
    "client_id": "client1",
    "messages": [{"role": "user", "content": "Hello!"}]
  }'
```

### Dispatcher API

```bash
# Check health
curl https://ml.ovh139.aliases.me/dispatcher-health

# List machines with status
curl https://ml.ovh139.aliases.me/admin/machines | jq .

# Register a new machine
curl -X POST https://ml.ovh139.aliases.me/register \
  -H "Content-Type: application/json" \
  -d '{
    "machine_name": "new-client",
    "url": "http://new-client:42618",
    "auth_token": "YOUR_BOT_TOKEN",
    "description": "New server",
    "default": false
  }'
```

---

## Part 8: Service Management

### Check Service Status

```bash
# Dispatcher
sudo systemctl status mclaw-dispatcher
sudo journalctl -u mclaw-dispatcher -f

# Gateway
sudo systemctl status mclaw-gateway
sudo journalctl -u mclaw-gateway -f

# Multi-Tenant Gateway
sudo systemctl status mclaw-multi-tenant-gateway
sudo journalctl -u mclaw-multi-tenant-gateway -f

# Heartbeat (remote clients)
sudo systemctl status mclaw-heartbeat
```

### Restart Services

```bash
sudo systemctl restart mclaw-dispatcher
sudo systemctl restart mclaw-gateway
sudo systemctl restart mclaw-multi-tenant-gateway
```

### View Logs

```bash
# All mclaw logs
sudo journalctl -u "mclaw-*" -f

# Specific service
sudo journalctl -u mclaw-dispatcher -n 100
```

---

## Configuration Reference

### Supported Providers

| Provider | `provider` value | Notes |
|----------|-----------------|-------|
| OpenRouter | `openrouter` | Requires API key |
| OpenAI | `openai` | Requires API key |
| Anthropic | `anthropic` | Requires API key |
| GLM (Zhipu) | `glm` | Uses `id.secret` format |
| Ollama | `ollama` | Local, requires `api_url` |
| Gemini | `gemini` | Requires API key |
| OpenAI Codex (OAuth) | `openai-codex` | Uses `auth_profile` |
| Gemini OAuth | `gemini-oauth` | Uses `auth_profile` |

### Multi-Tenant Group Options

```toml
[[groups]]
# Required
client_id = "group-name"
client_secret = "mc_..."
provider = "openrouter"
model = "model-name"

# Authentication (use one)
api_key = "your-api-key"
# OR
auth_profile = "profile-name"  # For OAuth providers

# Optional
api_url = "https://..."
temperature = 0.7
max_tokens = 4096
rate_limit = 60
```

### Dispatcher Machine Options

```toml
[[machines]]
name = "machine-name"      # Unique identifier
url = "http://host:42618"  # Gateway URL
default = true             # Is this the default machine?
description = "Description"
# token = "..."            # Optional auth token
```

---

## Security Considerations

1. **API Key Storage**: The gateway server holds all API keys. Secure the server machine.
2. **Client Secrets**: Never share client secrets publicly.
3. **Firewall**: Restrict access to ports 42618, 42619, 42620 to trusted networks.
4. **HTTPS**: Always use HTTPS in production with valid SSL certificates.
5. **Telegram Authorization**: Only allow trusted users in `allowed_users`.
6. **Rate Limiting**: Configure per-client rate limits to prevent abuse.
7. **Heartbeat Timeout**: Machines without heartbeat for 60 seconds are removed.

---

## Commands Reference

| Command | Description |
|---------|-------------|
| `mclaw daemon` | Start the agent daemon |
| `mclaw dispatcher` | Start the dispatcher |
| `mclaw gateway-server` | Start the multi-tenant LLM gateway |
| `mclaw gateway start` | Start the command gateway |
| `mclaw generate-secret <id>` | Generate a client secret |
| `mclaw list-clients` | List gateway clients |
| `mclaw auth login <provider>` | Login to OAuth provider |
| `mclaw agent -m "message"` | Run a single agent interaction |
| `mclaw onboard` | Run the setup wizard |
| `mclaw status` | Show system status |
| `mclaw doctor` | Run diagnostics |

---

## Documentation

- [CLAUDE.md](CLAUDE.md) - Project instructions and workflow
- [README.zeroclaw.md](README.zeroclaw.md) - ZeroClaw original README
- [docs/](docs/) - Comprehensive documentation
- [docs/client-setup.md](docs/client-setup.md) - Client machine setup guide
- [deploy/dispatcher/DEPLOYMENT.md](deploy/dispatcher/DEPLOYMENT.md) - Deployment details

---

## License

MIT OR Apache-2.0 (inherits from ZeroClaw)

---

## Acknowledgments

MClaw is a fork of [ZeroClaw](https://github.com/zeroclaw-labs/zeroclaw). All credit for the core agent runtime goes to the ZeroClaw developers.
