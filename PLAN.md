# Guardian DB architecture

DecentChat is a projection over Guardian DB 0.19. It defines no secondary wire protocol, gossip topic, CRDT, logical clock, ticket encoding, transport abstraction, or identity implementation.

## Runtime flow

1. `GuardianNode` opens Guardian beneath the DecentChat configuration directory. Guardian owns the persistent Iroh endpoint, discovery, transport, blobs, iroh-docs/Willow synchronization, and `node_secret.key`.
2. A room is one Guardian key-value namespace. Creating a group opens its stable local store; importing a raw write-capability `DocTicket` opens a stable store name derived from the ticket hash.
3. `RoomSession` reads Guardian's live local view every 250 ms. It validates versioned JSON, updates a deterministic local projection, and emits `ChatEvent` values for the TUI and MCP layers.
4. Guardian performs replication directly. `SyncCompleted` means the initial local projection, including `meta/room`, is ready; it does not describe a custom sync handshake.

## Room schema

All supported records use `version: 1`. Any other version is a compatibility error.

| Key | Value |
|---|---|
| `meta/room` | `{ version, name, created_at_ms }` |
| `messages/<uuid-v7>` | `{ version, id, author, sent_at_ms, content }` |
| `members/<node-id>` | `{ version, node_id, nickname, heartbeat_at_ms, offline }` |

Messages are immutable, deduplicated by key, and displayed in `(sent_at_ms, UUID)` order. Guardian provides LWW key resolution for member records. Active clients update their member record every 30 seconds; after 90 seconds without a heartbeat the projected presence becomes away. Graceful leave writes `offline: true`.

## Invitations and discovery

Room invitations are Guardian's raw write-capability `DocTicket` strings. Import waits at most 30 seconds for `meta/room`. Possession of the ticket grants read/write access and the ability to forward the ticket. Legacy `dchat...` tickets are intentionally incompatible.

Normal mode enables mDNS and n0 discovery. `--local` enables mDNS only. `relay` is an always-on Guardian super peer with a fixed endpoint port and may keep any number of requested room namespaces online in one process.

## Identity and storage

Guardian data defaults to `~/.config/decentchat/guardian/`. If `identity.key` exists and `guardian/node_secret.key` does not, its exact 32 raw bytes are copied once before Guardian starts. Guardian generates all subsequent identities. `decentchat identity --force` removes only the Guardian node secret and deliberately skips legacy re-import for that regeneration.

## Test boundary

`RoomStore` contains only `get`, `put`, `all`, `share_ticket`, and `close`. Unit tests use an in-memory implementation to verify JSON compatibility, ordering, deduplication, presence, and changed-record events without networking. Production delegates each method to Guardian's `KeyValueStore`.
