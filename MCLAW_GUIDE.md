# MClaw - Multi-Tenant AI Agent Runtime

MClaw is a fork of ZeroClaw with centralized multi-tenant LLM gateway functionality.

## What's New in MClaw

1. **Multi-tenant LLM Gateway** - Central server holds all LLM API keys and routes requests by client identity
2. **MClaw Provider** - Client daemons can connect to a remote MClaw gateway for LLM access
3. **New Commands** - `gateway-server`, `generate-secret`, `list-clients`

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    MClaw Gateway Server                     │
│  (One instance holds all LLM API keys)                      │
│  Port: 42618                                                 │
└──────────────────────────┬──────────────────────────────────┘
                           │
     ┌─────────────────────┼─────────────────────┐
     ▼                     ▼                     ▼
┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐
│  Group A        │ │  Group B        │ │  Group C        │
│  OpenRouter     │ │  ChatGPT        │ │  GLM            │
│  (dev team)     │ │  (production)   │ │  (china)        │
└────────┬────────┘ └────────┬────────┘ └────────┬────────┘
         │                   │                   │
   ┌─────┴─────┐       ┌─────┴─────┐       ┌─────┴─────┐
   │ MClaw     │       │ MClaw     │       │ MClaw     │
   │ Daemon    │       │ Daemon    │       │ Daemon    │
   │ (full     │       │ (full     │       │ (full     │
   │  agent)   │       │  agent)   │       │  agent)   │
   └───────────┘       └───────────┘       └───────────┘
```

## Configuration

### Gateway Server Config (`config.toml`)

```toml
[multi_tenant]
enabled = true
host = "0.0.0.0"
port = 42618

# Group A - Uses OpenRouter
[multi_tenant.groups.group-a]
client_id = "group-a"
client_secret = "mc_group-a_<generated_secret>"
provider = "openrouter"
model = "anthropic/claude-sonnet-4-20250514"
temperature = 0.7

# Group B - Uses OpenAI (ChatGPT)
[multi_tenant.groups.group-b]
client_id = "group-b"
client_secret = "mc_group-b_<generated_secret>"
provider = "openai"
model = "gpt-4o"

# Provider credentials (only on gateway server!)
[providers_config.openrouter]
api_key = "sk-or-..."
```

### Client Daemon Config

```toml
# Use environment variables to connect to gateway
export MCLAW_GATEWAY_URL="http://gateway-server:42618"
export MCLAW_CLIENT_ID="group-a"
export MCLAW_CLIENT_SECRET="mc_group-a_..."

# Or set provider to "mclaw"
[default_provider]
name = "mclaw"
```

## Commands

```bash
# Start the multi-tenant gateway server
mclaw gateway-server

# Generate a new client secret
mclaw generate-secret group-a

# List configured clients
mclaw list-clients

# Start the daemon (uses local or gateway provider)
mclaw daemon

# Run agent
mclaw agent
```

## Building

```bash
cd /opt/works/personal/github/mclaw
cargo build --release
```

## Files Added/Modified

### New Files
- `src/multi_tenant/mod.rs` - Multi-tenant module
- `src/multi_tenant/auth.rs` - Authentication for client groups
- `src/multi_tenant/config.rs` - Multi-tenant configuration schema
- `src/multi_tenant/server.rs` - HTTP gateway server
- `src/providers/mclaw.rs` - MClaw gateway provider (for client daemons)

### Modified Files
- `Cargo.toml` - Updated project name to "mclaw"
- `src/main.rs` - Added new commands, updated binary name
- `src/lib.rs` - Added multi_tenant module
- `src/config/schema.rs` - Added multi_tenant field to Config
- `src/providers/mod.rs` - Added mclaw and glm providers, added factory case

## License

MIT OR Apache-2.0 (inherits from ZeroClaw)
