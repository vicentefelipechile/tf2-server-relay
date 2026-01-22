# Event Dictionary

> Complete list of all relay events with payload specifications.

## Event Format

All events use the standard packet format from [PROTOCOL.md](./PROTOCOL.md).
Each event has a unique `packet_type` ID and specific payload structure.

---

## System Events (0x00-0x0F)

### 0x00 - HANDSHAKE
Initial connection from TF2 server to relay.

| Field | Type | Description |
|-------|------|-------------|
| server_id | u8 | Requested server ID (1-4) |
| server_name | string | Human-readable name |
| map_name | string | Current map |
| max_players | u8 | Server slot limit |
| current_players | u8 | Current player count |

### 0x01 - HANDSHAKE_ACK
Relay response to handshake.

| Field | Type | Description |
|-------|------|-------------|
| success | u8 | 1=accepted, 0=rejected |
| assigned_id | u8 | Confirmed server ID |
| connected_servers | u8 | Total connected servers |

### 0x02 - HEARTBEAT
Keep-alive from server.

| Field | Type | Description |
|-------|------|-------------|
| timestamp | u32 | Unix timestamp |
| current_players | u8 | Player count |

### 0x03 - HEARTBEAT_ACK
Relay heartbeat response.

| Field | Type | Description |
|-------|------|-------------|
| timestamp | u32 | Echo of received timestamp |
| connected_servers | u8 | Connected server count |

### 0x04 - SERVER_CONNECT
Broadcast when server joins relay.

| Field | Type | Description |
|-------|------|-------------|
| server_id | u8 | New server's ID |
| server_name | string | Server name |

### 0x05 - SERVER_DISCONNECT
Broadcast when server leaves relay.

| Field | Type | Description |
|-------|------|-------------|
| server_id | u8 | Disconnected server ID |
| reason | string | Disconnect reason |

### 0x06 - ERROR
Error notification.

| Field | Type | Description |
|-------|------|-------------|
| error_code | u16 | Numeric error code |
| error_message | string | Human-readable message |

---

## Chat Events (0x10-0x1F)

### 0x10 - CHAT_MESSAGE
Player chat message.

| Field | Type | Description |
|-------|------|-------------|
| player_name | string | Sender name |
| steam_id | u64 | Sender Steam ID |
| team | u8 | 0=Spec, 1=Unassigned, 2=RED, 3=BLU |
| chat_type | u8 | 0=All, 1=Team |
| message | string | Chat content |

### 0x11 - ADMIN_MESSAGE
Admin broadcast message.

| Field | Type | Description |
|-------|------|-------------|
| admin_name | string | Admin name |
| message | string | Broadcast content |
| color | u32 | RGBA color value |

---

## Player Events (0x20-0x2F)

### 0x20 - PLAYER_DEATH
Player killed.

| Field | Type | Description |
|-------|------|-------------|
| victim_name | string | Victim name |
| victim_steam_id | u64 | Victim Steam ID |
| victim_team | u8 | Victim team |
| victim_class | u8 | Victim class (1-9) |
| attacker_name | string | Attacker name |
| attacker_steam_id | u64 | Attacker Steam ID |
| attacker_team | u8 | Attacker team |
| attacker_class | u8 | Attacker class |
| weapon | string | Weapon name |
| crit_type | u8 | 0=Normal, 1=Mini, 2=Crit |
| death_flags | u16 | Special flags (see below) |

**Death Flags:**
| Bit | Value | Name |
|-----|-------|------|
| 0 | 0x0001 | HEADSHOT |
| 1 | 0x0002 | BACKSTAB |
| 2 | 0x0004 | AIRBORNE |
| 3 | 0x0008 | DOMINATION |
| 4 | 0x0010 | REVENGE |
| 5 | 0x0020 | FEIGN_DEATH |
| 6 | 0x0040 | SUICIDE |
| 7 | 0x0080 | ENVIRONMENTAL |

### 0x21 - PLAYER_CONNECT
Player joined server.

| Field | Type | Description |
|-------|------|-------------|
| player_name | string | Player name |
| steam_id | u64 | Steam ID |
| ip_hash | u32 | Hashed IP (privacy) |

### 0x22 - PLAYER_DISCONNECT
Player left server.

