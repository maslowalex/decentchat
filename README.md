# decentchat

Decentralized terminal chat using iroh P2P networking and CRDTs.

## Features

- Peer-to-peer messaging with no central server
- CRDT-based message ordering for eventual consistency
- Terminal UI with presence indicators
- Persistent identity across sessions

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
decentchat relay
```

This starts a chat room and displays a ticket. Share this ticket with others.

### 3. Join a Room

```bash
decentchat join <ticket>
```

Paste the ticket from the relay to join the chat.

## Slash Commands

Inside the chat TUI:

| Command | Description |
|---------|-------------|
| `/nick <name>` | Set your display name |
| `/quit` | Exit the chat |
| `/help` | Show available commands |
| `/users` | List connected users |
| `/clear` | Clear the chat screen |

## Architecture

- **decentchat** - CLI binary
- **decentchat-core** - Identity and state management
- **decentchat-protocol** - Wire protocol and CRDT sync
- **decentchat-tui** - Terminal UI with ratatui

## License

MIT
