# Architecture Overview

> System design and data flow for the TF2 Server Relay project.

## System Diagram

```
┌─────────────────────────────────────────────────────────────────────┐
│                         LOCAL INFRASTRUCTURE                        │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌──────────────┐     ┌──────────────┐     ┌──────────────┐         │
│  │  TF2 Server  │     │  TF2 Server  │     │  TF2 Server  │         │
│  │      #1      │     │      #2      │     │    #3/#4     │         │
│  │              │     │              │     │              │         │
│  │ ┌──────────┐ │     │ ┌──────────┐ │     │ ┌──────────┐ │         │
│  │ │SourceMod │ │     │ │SourceMod │ │     │ │SourceMod │ │         │
│  │ │  Plugin  │ │     │ │  Plugin  │ │     │ │  Plugin  │ │         │
│  │ └────┬─────┘ │     │ └────┬─────┘ │     │ └────┬─────┘ │         │
│  └──────┼───────┘     └──────┼───────┘     └──────┼───────┘         │
│         │                    │                    │                 │
│         │    Raw TCP         │    Raw TCP         │    Raw TCP      │
│         │    Binary          │    Binary          │    Binary       │
│         │                    │                    │                 │
│         └────────────────────┼────────────────────┘                 │
│                              │                                      │
│                              ▼                                      │
│                 ┌────────────────────────┐                          │
│                 │    RUST RELAY SERVER   │                          │
│                 │                        │                          │
│                 │  ┌──────────────────┐  │                          │
│                 │  │    TCP Server    │  │    Port 27050            │
│                 │  └────────┬─────────┘  │                          │
│                 │           │            │                          │
│                 │  ┌────────▼─────────┐  │                          │
│                 │  │  Event Router    │  │                          │
│                 │  │  (Broadcast Hub) │  │                          │
│                 │  └────────┬─────────┘  │                          │
│                 │           │            │                          │
│                 │  ┌────────▼─────────┐  │                          │
│                 │  │ TUI / CLI Output │  │    Terminal Interface    │
│                 │  └──────────────────┘  │                          │
│                 └────────────────────────┘                          │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Components

### 1. Rust Relay Server (`relay/`)

The central hub that manages all TF2 server connections.

| Module | Responsibility |
|--------|----------------|
| `main.rs` | Entry point, CLI args, mode selection (TUI/CLI) |
| `config.rs` | Load/validate settings.toml, hot-reload support |
| `server.rs` | TCP listener, accepts connections on port 27050 |
| `connection.rs` | Per-server state machine, handshake, heartbeat |
| `protocol.rs` | Parse/serialize binary packets |
| `relay.rs` | Route events between servers (broadcast/unicast) |
| `events.rs` | Event type definitions and handlers |
| `tui/` | ratatui-based terminal interface |

### 2. SourceMod Plugin (`sourcemod/`)

Runs on each TF2 server to capture and forward game events.

- **Extension**: AsyncSocket (srcdslab/sm-ext-asyncsocket)
- **Plugin**: `tf2_relay_client.sp`
- **Function**: Hook game events → serialize → send to relay

### 3. Ghost Player System

Virtual representations of remote players on each server.

```
Server A                          Server B
┌─────────────────┐               ┌─────────────────┐
│                 │               │                 │
│  Real Player 1  │◄──────────────│  Ghost Player 1 │
│  Ghost Player 2 │──────────────►│  Real Player 2  │
│                 │               │                 │
└─────────────────┘               └─────────────────┘
        │                                   │
        └─────────────► RELAY ◄─────────────┘
```

## Data Flow

### Event Broadcast (Chat Example)

```
1. Player types message on Server 1
2. SourceMod plugin hooks player_say event
3. Plugin serializes CHAT_MESSAGE packet
4. Packet sent via TCP to Relay
5. Relay broadcasts to Servers 2, 3, 4
6. Each server's plugin displays message
```

### Cross-Server Healing

```
1. Medic (Server A) heals Ghost (represents Player on Server B)
2. Plugin sends HEAL_REQUEST to Relay
3. Relay routes to Server B
4. Server B applies healing to real player
5. Server B sends HEAL_CONFIRM to Relay
6. Relay routes to Server A
7. Server A updates ghost health display
```

## Connection Lifecycle

```
┌─────────────┐
│   CONNECT   │  TCP connection established
└──────┬──────┘
       │
       ▼
┌─────────────┐
│  HANDSHAKE  │  Server sends ID, name, map, players
└──────┬──────┘
       │
       ▼
┌─────────────┐
│HANDSHAKE_ACK│  Relay confirms, assigns ID
└──────┬──────┘
       │
       ▼
┌─────────────┐
│    ACTIVE   │◄─────┐ Heartbeats every 1000ms
└──────┬──────┘      │ Events flow bidirectionally
       │             │
       └─────────────┘
       │
       ▼ (timeout or disconnect)
┌─────────────┐
│ DISCONNECT  │  Notify other servers
└─────────────┘
```

## Threading Model (Rust)

```
Main Thread
    │
    ├── Tokio Runtime
    │   │
    │   ├── TCP Listener Task
    │   │   └── Spawns Connection Tasks
    │   │
    │   ├── Connection Task (per server)
    │   │   ├── Read packets
    │   │   ├── Send to Event Router
    │   │   └── Write outgoing packets
    │   │
    │   └── Heartbeat Timer Task
    │
    └── TUI Render Loop (if TUI mode)
```

## Configuration Hot-Reload

```
settings.toml ──► Config Watcher ──► Apply DYNAMIC settings
                                          │
                                          ▼
                              ┌───────────────────────┐
                              │ connection_timeout_ms │
                              │ heartbeat_interval_ms │
                              │ position_sync_rate_hz │
                              │ log_level             │
                              │ ...                   │
                              └───────────────────────┘
```

STATIC settings (bind_address, max_servers) require restart.
