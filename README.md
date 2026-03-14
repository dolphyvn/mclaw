# MClaw - Multi-Tenant AI Agent Runtime

**MClaw** is a multi-tenant fork of [ZeroClaw](https://github.com/zeroclaw-labs/zeroclaw) with centralized LLM gateway capabilities.

## What is MClaw?

MClaw is a Rust-first autonomous agent runtime that adds **multi-tenant LLM gateway** functionality to ZeroClaw. It allows you to:

- **Centralize LLM API keys** on one secure gateway server
- **Route requests by client identity** - different groups use different providers
- **Run multiple agent machines** without duplicating API keys on each machine

### Key Features

- **All ZeroClaw features**: Agent orchestration, channels (Telegram, Discord, Slack, etc.), tools, memory, cron, hardware peripherals
- **Multi-tenant LLM gateway**: Centralize LLM API keys and route requests by client identity
- **Client authentication**: Secure Bearer token or Basic auth per client group
- **Provider routing**: Different groups use different LLM providers (OpenRouter, OpenAI, Anthropic, GLM, Ollama, etc.)

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    MClaw Gateway Server                     │
│  (One instance holds all LLM API keys)                      │
│  Port: 42618                                                │
└──────────────────────────┬──────────────────────────────────┘
                           │
     ┌─────────────────────┼─────────────────────┐
     ▼                     ▼                     ▼
┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐
│  Group A        │ │  Group B        │ │  Group C        │
│  OpenRouter     │ │  ChatGPT        │ │  GLM            │
└────────┬────────┘ └────────┬────────┘ └────────┬────────┘
         │                   │                   │
   ┌─────┴─────┐       ┌─────┴─────┐       ┌─────┴─────┐
   │ MClaw     │       │ MClaw     │       │ MClaw     │
   │ Daemon    │       │ Daemon    │       │ Daemon    │
   │ (full     │       │ (full     │       │ (full     │
   │  agent)   │       │  agent)   │       │  agent)   │
   └───────────┘       └───────────┘       └───────────┘
```

## Complete Setup Guide

This guide walks you through setting up MClaw from scratch, including:

1. Building MClaw
2. Setting up the gateway server
3. Configuring client groups
4. Connecting client machines

---

## Part 1: Build MClaw

### Prerequisites

- Rust 1.70+ (install via [rustup](https://rustup.rs/))
- Git

### Build from source

```bash
# Clone the repository
git clone https://github.com/dolphyvn/mclaw.git
cd mclaw

# Build (use cargo build for development, cargo build --release for production)
cargo build

# The binary will be at: ./target/debug/mclaw
```

---

## Part 2: Gateway Server Setup

The gateway server is the central component that holds all LLM API keys and routes requests.

### Step 1: Generate Client Secrets

For each client group, generate a unique secret:

```bash
# Generate secret for Group A
./target/debug/mclaw generate-secret group-a

# Output example:
# Client ID: group-a
# Client Secret: mc_group-a_1a2b3c4d5e6f...
```

Repeat for each group you need:
```bash
./target/debug/mclaw generate-secret group-b
./target/debug/mclaw generate-secret group-c
```

### Step 2: Configure the Gateway

Create or edit `~/.mclaw/config.toml`:

```toml
[multi_tenant]
enabled = true
host = "0.0.0.0"   # Bind to all interfaces (use 127.0.0.1 for local only)
port = 42618

# Group A - Uses OpenRouter
[multi_tenant.groups.group-a]
client_id = "group-a"
client_secret = "mc_group-a_1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3"
provider = "openrouter"
model = "anthropic/claude-sonnet-4-20250514"
api_key = "sk-or-v1-your-openrouter-api-key"
temperature = 0.7
max_tokens = 4096
rate_limit = 60

# Group B - Uses OpenAI/ChatGPT
[multi_tenant.groups.group-b]
client_id = "group-b"
client_secret = "mc_group-b_9z8y7x6w5v4u3t2s1r0q9p8o7n6m5l4k3j2i1h0g9f8e7d6c5b4a3z2y1x0w9v8u7"
provider = "openai"
model = "gpt-4o"
api_key = "sk-proj-your-openai-api-key"
temperature = 0.7

# Group C - Uses GLM (Zhipu AI)
[multi_tenant.groups.group-c]
client_id = "group-c"
client_secret = "mc_group-c_a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2"
provider = "glm"
model = "glm-4"
api_key = "your-id.secret"  # GLM uses format: id.secret

# Group D - Uses Ollama (local)
[multi_tenant.groups.group-d]
client_id = "group-d"
client_secret = "mc_group-d_f1e2d3c4b5a6f7e8d9c0b1a2f3e4d5c6b7a8f9e0d1c2b3a4f5e6d7c8b9a0f1e2d3"
provider = "ollama"
model = "llama3.2"
api_url = "http://localhost:11434"  # Ollama local endpoint
api_key = ""  # Ollama doesn't need API key
```

### Step 3: List Configured Clients

Verify your configuration:

```bash
./target/debug/mclaw list-clients
```

Output example:
```
Configured Clients:

  [group-a]
    Provider: openrouter
    Model: anthropic/claude-sonnet-4-20250514
    Secret: mc_group-a_1a2b3c4d...

  [group-b]
    Provider: openai
    Model: gpt-4o
    Secret: mc_group-b_9z8y7x6w...
```

### Step 4: Start the Gateway Server

```bash
# Use defaults from config.toml
./target/debug/mclaw gateway-server

# Or override port/host
./target/debug/mclaw gateway-server --port 8080 --host 0.0.0.0
```

You'll see:
```
🧠 MClaw Multi-Tenant Gateway
   Host: 0.0.0.0
   Port: 42618
   Health: http://0.0.0.0:42618/health
   Chat API: http://0.0.0.0:42618/api/v1/chat

   Configured clients: 3
     - group-a -> openrouter (anthropic/claude-sonnet-4-20250514)
     - group-b -> openai (gpt-4o)
     - group-c -> glm (glm-4)

   Ctrl+C to stop
```

---

## Part 3: Gateway API Reference

Once the gateway is running, it exposes these endpoints:

### Health Check

```bash
curl http://localhost:42618/health
```

Response:
```json
{
  "status": "ok",
  "service": "mclaw-gateway",
  "version": "0.1.0"
}
```

### Chat Completion

```bash
curl -X POST http://localhost:42618/api/v1/chat \
  -H "Authorization: Bearer mc_group-a_1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3" \
  -H "Content-Type: application/json" \
  -d '{
    "messages": [
      {"role": "user", "content": "Hello! Can you help me?"}
    ],
    "temperature": 0.7
  }'
