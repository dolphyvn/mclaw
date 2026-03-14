# MClaw Dispatcher - Deployment Complete

## Deployment Summary

The MClaw Dispatcher service has been successfully deployed and configured.

### Infrastructure

| Component | Host | Port | Notes |
|-----------|------|------|-------|
| Dispatcher Service | ns3366383.ip-37-187-77.eu | 42619 | Running as systemd service |
| Nginx Reverse Proxy | ns3366383.ip-37-187-77.eu | 443 (HTTPS) | SSL: ml.ovh139.aliases.me |
| Telegram Webhook | - | - | https://ml.ovh139.aliases.me/webhook |

### Endpoints

| Endpoint | URL | Purpose |
|----------|-----|---------|
| Health | `https://ml.ovh139.aliases.me/dispatcher-health` | Service health check |
| Machines List | `https://ml.ovh139.aliases.me/machines` | List all configured machines |
| Admin Machines | `https://ml.ovh139.aliases.me/admin/machines` | List machines with health status |
| Dispatch API | `https://ml.ovh139.aliases.me/dispatch` | Direct API access (for testing) |
| Telegram Webhook | `https://ml.ovh139.aliases.me/webhook` | Telegram bot integration |
| Register Machine | `https://ml.ovh139.aliases.me/register` | Dynamic machine registration (POST) |
| Unregister Machine | `https://ml.ovh139.aliases.me/unregister` | Unregister a machine (POST) |
| Heartbeat | `https://ml.ovh139.aliases.me/heartbeat` | Send machine heartbeat (POST) |

### Configured Machines

| Name | URL | Default | Description |
|------|-----|---------|-------------|
| client1 | http://localhost:42618 | Yes | Local machine (gateway) |
| client2 | http://51.255.93.22:42618 | No | Remote production server |

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

- **Bot Token**: `8341625895:AAGV6hZi0NWWwSuNg2pL1q5Z9ClyK1f3feE`
- **Allowed Users**: `@POs3id0nn`
- **Webhook**: `https://ml.ovh139.aliases.me/webhook` (Active)

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
ssh root@ns3366383.ip-37-187-77.eu systemctl status mclaw-dispatcher

# View logs
ssh root@ns3366383.ip-37-187-77.eu journalctl -u mclaw-dispatcher -f

# Restart dispatcher
ssh root@ns3366383.ip-37-187-77.eu systemctl restart mclaw-dispatcher

# Reload nginx
ssh root@ns3366383.ip-37-187-77.eu systemctl reload nginx
```

### Dynamic Machine Registration

The dispatcher supports dynamic machine registration, allowing new machines to register themselves without manually editing `machines.toml`.

#### Registration API

**Register a new machine:**
```bash
curl -X POST https://ml.ovh139.aliases.me/register \
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
curl -X POST https://ml.ovh139.aliases.me/heartbeat \
  -H "Content-Type: application/json" \
  -d '{
    "machine_name": "new-client",
    "url": "http://new-client:42618"
  }'
```

**Unregister a machine:**
```bash
curl -X POST https://ml.ovh139.aliases.me/unregister \
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
endpoint = "http://ns3366383.ip-37-187-77.eu:42619"
machine_name = "client3"
auth_token = "YOUR_BOT_TOKEN"
description = "Auto-registered client"
default = false
registration_interval_secs = 30
```

### Next Steps

To fully utilize the dispatcher:

1. **Start MClaw gateway on client1** (gateway server itself):
   ```bash
   mclaw gateway start
   ```

2. **Install and start MClaw on client2** (51.255.93.22):
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

- Only `@POs3id0nn` is authorized to use the bot
- All machines are accessed via localhost on gateway (client1)
- Client2 requires SSH tunnel or direct network access
- Consider setting up pairing tokens for production use
