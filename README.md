# DecentChat

Decentralized terminal chat backed by [Guardian DB](https://crates.io/crates/guardian-db). Guardian's iroh-docs/Willow store provides persistence, synchronization, transport, discovery, and room invitations.

## Features

- Peer-to-peer messaging with persistent late-join history
- Shareable Guardian `DocTicket` room invitations
- Terminal UI with nicknames and online/away/offline presence
- Always-on, multi-room hosting with automatic host/client isolation
- Persistent Iroh identities and one-time legacy identity import
- MCP tools and resources for AI agent integrations

## Install

On macOS or Linux (x86_64 or arm64):

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://raw.githubusercontent.com/maslowalex/decentchat/main/install.sh | sh
```

The installer verifies the release checksum and places `decentchat` in `~/.local/bin`. If that directory is not already on `PATH`, the installer prints the required adjustment. Set `DECENTCHAT_INSTALL_DIR` to install elsewhere.

Developers can instead build with the Rust version pinned in `rust-toolchain.toml`:

```bash
git clone https://github.com/maslowalex/decentchat.git
cd decentchat
cargo install --path crates/decentchat --locked
```

## Quick start

Start an always-on room host:

```bash
decentchat host
```

This hosts a room named `lobby` on UDP port `4001` and prints its ticket plus a ready-to-paste command:

```text
Room 'lobby' is ready
Ticket:
  doc...
Join with:
  decentchat join 'doc...'
```

Keep the host running. In another terminal or on another machine, join with the complete ticket:

```bash
decentchat join 'doc...'
```

The first client join asks:

```text
Choose your display name:
```

The name is remembered for future rooms. It can also be supplied or replaced non-interactively:

```bash
decentchat join 'doc...' --name Alice
```

The host automatically uses `~/.config/decentchat/host`, while normal clients use `~/.config/decentchat`, so a host and client can run together without locking Guardian's database. To simulate another client on one machine, give it a separate directory:

```bash
decentchat --config-dir /tmp/decentchat-bob \
  join 'doc...' --name Bob --local
```

### Overrides

Create a named room or change the host port:

```bash
decentchat host myroom --port 5001
```

Host several rooms:

```bash
decentchat host --groups lobby,team
```

For mDNS-only LAN discovery, pass `--local` to both host and clients. Without `--local`, Guardian/Iroh uses normal global n0 discovery as well as mDNS.

The previous commands remain supported:

```bash
decentchat relay --groups myroom --port 4001
decentchat join --ticket 'doc...' --name Bob
decentchat join --group myroom --name Alice
```

A legacy `dchat...` ticket is unsupported; distribute the complete raw Guardian `doc...` ticket. Never copy or share `node_secret.key`.

## TUI commands

| Command | Description |
|---|---|
| `/nick <name>` | Change and remember your nickname |
| `/quit` | Leave gracefully and exit |
| `/help` | Show commands |
| `/members` | Toggle the members sidebar |
| `/clear` | Clear the local display |

Members heartbeat every 30 seconds. A graceful leave is shown offline immediately; a peer that disappears becomes away after 90 seconds.

## Diagnostics

Identity creation is automatic. These commands remain available for inspection and recovery:

```bash
decentchat identity
decentchat info
```

Client data is stored below `~/.config/decentchat/` by default. `profile.json` contains the preferred display name, while Guardian identity and room data live below `guardian/`.

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

See [PLAN.md](PLAN.md) for record keys, synchronization behavior, and lifecycle details. For systemd hosting, see [deploy/README.md](deploy/README.md).

## License

MIT
