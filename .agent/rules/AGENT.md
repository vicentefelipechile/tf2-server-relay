# TF2 Server Relay - AI Agent Guide

> Quick reference for AI assistants working on this project.

## Project Overview

**tf2-server-relay** connects 1-4 TF2 game servers to enable cross-server gameplay (healing, damage, chat) using a high-performance Rust relay server and SourceMod plugins.

## Tech Stack

| Component | Technology |
|-----------|------------|
| Relay Server | Rust + Tokio (async) |
| TUI | ratatui + crossterm |
| Protocol | Raw TCP Binary (not JSON) |
| TF2 Plugin | SourceMod + AsyncSocket Extension |
| Config | TOML (settings.toml) |

## Key Files

```
tf2-server-relay/
├── BLUEPRINT.xml          # Complete technical specification
├── AGENT.md               # This file (AI quick reference)
├── docs/
│   ├── ARCHITECTURE.md    # System design and data flow
│   ├── PROTOCOL.md        # Binary packet format specification
│   ├── EVENTS.md          # Complete event dictionary
│   └── CROSS_SERVER.md    # Ghost system & healing mechanics
├── relay/                 # Rust relay server
│   └── src/
│       ├── main.rs        # Entry point + CLI parsing
│       ├── config.rs      # Configuration loading
│       ├── server.rs      # TCP listener
│       ├── connection.rs  # Per-server handler
│       ├── protocol.rs    # Packet parsing
│       ├── relay.rs       # Event routing
│       ├── events.rs      # Event definitions
│       └── tui/           # Terminal UI
└── sourcemod/             # SourceMod plugin
    └── scripting/
        └── tf2_relay_client.sp
```

## Core Concepts

### Ghost Player System
Each server maintains "ghost" entities representing players from other servers. When a local player interacts with a ghost, the interaction is forwarded to the real player's server.

### Cross-Server Healing (Critical Feature)
1. Medic on Server A targets ghost of Player X (from Server B)
2. Server A sends `HEAL_REQUEST` via relay
3. Server B applies healing to real Player X
4. Server B sends `HEAL_CONFIRM` back
5. Server A updates ghost's health display

### Packet Structure
```
[MAGIC:2][VER:1][TYPE:1][LEN:2][SRV:1][SEQ:2][PAYLOAD:var][CRC:1]
```
- Little-endian byte order
- Strings: length-prefixed (1 byte length + UTF-8 data)

## Configuration

- **STATIC** settings require restart (bind_address, max_servers)
- **DYNAMIC** settings can change at runtime via TUI (F2 key)
- CLI mode (--cli) reads from settings.toml, no runtime changes

## Running

```bash
# TUI mode (default)
./tf2-server-relay

# CLI mode (headless)
./tf2-server-relay --cli

# Custom config
./tf2-server-relay -C /path/to/settings.toml
```

## Important Dependencies

### Rust (relay/Cargo.toml)
- tokio (async runtime)
- ratatui + crossterm (TUI)
- clap (CLI)
- serde + toml (config)
- bytes (buffer handling)
- tracing (logging)

### SourceMod
- **AsyncSocket Extension**: https://github.com/srcdslab/sm-ext-asyncsocket
- Uses libuv for high-performance TCP

## Event ID Ranges

| Range | Category |
|-------|----------|
| 0x00-0x0F | System (handshake, heartbeat) |
| 0x10-0x1F | Chat |
| 0x20-0x2F | Player (death, connect) |
| 0x30-0x3F | Gameplay (heal, damage, buildings) |
| 0x40-0x5F | Game (round, map) |
| 0x70-0x7F | Ghost sync & cross-server |

## When Making Changes

1. **Protocol changes**: Update BLUEPRINT.xml AND docs/PROTOCOL.md
2. **New events**: Add to EVENTS.md with full payload spec
3. **Config changes**: Mark as STATIC or DYNAMIC in settings.toml
4. **Cross-server features**: Document in CROSS_SERVER.md

## Reference

Full specification: [BLUEPRINT.xml](./BLUEPRINT.xml)
