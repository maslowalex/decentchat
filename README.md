# DecentChat

Decentralized terminal chat backed by [Guardian DB](https://crates.io/crates/guardian-db). Guardian's iroh-docs/Willow key-value store is the only persistence, synchronization, transport, discovery, and invitation layer.

## Features

- Peer-to-peer messaging with persistent late-join history
- Raw Guardian `DocTicket` room invitations with read/write capability
- Terminal UI with nicknames and online/away/offline presence
- Persistent Iroh node identity, including one-time import of the legacy raw key
- One-process, multi-room Guardian super-peer mode
- MCP tools and resources for AI agent integrations

## Requirements and installation

Rust 1.97 is pinned in `rust-toolchain.toml`.

```bash
git clone https://github.com/youruser/decentchat.git
cd decentchat
cargo install --path crates/decentchat
```

Guardian data is stored under `~/.config/decentchat/guardian/` by default. The Iroh identity is `guardian/node_secret.key`; room namespaces and Guardian metadata live below the same directory. If the old `identity.key` contains exactly 32 raw bytes, DecentChat imports it once and preserves the node ID.

## Quick start

Create or inspect the local identity:

```bash
decentchat identity
decentchat info
```

Create a room and open the TUI:

```bash
decentchat join --group myroom --name Alice
```

The creator can obtain a shareable ticket through MCP, or run an always-on super peer which prints one ticket per room:

```bash
decentchat relay --groups myroom,team --port 4001
```

Join using the exact raw Guardian ticket:

```bash
decentchat join --ticket '<guardian-doc-ticket>' --name Bob
```

`join` accepts exactly one of `--group` and `--ticket`. A legacy `dchat...` ticket returns an explicit migration error; create a Guardian room and distribute its new raw ticket. The removed `--peer`, `--state-file`, and `--external-ip` flags are no longer needed.

For LAN-only discovery, pass `--local` to `join` or `relay`. This keeps mDNS enabled and disables n0 global discovery.

To simulate another identity on one machine:

```bash
decentchat --config-dir /tmp/decentchat-bob \
  join --local --ticket '<guardian-doc-ticket>' --name Bob
```

## TUI commands

| Command | Description |
|---|---|
| `/nick <name>` | Change your nickname |
| `/quit` | Leave gracefully and exit |
| `/help` | Show commands |
| `/users` | Toggle the members sidebar |
| `/clear` | Clear the local display |

Members heartbeat every 30 seconds. A graceful leave is shown offline immediately; a peer that disappears becomes away after 90 seconds.

## MCP server

```bash
decentchat mcp
```

The stable tool set is `join_room`, `send_message`, `set_nickname`, `leave_room`, `get_new_messages`, and `get_ticket`. `join_room` accepts exactly one of `room` (create) or `ticket` (import), and ticket results are raw Guardian tickets.

Resources remain:

| URI | Description |
|---|---|
| `chat://messages` | Recent messages as JSON |
| `chat://users` | Current room members as JSON |
| `chat://status` | Guardian room and node status as JSON |

## Architecture

- `decentchat`: CLI and process orchestration
- `decentchat-core`: versioned JSON domain records and `ChatEvent`
- `decentchat-guardian`: Guardian node, room-store adapter, projection, tickets, and presence
- `decentchat-tui`: ratatui terminal UI
- `decentchat-mcp`: MCP tools and resources

See [PLAN.md](PLAN.md) for record keys, synchronization behavior, and lifecycle details.

## License

MIT
