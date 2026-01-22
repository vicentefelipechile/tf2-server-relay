# Cross-Server Mechanics

> Ghost Player System and Medic Healing implementation details.

## Overview

Cross-server gameplay requires synchronizing player state between multiple TF2 servers. This is achieved through the **Ghost Player System** - virtual representations of remote players that can receive interactions.

---

## Ghost Player System

### What is a Ghost?

A ghost is a fake player entity (bot or fake client) created on Server A that represents a real player on Server B.

```
┌─────────────────────────────────────────────────────────────────┐
│                         SERVER A                                │
│                                                                 │
│   ┌─────────────┐    ┌─────────────┐    ┌─────────────┐         │
│   │ Real Player │    │ Real Player │    │   GHOST     │         │
│   │   Alice     │    │    Bob      │    │   (Charlie) │◄─────┐  │
│   │  (local)    │    │  (local)    │    │  (remote)   │      │  │
│   └─────────────┘    └─────────────┘    └─────────────┘      │  │
│                                                              │  │
└──────────────────────────────────────────────────────────────┼──┘
                                                               │
                              RELAY                            │
                                                               │
┌──────────────────────────────────────────────────────────────┼──┐
│                         SERVER B                             │  │
│                                                              │  │
│   ┌─────────────┐    ┌─────────────┐    ┌─────────────┐      │  │
│   │   GHOST     │    │   GHOST     │    │ Real Player │──────┘  │
│   │  (Alice)    │    │   (Bob)     │    │   Charlie   │         │
│   │  (remote)   │    │  (remote)   │    │  (local)    │         │
│   └─────────────┘    └─────────────┘    └─────────────┘         │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Ghost Properties

Ghosts maintain synchronized state:
- **Position** (x, y, z) + angle
- **Team** (RED/BLU/Spectator)
- **Class** (Scout, Soldier, etc.)
- **Health** (current and max)
- **Alive/Dead** status
- **Visual appearance** (model, cosmetics if possible)

### Ghost Events

#### PLAYER_SYNC (0x70)
Full state sync for ghost creation/update.

```
┌──────────┬─────────────┬──────┬───────┬────────┬────────────┐
│ steam_id │ player_name │ team │ class │ health │ max_health │
│   u64    │   string    │  u8  │  u8   │  u16   │    u16     │
├──────────┴─────────────┴──────┴───────┴────────┴────────────┤
│ is_alive │ position_x │ position_y │ position_z │ angle_yaw │
│    u8    │    f32     │    f32     │    f32     │    f32    │
└──────────┴────────────┴────────────┴────────────┴───────────┘
```

#### PLAYER_POSITION (0x71)
Lightweight position update (sent at 20Hz by default).

```
┌──────────┬────────────┬────────────┬────────────┬───────────┐
│ steam_id │ position_x │ position_y │ position_z │ angle_yaw │
│   u64    │    f32     │    f32     │    f32     │    f32    │
├──────────┴────────────┴────────────┴────────────┴───────────┤
│ velocity_x │ velocity_y │ velocity_z │
│    f32     │    f32     │    f32     │
└────────────┴────────────┴────────────┘
```

#### GHOST_REMOVE (0x72)
Remove ghost when player disconnects.

```
┌──────────┬────────┐
│ steam_id │ reason │
│   u64    │ string │
└──────────┴────────┘
```

---

## Cross-Server Medic Healing

**Priority: CRITICAL** - This is the most important cross-server feature.

### The Problem

When a Medic on Server A heals a ghost (representing Player X from Server B), the healing must be applied to the **real player on Server B**, not just the ghost.

### Solution: Request/Confirm Pattern

```
┌─────────────────┐                              ┌─────────────────┐
│    SERVER A     │                              │    SERVER B     │
│                 │                              │                 │
│  Medic ───────► Ghost (Player X)               │  Player X       │
│                 │                              │                 │
│  1. Detect heal │                              │                 │
│     attempt     │                              │                 │
│                 │                              │                 │
│  2. Send        │        HEAL_REQUEST          │                 │
│     request ────┼───────────────────────────────►  5. Apply heal │
│                 │           RELAY              │     to real     │
│                 │                              │     player      │
│                 │                              │                 │
│  7. Update      │        HEAL_CONFIRM          │  6. Send        │
│     ghost HP ◄──┼──────────────────────────────┤     confirm     │
│                 │           RELAY              │                 │
│                 │                              │                 │
└─────────────────┘                              └─────────────────┘
```

### Step-by-Step Workflow

| Step | Server | Action |
|------|--------|--------|
| 1 | A | Medic targets ghost with Medi Gun |
| 2 | A | Plugin hooks healing, captures healer/target/amount |
| 3 | A | Send HEAL_REQUEST (target's Steam ID, heal amount) |
| 4 | Relay | Route to server owning target Steam ID |
| 5 | B | Receive request, find real player |
| 6 | B | Apply healing with `SetEntityHealth` or `TF2_AddCondition` |
| 7 | B | Send HEAL_CONFIRM with new health value |
| 8 | Relay | Route back to healer's server |
| 9 | A | Update ghost's displayed health |

### Healing Events

#### HEAL_REQUEST (0x73)

```
┌────────────────┬────────────────┬─────────────┬───────────┐
│ healer_steam_id│ target_steam_id│ heal_amount │ heal_rate │
│      u64       │      u64       │     u16     │    u8     │
├────────────────┼────────────────┼─────────────┼───────────┤
│ medigun_type   │ is_healing     │
│      u8        │      u8        │
└────────────────┴────────────────┘
```

- `medigun_type`: 0=Stock, 1=Kritz, 2=QuickFix, 3=Vaccinator
- `is_healing`: 1=started, 0=stopped (for continuous heal tracking)

#### HEAL_CONFIRM (0x74)

```
┌────────────────┬────────────┬────────────┬────────────────┬──────────────┐
│ target_steam_id│ new_health │ max_health │ overheal_amount│ heal_applied │
│      u64       │    u16     │    u16     │      u16       │     u16      │
└────────────────┴────────────┴────────────┴────────────────┴──────────────┘
```

---

## Cross-Server ÜberCharge

When the Über target is on a different server, the invulnerability effect must be applied remotely.

### UBER_SHARE (0x75)

```
┌────────────────┬────────────────┬───────────┬──────────────────┬───────────┐
│ medic_steam_id │ target_steam_id│ uber_type │ uber_duration_ms │ is_active │
│      u64       │      u64       │    u8     │       u16        │    u8     │
└────────────────┴────────────────┴───────────┴──────────────────┴───────────┘
```

### Workflow

1. Medic activates Über on ghost target
2. Server A sends UBER_SHARE with `is_active=1`
3. Server B applies `TF2_AddCondition(TFCond_Ubercharged)`
4. Ghost on Server A shows Über visual effect
5. When Über ends, send UBER_SHARE with `is_active=0`

### Über Types

| ID | Type | Medi Gun |
|----|------|----------|
| 0 | INVULNERABILITY | Stock |
| 1 | CRITBOOST | Kritzkrieg |
| 2 | MEGAHEAL | Quick-Fix |
| 3 | BULLETBLOCK | Vaccinator (bullets) |
| 4 | BLASTBLOCK | Vaccinator (explosives) |
| 5 | FIREBLOCK | Vaccinator (fire) |

---

## Cross-Server Damage

Similar pattern for damage dealt to ghost entities.

### DAMAGE_REQUEST (0x76)

```
┌──────────────────┬────────────────┬───────────────┬───────────┐
│ attacker_steam_id│ victim_steam_id│ damage_amount │ weapon_id │
│       u64        │      u64       │      u16      │    u16    │
├──────────────────┼────────────────┼───────────────┼───────────┤
│ damage_type      │ crit_type      │ hitgroup      │
│      u8          │      u8        │     u8        │
└──────────────────┴────────────────┴───────────────┘
```

### DAMAGE_CONFIRM (0x77)

```
┌────────────────┬────────────┬────────────────┬─────────┐
│ victim_steam_id│ new_health │ damage_applied │ is_dead │
│      u64       │    u16     │      u16       │   u8    │
└────────────────┴────────────┴────────────────┴─────────┘
```

---

## Latency Considerations

### Target Latency
- Local infrastructure: <50ms round-trip
- Maximum acceptable: 100ms

### Predictive Healing

To make healing feel responsive despite network latency:

```
Without Prediction:
  Medic heals → Wait for confirm → Show result
  [====== 50-100ms delay ======]

