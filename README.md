# TF2 Server Relay

> Cross-server communication for TF2 game servers.

Connect 1-4 TF2 servers to enable shared chat, death notifications, cross-server healing, and synchronized gameplay.

## Quick Start

```bash
# Run with TUI (default)
./tf2-server-relay

# Run in CLI mode (headless)
./tf2-server-relay --cli

# Custom config
./tf2-server-relay -C /path/to/settings.toml
```

## Features

- 🔄 **Cross-Server Events**: Chat, deaths, connects synced across all servers
- 💉 **Medic Healing**: Heal players on other servers through ghost entities
- ⚡ **Low Latency**: Raw TCP binary protocol, <50ms on local networks
- 🖥️ **Terminal UI**: Real-time monitoring with keyboard navigation
- ⚙️ **Runtime Config**: Change settings on-the-fly via TUI (F2)

## Architecture

```
TF2 Server 1 ──┐
TF2 Server 2 ──┼──► Rust Relay Server ──► TUI / CLI
TF2 Server 3 ──┤        (TCP)
TF2 Server 4 ──┘
```

## Documentation

| Document | Description |
|----------|-------------|
| [AGENT.md](./.agent/rules/AGENT.md) | Quick reference for AI assistants |
| [ARCHITECTURE.md](./.agent/rules/ARCHITECTURE.md) | System design and data flow |
| [PROTOCOL.md](./.agent/rules/PROTOCOL.md) | Binary packet specification |
| [EVENTS.md](./.agent/rules/EVENTS.md) | Complete event dictionary |
| [CROSS_SERVER.md](./.agent/rules/CROSS_SERVER.md) | Ghost system & healing |
| [CONFIG.md](./.agent/rules/CONFIG.md) | Configuration reference |
| [BLUEPRINT.xml](./BLUEPRINT.xml) | Full technical specification |

## Requirements

### Relay Server
- Rust 1.70+
- Tokio runtime

### TF2 Servers
- SourceMod 1.11+
- [AsyncSocket Extension](https://github.com/srcdslab/sm-ext-asyncsocket)

## Building

```bash
cd relay
cargo build --release
```

## Configuration

Edit `settings.toml`:

```toml
[server]
bind_address = "0.0.0.0:27050"
max_servers = 4

[relay]
enable_cross_healing = true
enable_cross_damage = true
```

See [CONFIG.md](./.agent/rules/CONFIG.md) for all options.

## TUI Keybindings

| Key | Action |
|-----|--------|
| Q | Quit |
| P | Pause event feed |
| C | Clear history |
| F2 | Open settings |
| 1-4 | Focus server |
| ? | Help |

## License

MIT