```

Response:
```json
{
  "content": "Hello! I'd be happy to help you. What would you like to know?",
  "model": "anthropic/claude-sonnet-4-20250514"
}
```

### List Clients

```bash
curl http://localhost:42618/api/v1/clients
```

---

## Part 4: Client Machine Setup

Each client machine runs MClaw and connects to the gateway server.

### Option A: Using Environment Variables

```bash
# Set gateway connection details
export MCLAW_GATEWAY_URL="http://your-gateway-server:42618"
export MCLAW_CLIENT_ID="group-a"
export MCLAW_CLIENT_SECRET="mc_group-a_1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3"

# Run the agent
./target/debug/mclaw daemon
```

### Option B: Using Config File

Create `~/.mclaw/config.toml` on the client machine:

```toml
# Configure mclaw provider to use the gateway
[providers.mclaw]
type = "mclaw"  # This tells MClaw to use the gateway provider
gateway_url = "http://your-gateway-server:42618"
client_id = "group-a"
client_secret = "mc_group-a_1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3"

# Use mclaw as default provider
[default_model]
name = "mclaw"
type = "mclaw"
```

Then start the daemon:
```bash
./target/debug/mclaw daemon
```

---

## Part 5: Configuration Reference

### Supported Providers

| Provider | `provider` value | Notes |
|----------|-----------------|-------|
| OpenRouter | `openrouter` | Requires API key |
| OpenAI | `openai` | Requires API key |
| Anthropic | `anthropic` | Requires API key |
| GLM (Zhipu) | `glm` | Uses `id.secret` format |
| Ollama | `ollama` | Local, requires `api_url` |
| Gemini | `gemini` | Requires API key |
| OpenAI-compatible | (custom) | Requires `api_url` |

### Client Group Options

```toml
[multi_tenant.groups.<name>]
# Required fields
client_id = "group-name"           # Unique identifier
client_secret = "mc_..."           # Generated secret
provider = "openrouter"            # Provider name
model = "model-name"               # Model to use
api_key = "your-api-key"           # API key for this provider

# Optional fields
api_url = "https://..."            # Custom API endpoint
temperature = 0.7                  # Temperature (0.0-1.0)
max_tokens = 4096                  # Max tokens per response
rate_limit = 60                    # Requests per minute limit
```

---

## Part 6: Running as a Service

### Systemd (Linux)

Create `/etc/systemd/user/mclaw-gateway.service`:

```ini
[Unit]
Description=MClaw Gateway Server
After=network.target

[Service]
Type=simple
ExecStart=/opt/mclaw/target/debug/mclaw gateway-server
Restart=on-failure

[Install]
WantedBy=default.target
```

Enable and start:
```bash
systemctl --user enable mclaw-gateway
systemctl --user start mclaw-gateway
```

### Launchd (macOS)

Create `~/Library/LaunchAgents/com.mclaw.gateway.plist`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.mclaw.gateway</string>
    <key>ProgramArguments</key>
    <array>
        <string>/opt/mclaw/target/debug/mclaw</string>
        <string>gateway-server</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
</dict>
</plist>
```

Load and start:
```bash
launchctl load ~/Library/LaunchAgents/com.mclaw.gateway.plist
```

---

## Part 7: Security Considerations

1. **API Key Storage**: The gateway server holds all API keys. Secure the server machine properly.
2. **Client Secrets**: Never share client secrets publicly. Treat them like passwords.
3. **Network Security**: Use HTTPS/TLS in production. Consider using a reverse proxy (nginx, traefik).
4. **Firewall**: Restrict access to the gateway port (42618) to trusted networks only.
5. **Rate Limiting**: Configure `rate_limit` per client to prevent abuse.

---

## Commands Reference

| Command | Description |
|---------|-------------|
| `mclaw daemon` | Start the agent daemon (full agent with channels, scheduler) |
| `mclaw gateway-server` | Start the multi-tenant LLM gateway |
| `mclaw agent -m "message"` | Run a single agent interaction |
| `mclaw generate-secret <id>` | Generate a new client secret |
| `mclaw list-clients` | List configured gateway clients |
| `mclaw onboard` | Run the setup wizard |
| `mclaw status` | Show system status |
| `mclaw doctor` | Run diagnostics |

---

## Documentation

For full ZeroClaw documentation (inherited by MClaw):

- [CLAUDE.md](CLAUDE.md) - Project instructions and workflow
- [README.zeroclaw.md](README.zeroclaw.md) - ZeroClaw original README
- [docs/](docs/) - Comprehensive documentation

---

## License

MIT OR Apache-2.0 (inherits from ZeroClaw)

---

## Acknowledgments

MClaw is a fork of [ZeroClaw](https://github.com/zeroclaw-labs/zeroclaw). All credit for the core agent runtime goes to the ZeroClaw developers.
