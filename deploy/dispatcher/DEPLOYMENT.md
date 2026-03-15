# MClaw Dispatcher - Deployment Complete

## Deployment Summary

The MClaw Dispatcher service has been successfully deployed and configured.

### Infrastructure

| Component | Host | Port | Notes |
|-----------|------|------|-------|
| Dispatcher Service | your-gateway-server.example.com | 42619 | Running as systemd service |
| Multi-Tenant Gateway | your-gateway-server.example.com | 42620 | Centralized LLM API management |
| Nginx Reverse Proxy | your-gateway-server.example.com | 443 (HTTPS) | SSL: your-domain.example.com |
| Telegram Webhook | - | - | https://your-domain.example.com/webhook |

### Endpoints

| Endpoint | URL | Purpose |
|----------|-----|---------|
| Health | `https://your-domain.example.com/dispatcher-health` | Service health check |
| Machines List | `https://your-domain.example.com/machines` | List all configured machines |
| Admin Machines | `https://your-domain.example.com/admin/machines` | List machines with health status |
| Dispatch API | `https://your-domain.example.com/dispatch` | Direct API access (for testing) |
| Telegram Webhook | `https://your-domain.example.com/webhook` | Telegram bot integration |
| Register Machine | `https://your-domain.example.com/register` | Dynamic machine registration (POST) |
| Unregister Machine | `https://your-domain.example.com/unregister` | Unregister a machine (POST) |
| Heartbeat | `https://your-domain.example.com/heartbeat` | Send machine heartbeat (POST) |
| MT Health | `https://your-domain.example.com/mt-health` | Multi-tenant gateway health |
| MT Clients | `https://your-domain.example.com/api/v1/clients` | List multi-tenant clients |
| MT Chat API | `https://your-domain.example.com/api/v1/chat` | Multi-tenant chat completions |

### Configured Machines

| Name | URL | Default | Description |
|------|-----|---------|-------------|
| client1 | http://localhost:42618 | Yes | Local machine (gateway) |
| client2 | http://your-client-server.example.com:42618 | No | Remote production server |

### Command Syntax

```
@client1 uptime        # Run on client1 (gateway)
@client2 df -h         # Run on client2 (remote)
@all uptime            # Run on ALL machines
@list                  # List all machines
uptime                 # Run on default machine (client1)
```

### Configuration Files

- **Dispatcher config**: `/etc/mclaw/dispatcher.toml`
- **Machines registry**: `/etc/mclaw/machines.toml`
- **Systemd service**: `/etc/systemd/system/mclaw-dispatcher.service`
- **Nginx config**: `/etc/nginx/sites-available/dispatcher.conf`

### Telegram Bot

- **Bot Token**: `YOUR_BOT_TOKEN` (from @BotFather)
- **Allowed Users**: `@YOUR_TELEGRAM_USERNAME` (your Telegram username)
- **Webhook**: `https://your-domain.example.com/webhook` (Active)

### Telegram Usage

Send a message to your bot with any of these commands:

```
@list                    # List available machines
@client1 uptime          # Check uptime on gateway
@client2 free -h         # Check memory on remote server
@all df -h               # Check disk space on all machines
@list                    # List configured machines
```

### Service Management

```bash
# Check dispatcher status
ssh root@your-gateway-server systemctl status mclaw-dispatcher

# View logs
ssh root@your-gateway-server journalctl -u mclaw-dispatcher -f

# Restart dispatcher
ssh root@your-gateway-server systemctl restart mclaw-dispatcher

# Reload nginx
ssh root@your-gateway-server systemctl reload nginx
```

### Dynamic Machine Registration

The dispatcher supports dynamic machine registration, allowing new machines to register themselves without manually editing `machines.toml`.

#### Registration API

**Register a new machine:**
```bash
curl -X POST https://your-domain.example.com/register \
  -H "Content-Type: application/json" \
  -d '{
    "machine_name": "new-client",
    "url": "http://new-client:42618",
    "auth_token": "YOUR_BOT_TOKEN",
    "description": "New production server",
    "default": false
  }'
```

**Send heartbeat (to keep registration active):**
```bash
curl -X POST https://your-domain.example.com/heartbeat \
  -H "Content-Type: application/json" \
  -d '{
    "machine_name": "new-client",
    "url": "http://new-client:42618"
  }'
```

**Unregister a machine:**
```bash
curl -X POST https://your-domain.example.com/unregister \
  -H "Content-Type: application/json" \
  -d '{
    "machine_name": "new-client",
    "auth_token": "YOUR_BOT_TOKEN"
  }'
```

#### Notes

- **Auth Token**: Use the same bot token as configured in `dispatcher.toml`
- **Heartbeat Timeout**: Machines without heartbeat for 60 seconds are marked as stale and automatically removed
- **Static vs Dynamic**: Machines in `machines.toml` are static and never removed due to timeout

#### Client-Side Configuration

MClaw clients can be configured to auto-register with the dispatcher by adding to `config.toml`:

```toml
[dispatcher]
enabled = true
endpoint = "http://your-gateway-server.example.com:42619"
machine_name = "client3"
auth_token = "YOUR_BOT_TOKEN"
description = "Auto-registered client"
default = false
registration_interval_secs = 30
```

### Multi-Tenant Gateway

The Multi-Tenant Gateway provides centralized LLM API management for multiple clients. It allows different applications/services to share LLM providers with isolated configurations.

#### Features

- **Client Isolation**: Each client has independent provider/model configuration
- **Multiple Providers**: OpenRouter, OpenAI, GLM, OpenAI Codex supported
- **OAuth Support**: Supports API key and OAuth authentication profiles
- **Rate Limiting**: Per-client rate limiting (optional)

#### Usage

**List all configured clients:**
```bash
curl https://your-domain.example.com/api/v1/clients | jq .
```

**Send chat completion request:**
```bash
curl -X POST https://your-domain.example.com/api/v1/chat \
  -H "Content-Type: application/json" \
  -d '{
    "client_id": "client1",
    "messages": [{"role": "user", "content": "Hello!"}]
  }'
```

**Health check:**
```bash
curl https://your-domain.example.com/mt-health | jq .
```

#### Configuration

Multi-tenant clients are configured in `/etc/mclaw/multi_tenant.toml`:

```toml
[[groups]]
client_id = "client1"
client_secret = ""
provider = "openrouter"
model = "anthropic/claude-sonnet-4.6"
api_key = "sk-or-..."

[[groups]]
client_id = "client2"
client_secret = ""
provider = "openrouter"
model = "anthropic/claude-sonnet-4.6"
api_key = "sk-or-..."
```

#### Systemd Service

The multi-tenant gateway runs as:
```bash
/usr/local/bin/mclaw gateway-server --config-dir /etc/mclaw --port 42620 --host 0.0.0.0
```

Service file: `/etc/systemd/system/mclaw-multi-tenant-gateway.service`

### Next Steps

To fully utilize the dispatcher:

1. **Start MClaw gateway on client1** (gateway server itself):
   ```bash
   mclaw gateway start
   ```

2. **Install and start MClaw on client2** (your-client-server.example.com):
   ```bash
   # Copy config and install mclaw
   # Start gateway server
   mclaw gateway start
   ```

3. **Test via Telegram**:
   - Send `@list` to your bot
   - Send `@client1 echo hello` to test gateway machine
   - Send `@client2 echo hello` to test remote machine

### Security Notes

- Only `@YOUR_TELEGRAM_USERNAME` is authorized to use the bot
- All machines are accessed via localhost on gateway (client1)
- Client2 requires SSH tunnel or direct network access
- Consider setting up pairing tokens for production use
