# decentchat

Decentralized terminal chat using iroh P2P networking and CRDTs.

## Features

- Peer-to-peer messaging with no central server
- CRDT-based message ordering for eventual consistency
- Terminal UI with presence indicators
- Persistent identity across sessions
- MCP server for AI agent integration (Claude Code, etc.)

## Installation

### From Source

```bash
git clone https://github.com/youruser/decentchat.git
cd decentchat
cargo install --path crates/decentchat
```

### From Git (once published)

```bash
cargo install --git https://github.com/youruser/decentchat
```

## Quick Start

### 1. Create Your Identity

```bash
decentchat identity
```

This generates a keypair stored in `~/.config/decentchat/`.

### 2. Start a Relay (First User)

```bash
decentchat relay --groups myroom
```

This starts a chat room and displays a connection ticket. Share this ticket with others.

### 3. Join a Room

```bash
decentchat join --ticket <ticket> --name YourName
```

Paste the ticket from the relay to join the chat.

## Local Testing (LAN/Same Machine)

For testing without internet relay servers, use the `--local` flag.

### Terminal 1: Start the Relay

```bash
decentchat relay --local --groups test --port 4433
```

Output will show:
```
Relay node started
Node ID: <64-char-hex>

Share this ticket to join 'test':
  dchat...

Or use traditional format:
  --peer <node_id>@<ip>:<port>

Hosting groups: test
```

### Terminal 2: Join as a Peer

Using the ticket (recommended):
```bash
decentchat join --local --ticket dchat... --name Alice
```

Or using the peer address directly:
```bash
decentchat join --local --group test --name Alice \
  --peer <node_id>@192.168.x.x:4433
```

### Terminal 3: Join as Another Peer

```bash
# Use a different config directory to simulate a different user
decentchat --config-dir /tmp/bob join --local --ticket dchat... --name Bob
```

Now Alice and Bob can chat! Messages sync via CRDTs.

## Slash Commands

Inside the chat TUI:

| Command | Description |
|---------|-------------|
| `/nick <name>` | Set your display name |
| `/quit` | Exit the chat |
| `/help` | Show available commands |
| `/users` | List connected users |
| `/clear` | Clear the chat screen |

## MCP Server (AI Agent Integration)

DecentChat includes an MCP (Model Context Protocol) server for AI agent integration.

### Start the MCP Server

```bash
decentchat mcp
```

This starts a stdio-based MCP server that AI tools like Claude Code can connect to.

### Configure Claude Code

Add to your Claude Code settings (`.claude/settings.json`):

```json
{
  "mcpServers": {
    "decentchat": {
      "command": "decentchat",
      "args": ["mcp"]
    }
  }
}
```

### Available MCP Tools

| Tool | Description |
|------|-------------|
| `join_room` | Join a chat room by name or ticket |
| `send_message` | Send a message to the current room |
| `set_nickname` | Change display name |
| `leave_room` | Leave the current room |
| `get_messages` | Get recent messages |

### Available MCP Resources

| URI | Description |
|-----|-------------|
| `chat://messages` | Recent messages (JSON) |
| `chat://users` | Online users list |
| `chat://status` | Connection status |

## Architecture

- **decentchat** - CLI binary
- **decentchat-core** - CRDTs and event types
- **decentchat-mcp** - MCP server for AI integration
- **decentchat-protocol** - Wire protocol and sync
- **decentchat-tui** - Terminal UI with ratatui

## License

MIT
