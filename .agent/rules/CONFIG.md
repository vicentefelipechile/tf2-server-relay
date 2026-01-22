# Configuration Reference

> Complete settings.toml documentation with all options.

## File Location

Default: `./settings.toml` (same directory as executable)
Override: `./tf2-server-relay -C /path/to/settings.toml`

## Hot-Reload Classification

- **STATIC**: Requires restart to change
- **DYNAMIC**: Can change at runtime via TUI (F2)

---

## [server]

Core relay server settings.

```toml
[server]
bind_address = "0.0.0.0:27050"  # [STATIC] IP:PORT to listen on
max_servers = 4                  # [STATIC] Maximum connected servers (1-4)
connection_timeout_ms = 5000     # [DYNAMIC] Connection timeout
heartbeat_interval_ms = 1000     # [DYNAMIC] Heartbeat interval
```

| Option | Type | Default | Hot-Reload | Description |
|--------|------|---------|------------|-------------|
| bind_address | String | 0.0.0.0:27050 | ❌ | IP and port to bind |
| max_servers | u8 | 4 | ❌ | Max TF2 servers (1-4) |
| connection_timeout_ms | u64 | 5000 | ✅ | Timeout (1000-30000) |
| heartbeat_interval_ms | u64 | 1000 | ✅ | Heartbeat interval (100-5000) |

---

## [sync]

Cross-server synchronization settings (Ghost Player System).

```toml
[sync]
position_sync_rate_hz = 20       # [DYNAMIC] Ghost position updates per second
predictive_healing = true        # [DYNAMIC] Optimistic healing
ghost_interpolation = true       # [DYNAMIC] Smooth ghost movement
max_ghost_latency_ms = 100       # [DYNAMIC] Max acceptable ghost latency
```

| Option | Type | Default | Hot-Reload | Description |
|--------|------|---------|------------|-------------|
| position_sync_rate_hz | u8 | 20 | ✅ | Position updates/sec (5-60) |
| predictive_healing | bool | true | ✅ | Optimistic heal display |
| ghost_interpolation | bool | true | ✅ | Smooth ghost movement |
| max_ghost_latency_ms | u64 | 100 | ✅ | Max latency (20-500) |

---

## [logging]

Log output configuration.

```toml
[logging]
level = "info"                   # [DYNAMIC] trace/debug/info/warn/error
file = null                      # [STATIC] Optional log file path
log_events = true                # [DYNAMIC] Log relayed events
log_packets = false              # [DYNAMIC] Log raw packets (debug)
```

| Option | Type | Default | Hot-Reload | Description |
|--------|------|---------|------------|-------------|
| level | String | info | ✅ | Log verbosity |
| file | Option\<String\> | null | ❌ | Log file path |
| log_events | bool | true | ✅ | Log relayed events |
| log_packets | bool | false | ✅ | Log raw packets (high volume!) |

### Log Levels

| Level | Description |
|-------|-------------|
| trace | Everything (very verbose) |
| debug | Debug info + below |
| info | Normal operation |
| warn | Warnings + errors |
| error | Errors only |

---

## [tui]

Terminal User Interface settings.

```toml
[tui]
refresh_rate_ms = 100            # [DYNAMIC] UI refresh rate
max_event_history = 1000         # [DYNAMIC] Event history size
color_scheme = "default"         # [DYNAMIC] default/dark/light/high_contrast
show_timestamps = true           # [DYNAMIC] Show event timestamps
compact_mode = false             # [DYNAMIC] Compact layout
```

| Option | Type | Default | Hot-Reload | Description |
|--------|------|---------|------------|-------------|
| refresh_rate_ms | u64 | 100 | ✅ | UI refresh (16-1000) |
| max_event_history | usize | 1000 | ✅ | Events to keep (100-10000) |
| color_scheme | String | default | ✅ | Color theme |
| show_timestamps | bool | true | ✅ | Show timestamps |
| compact_mode | bool | false | ✅ | Compact layout |

### Color Schemes

| Scheme | Description |
|--------|-------------|
| default | Standard colors |
| dark | Dark background, bright text |
| light | Light background, dark text |
| high_contrast | Maximum readability |

---

## [relay]

Event relay behavior.

```toml
[relay]
broadcast_chat = true            # [DYNAMIC] Relay chat messages
broadcast_deaths = true          # [DYNAMIC] Relay death events
broadcast_connect = true         # [DYNAMIC] Relay connect/disconnect
enable_cross_healing = true      # [DYNAMIC] Cross-server Medic healing
enable_cross_damage = true       # [DYNAMIC] Cross-server damage
```

| Option | Type | Default | Hot-Reload | Description |
|--------|------|---------|------------|-------------|
| broadcast_chat | bool | true | ✅ | Relay chat messages |
| broadcast_deaths | bool | true | ✅ | Relay death events |
| broadcast_connect | bool | true | ✅ | Relay connect/disconnect |
| enable_cross_healing | bool | true | ✅ | Cross-server healing |
| enable_cross_damage | bool | true | ✅ | Cross-server damage |

---

## Complete Example

```toml
# TF2 Server Relay Configuration
# Generated: 2026-01-22

[server]
bind_address = "0.0.0.0:27050"
max_servers = 4
connection_timeout_ms = 5000
heartbeat_interval_ms = 1000

[sync]
position_sync_rate_hz = 20
predictive_healing = true
ghost_interpolation = true
max_ghost_latency_ms = 100

[logging]
level = "info"
file = null
log_events = true
log_packets = false

[tui]
refresh_rate_ms = 100
max_event_history = 1000
color_scheme = "default"
show_timestamps = true
compact_mode = false

[relay]
broadcast_chat = true
broadcast_deaths = true
broadcast_connect = true
enable_cross_healing = true
enable_cross_damage = true
```

---

## TUI Settings Panel (F2)

In TUI mode, press **F2** to open the Settings panel.

### Navigation

| Key | Action |
|-----|--------|
| Up/Down | Navigate between settings |
| Left/Right | Decrease/increase numeric values |
| Enter | Edit text / toggle boolean |
| S | Save changes to settings.toml |
| R | Reset to defaults |
| Escape | Close settings panel |

### Categories

Settings are organized into categories:
- **Network** - Connection and timing
- **Display** - TUI appearance
- **Sync** - Ghost player system
- **Logging** - Log verbosity

STATIC settings are grayed out and cannot be modified at runtime.