| Field | Type | Description |
|-------|------|-------------|
| player_name | string | Player name |
| steam_id | u64 | Steam ID |
| reason | string | Disconnect reason |

### 0x23 - PLAYER_TEAM_CHANGE
Player changed teams.

| Field | Type | Description |
|-------|------|-------------|
| player_name | string | Player name |
| steam_id | u64 | Steam ID |
| old_team | u8 | Previous team |
| new_team | u8 | New team |

### 0x24 - PLAYER_CLASS_CHANGE
Player changed class.

| Field | Type | Description |
|-------|------|-------------|
| player_name | string | Player name |
| steam_id | u64 | Steam ID |
| new_class | u8 | Class ID (see below) |

**Class IDs:**
| ID | Class |
|----|-------|
| 1 | Scout |
| 2 | Sniper |
| 3 | Soldier |
| 4 | Demoman |
| 5 | Medic |
| 6 | Heavy |
| 7 | Pyro |
| 8 | Spy |
| 9 | Engineer |

---

## Gameplay Events (0x30-0x3F)

### 0x30 - PLAYER_HEALED
Player receives healing. Source: `player_healed`

| Field | Type | Description |
|-------|------|-------------|
| patient_steam_id | u64 | Healed player |
| healer_steam_id | u64 | Healer (0 if healthpack) |
| amount | u16 | Health restored |
| heal_source | u8 | Source type (see below) |

**Heal Sources:** 0=Medigun, 1=Dispenser, 2-4=Healthpacks, 5=Cart, 6=Regen, 7=Other

### 0x31 - BUILDING_HEALED
Building repaired. Source: `building_healed`

| Field | Type | Description |
|-------|------|-------------|
| building_id | u16 | Building entity index |
| healer_steam_id | u64 | Engineer |
| amount | u16 | Health restored |
| building_type | u8 | Type (see below) |

### 0x32 - UBER_DEPLOYED
Medic activates ÜberCharge. Source: `player_chargedeployed`

| Field | Type | Description |
|-------|------|-------------|
| medic_steam_id | u64 | Medic |
| target_steam_id | u64 | Über target |
| uber_type | u8 | Type (see below) |

**Über Types:** 0=Invuln, 1=Kritz, 2=QuickFix, 3-5=Vaccinator variants

### 0x33 - PLAYER_INVULNED
Player becomes invulnerable. Source: `player_invulned`

| Field | Type | Description |
|-------|------|-------------|
| player_steam_id | u64 | Player |
| medic_steam_id | u64 | Medic (0 if self) |
| duration_ms | u16 | Duration |

### 0x34 - PLAYER_HURT
Player takes damage. Source: `player_hurt`

| Field | Type | Description |
|-------|------|-------------|
| victim_steam_id | u64 | Victim |
| attacker_steam_id | u64 | Attacker (0 if world) |
| damage_amount | u16 | Damage dealt |
| health_remaining | u16 | Health after |
| weapon_id | u16 | Weapon index |
| damage_type | u8 | Damage flags |
| crit_type | u8 | 0=Normal, 1=Mini, 2=Crit |
| hitgroup | u8 | Body part (0-7) |

### 0x35 - BUILDING_BUILT
Engineer places building. Source: `player_builtobject`

| Field | Type | Description |
|-------|------|-------------|
| builder_steam_id | u64 | Engineer |
| building_type | u8 | Type |
| building_id | u16 | Entity index |
| level | u8 | Level (1-3) |
| position_x | f32 | X pos |
| position_y | f32 | Y pos |
| position_z | f32 | Z pos |

**Building Types:** 0=Sentry, 1=Dispenser, 2=Tele Entrance, 3=Tele Exit

### 0x36 - BUILDING_DESTROYED
Building destroyed. Source: `object_destroyed`

| Field | Type | Description |
|-------|------|-------------|
| owner_steam_id | u64 | Owner |
| attacker_steam_id | u64 | Destroyer |
| building_type | u8 | Type |
| building_id | u16 | Entity index |
| weapon | string | Weapon used |
| was_sapped | u8 | 1 if sapper |

### 0x37 - BUILDING_SAPPED
Spy saps building. Source: `player_sapped_object`