With Prediction:
  Medic heals → Show immediate result → Reconcile on confirm
  [Instant visual] ← Correction if needed
```

**Config option**: `predictive_healing = true` (default)

1. Server A optimistically applies heal to ghost immediately
2. When HEAL_CONFIRM arrives, reconcile with actual value
3. If difference is significant, smoothly interpolate to correct value

---

## Implementation Notes

### SourceMod Side

```sourcepawn
// Detect healing on ghost entity
public Action OnTakeDamageAlive(int victim, int &attacker, ...) {
    if (IsGhostEntity(victim) && IsPlayerMedic(attacker)) {
        // Don't apply locally, send to relay
        SendHealRequest(GetGhostSteamId(victim), GetHealAmount());
        return Plugin_Handled;
    }
    return Plugin_Continue;
}
```

### Rust Relay Side

```rust
async fn handle_heal_request(&self, packet: HealRequest) {
    // Find which server owns the target
    if let Some(target_server) = self.find_server_by_steam_id(packet.target_steam_id) {
        // Forward to target server
        target_server.send(packet).await;
    }
}
```

### Ghost Entity Creation

```sourcepawn
// Create ghost for remote player
int CreateGhost(const char[] steamid, const char[] name, int team, int class) {
    int bot = CreateFakeClient(name);
    ChangeClientTeam(bot, team);
    TF2_SetPlayerClass(bot, view_as<TFClassType>(class));
    SetGhostSteamId(bot, steamid);
    return bot;
}
```