| Field | Type | Description |
|-------|------|-------------|
| spy_steam_id | u64 | Spy |
| owner_steam_id | u64 | Engineer |
| building_type | u8 | Type |
| sapper_id | u16 | Sapper entity |

### 0x38 - PROJECTILE_DEFLECTED
Pyro airblast. Source: `object_deflected`

| Field | Type | Description |
|-------|------|-------------|
| pyro_steam_id | u64 | Pyro |
| original_owner_steam_id | u64 | Original owner |
| weapon_id | u16 | 0 if player push |
| projectile_id | u16 | Projectile entity |

### 0x39 - PLAYER_IGNITED
Player on fire. Source: `player_ignited`

| Field | Type | Description |
|-------|------|-------------|
| victim_steam_id | u64 | Burning player |
| pyro_steam_id | u64 | Pyro |
| weapon_id | u16 | Weapon |

### 0x3A - PLAYER_EXTINGUISHED
Fire extinguished. Source: `player_extinguished`

| Field | Type | Description |
|-------|------|-------------|
| victim_steam_id | u64 | Extinguished player |
| healer_steam_id | u64 | Helper (0 if self) |
| item_def_index | u16 | Item used |

### 0x3B - PLAYER_JARATED
Player hit by Jarate. Source: `player_jarated`

| Field | Type | Description |
|-------|------|-------------|
| victim_steam_id | u64 | Victim |
| thrower_steam_id | u64 | Sniper |

### 0x3C - PLAYER_TELEPORTED
Player uses teleporter. Source: `player_teleported`

| Field | Type | Description |
|-------|------|-------------|
| player_steam_id | u64 | Player |
| builder_steam_id | u64 | Tele owner |
| distance | f32 | Distance traveled |

### 0x3D - PLAYER_SPAWN
Player spawns. Source: `player_spawn`

| Field | Type | Description |
|-------|------|-------------|
| player_steam_id | u64 | Player |
| team | u8 | Team ID |
| class | u8 | Class ID |

### 0x3E - MEDIC_DEATH
Medic dies. Source: `medic_death`

| Field | Type | Description |
|-------|------|-------------|
| medic_steam_id | u64 | Dead Medic |
| attacker_steam_id | u64 | Killer |
| healing_done | u16 | Healing this life |
| had_uber | u8 | 1 if had full Über |

### 0x3F - SENTRY_ATTACK
Sentry fires at target.

| Field | Type | Description |
|-------|------|-------------|
| sentry_id | u16 | Sentry entity |
| owner_steam_id | u64 | Engineer |
| target_steam_id | u64 | Target |
| sentry_level | u8 | Level (1-3) |

---

## Game Events (0x40-0x5F)

### 0x40 - ROUND_START
Round begins.

| Field | Type | Description |
|-------|------|-------------|
| map_name | string | Current map |
| round_number | u8 | Round number |
| time_limit | u16 | Time limit (seconds) |

### 0x41 - ROUND_END
Round ends.

| Field | Type | Description |
|-------|------|-------------|
| winning_team | u8 | Winner (0=Stalemate) |
| reason | u8 | Win reason |
| round_time | u16 | Duration (seconds) |
| red_score | u16 | RED score |
| blu_score | u16 | BLU score |

### 0x42 - MAP_CHANGE
Server changing maps.

| Field | Type | Description |
|-------|------|-------------|
| old_map | string | Previous map |
| new_map | string | New map |

---

## Ghost/Cross-Server Events (0x70-0x7F)

See [CROSS_SERVER.md](./CROSS_SERVER.md) for detailed documentation.

| ID | Name | Description |
|----|------|-------------|
| 0x70 | PLAYER_SYNC | Full player state for ghost |
| 0x71 | PLAYER_POSITION | Position update (high freq) |
| 0x72 | GHOST_REMOVE | Remove ghost entity |
| 0x73 | HEAL_REQUEST | Request heal on remote player |
| 0x74 | HEAL_CONFIRM | Confirm heal applied |
| 0x75 | UBER_SHARE | Share Über between servers |
| 0x76 | DAMAGE_REQUEST | Request damage on remote player |
| 0x77 | DAMAGE_CONFIRM | Confirm damage applied |
