//! Event type definitions for TF2 Server Relay.
//!
//! Event ID ranges (from BLUEPRINT.xml):
//! - 0x00-0x0F: System events (handshake, heartbeat)
//! - 0x10-0x1F: Chat events
//! - 0x20-0x2F: Player events (death, connect)
//! - 0x30-0x3F: Gameplay events (heal, damage, buildings)
//! - 0x40-0x5F: Game events (round, map)
//! - 0x60-0x7F: Custom/extension events
//! - 0x70-0x7F: Ghost sync & cross-server events

use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::fmt;

use crate::error::{RelayError, Result};
use crate::protocol::{read_string, write_string};

// ============================================================================
// Event Type IDs
// ============================================================================

/// System Event IDs (0x00-0x0F)
pub mod system {
    pub const HANDSHAKE: u8 = 0x00;
    pub const HANDSHAKE_ACK: u8 = 0x01;
    pub const HEARTBEAT: u8 = 0x02;
    pub const HEARTBEAT_ACK: u8 = 0x03;
    pub const SERVER_CONNECT: u8 = 0x04;
    pub const SERVER_DISCONNECT: u8 = 0x05;
    pub const ERROR: u8 = 0x06;
}

/// Chat Event IDs (0x10-0x1F)
pub mod chat {
    pub const CHAT_MESSAGE: u8 = 0x10;
    pub const ADMIN_MESSAGE: u8 = 0x11;
}

/// Player Event IDs (0x20-0x3F)
pub mod player {
    pub const PLAYER_DEATH: u8 = 0x20;
    pub const PLAYER_CONNECT: u8 = 0x21;
    pub const PLAYER_DISCONNECT: u8 = 0x22;
    pub const PLAYER_TEAM_CHANGE: u8 = 0x23;
    pub const PLAYER_CLASS_CHANGE: u8 = 0x24;
}

/// Gameplay Event IDs (0x30-0x3F)
/// Note: These are protocol constants, some are reserved for future cross-server features
#[allow(dead_code)]
pub mod gameplay {
    pub const PLAYER_HEALED: u8 = 0x30;
    pub const BUILDING_HEALED: u8 = 0x31;
    pub const UBER_DEPLOYED: u8 = 0x32;
    pub const PLAYER_INVULNED: u8 = 0x33;
    pub const PLAYER_HURT: u8 = 0x34;
    pub const BUILDING_BUILT: u8 = 0x35;
    pub const BUILDING_DESTROYED: u8 = 0x36;
    pub const BUILDING_SAPPED: u8 = 0x37;
    pub const PROJECTILE_DEFLECTED: u8 = 0x38;
    pub const PLAYER_IGNITED: u8 = 0x39;
    pub const PLAYER_EXTINGUISHED: u8 = 0x3A;
    pub const PLAYER_JARATED: u8 = 0x3B;
    pub const PLAYER_TELEPORTED: u8 = 0x3C;
    pub const PLAYER_SPAWN: u8 = 0x3D;
    pub const MEDIC_DEATH: u8 = 0x3E;
    pub const SENTRY_ATTACK: u8 = 0x3F;
}

/// Game Event IDs (0x40-0x5F)
#[allow(dead_code)]
pub mod game {
    pub const ROUND_START: u8 = 0x40;
    pub const ROUND_END: u8 = 0x41;
    pub const MAP_CHANGE: u8 = 0x42;
    pub const GAME_MODE_INFO: u8 = 0x43;
}

/// Ghost/Cross-Server Event IDs (0x70-0x7F)
#[allow(dead_code)]
pub mod ghost {
    pub const PLAYER_SYNC: u8 = 0x70;
    pub const PLAYER_POSITION: u8 = 0x71;
    pub const GHOST_REMOVE: u8 = 0x72;
    pub const HEAL_REQUEST: u8 = 0x73;
    pub const HEAL_CONFIRM: u8 = 0x74;
    pub const UBER_SHARE: u8 = 0x75;
    pub const DAMAGE_REQUEST: u8 = 0x76;
    pub const DAMAGE_CONFIRM: u8 = 0x77;
}

// ============================================================================
// Team & Class definitions
// ============================================================================

/// TF2 team IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Team {
    Unassigned = 0,
    Spectator = 1,
    Red = 2,
    Blu = 3,
}

impl Team {
    pub fn from_u8(value: u8) -> Self {
        match value {
            0 => Team::Unassigned,
            1 => Team::Spectator,
            2 => Team::Red,
            3 => Team::Blu,
            _ => Team::Unassigned,
        }
    }

    pub fn color(&self) -> u32 {
        match self {
            Team::Unassigned => 0xCCCCCC,
            Team::Spectator => 0xBDBDBD,
            Team::Red => 0xBD3B3B,
            Team::Blu => 0x5B8DD8,
        }
    }
}

impl fmt::Display for Team {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Team::Unassigned => write!(f, "Unassigned"),
            Team::Spectator => write!(f, "Spectator"),
            Team::Red => write!(f, "RED"),
            Team::Blu => write!(f, "BLU"),
        }
    }
}

/// TF2 class IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TfClass {
    Unknown = 0,
    Scout = 1,
    Sniper = 2,
    Soldier = 3,
    Demoman = 4,
    Medic = 5,
    Heavy = 6,
    Pyro = 7,
    Spy = 8,
    Engineer = 9,
}

impl TfClass {
    pub fn from_u8(value: u8) -> Self {
        match value {
            1 => TfClass::Scout,
            2 => TfClass::Sniper,
            3 => TfClass::Soldier,
            4 => TfClass::Demoman,
            5 => TfClass::Medic,
            6 => TfClass::Heavy,
            7 => TfClass::Pyro,
            8 => TfClass::Spy,
            9 => TfClass::Engineer,
            _ => TfClass::Unknown,
        }
    }
}

impl fmt::Display for TfClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TfClass::Unknown => write!(f, "Unknown"),
            TfClass::Scout => write!(f, "Scout"),
            TfClass::Sniper => write!(f, "Sniper"),
            TfClass::Soldier => write!(f, "Soldier"),
            TfClass::Demoman => write!(f, "Demoman"),
            TfClass::Medic => write!(f, "Medic"),
            TfClass::Heavy => write!(f, "Heavy"),
            TfClass::Pyro => write!(f, "Pyro"),
            TfClass::Spy => write!(f, "Spy"),
            TfClass::Engineer => write!(f, "Engineer"),
        }
    }
}

// ============================================================================
// Event enum
// ============================================================================

/// Relay event containing parsed event data.
#[derive(Debug, Clone)]
pub enum Event {
    // System events
    Handshake(HandshakeEvent),
    HandshakeAck(HandshakeAckEvent),
    Heartbeat(HeartbeatEvent),
    HeartbeatAck(HeartbeatAckEvent),
    ServerConnect(ServerConnectEvent),
    ServerDisconnect(ServerDisconnectEvent),
    Error(ErrorEvent),

    // Chat events
    ChatMessage(ChatMessageEvent),
    AdminMessage(AdminMessageEvent),

    // Player events
    PlayerDeath(PlayerDeathEvent),
    PlayerConnect(PlayerConnectEvent),
    PlayerDisconnect(PlayerDisconnectEvent),
    PlayerTeamChange(PlayerTeamChangeEvent),
    PlayerClassChange(PlayerClassChangeEvent),

    // Gameplay events
    PlayerHealed(PlayerHealedEvent),
    PlayerHurt(PlayerHurtEvent),
    UberDeployed(UberDeployedEvent),
    PlayerSpawn(PlayerSpawnEvent),

    // Game events
    RoundStart(RoundStartEvent),
    RoundEnd(RoundEndEvent),
    MapChange(MapChangeEvent),

    // Ghost/Cross-server events
    PlayerSync(PlayerSyncEvent),
    PlayerPosition(PlayerPositionEvent),
    GhostRemove(GhostRemoveEvent),
    HealRequest(HealRequestEvent),
    HealConfirm(HealConfirmEvent),
    DamageRequest(DamageRequestEvent),
    DamageConfirm(DamageConfirmEvent),

    /// Unknown event type (for forward compatibility).
    Unknown {
        event_type: u8,
        payload: Bytes,
    },
}

impl Event {
    /// Get the event type ID.
    pub fn event_type(&self) -> u8 {
        match self {
            Event::Handshake(_) => system::HANDSHAKE,
            Event::HandshakeAck(_) => system::HANDSHAKE_ACK,
            Event::Heartbeat(_) => system::HEARTBEAT,
            Event::HeartbeatAck(_) => system::HEARTBEAT_ACK,
            Event::ServerConnect(_) => system::SERVER_CONNECT,
            Event::ServerDisconnect(_) => system::SERVER_DISCONNECT,
            Event::Error(_) => system::ERROR,
            Event::ChatMessage(_) => chat::CHAT_MESSAGE,
            Event::AdminMessage(_) => chat::ADMIN_MESSAGE,
            Event::PlayerDeath(_) => player::PLAYER_DEATH,
            Event::PlayerConnect(_) => player::PLAYER_CONNECT,
            Event::PlayerDisconnect(_) => player::PLAYER_DISCONNECT,
            Event::PlayerTeamChange(_) => player::PLAYER_TEAM_CHANGE,
            Event::PlayerClassChange(_) => player::PLAYER_CLASS_CHANGE,
            Event::PlayerHealed(_) => gameplay::PLAYER_HEALED,
            Event::PlayerHurt(_) => gameplay::PLAYER_HURT,
            Event::UberDeployed(_) => gameplay::UBER_DEPLOYED,
            Event::PlayerSpawn(_) => gameplay::PLAYER_SPAWN,
            Event::RoundStart(_) => game::ROUND_START,
            Event::RoundEnd(_) => game::ROUND_END,
            Event::MapChange(_) => game::MAP_CHANGE,
            Event::PlayerSync(_) => ghost::PLAYER_SYNC,
            Event::PlayerPosition(_) => ghost::PLAYER_POSITION,
            Event::GhostRemove(_) => ghost::GHOST_REMOVE,
            Event::HealRequest(_) => ghost::HEAL_REQUEST,
            Event::HealConfirm(_) => ghost::HEAL_CONFIRM,
            Event::DamageRequest(_) => ghost::DAMAGE_REQUEST,
            Event::DamageConfirm(_) => ghost::DAMAGE_CONFIRM,
            Event::Unknown { event_type, .. } => *event_type,
        }
    }

    /// Parse an event from a packet type and payload.
    pub fn parse(event_type: u8, payload: Bytes) -> Result<Self> {
        let mut buf = payload.clone();

        let event = match event_type {
            system::HANDSHAKE => Event::Handshake(HandshakeEvent::parse(&mut buf)?),
            system::HANDSHAKE_ACK => Event::HandshakeAck(HandshakeAckEvent::parse(&mut buf)?),
            system::HEARTBEAT => Event::Heartbeat(HeartbeatEvent::parse(&mut buf)?),
            system::HEARTBEAT_ACK => Event::HeartbeatAck(HeartbeatAckEvent::parse(&mut buf)?),
            system::SERVER_CONNECT => Event::ServerConnect(ServerConnectEvent::parse(&mut buf)?),
            system::SERVER_DISCONNECT => {
                Event::ServerDisconnect(ServerDisconnectEvent::parse(&mut buf)?)
            }
            system::ERROR => Event::Error(ErrorEvent::parse(&mut buf)?),

            chat::CHAT_MESSAGE => Event::ChatMessage(ChatMessageEvent::parse(&mut buf)?),
            chat::ADMIN_MESSAGE => Event::AdminMessage(AdminMessageEvent::parse(&mut buf)?),

            player::PLAYER_DEATH => Event::PlayerDeath(PlayerDeathEvent::parse(&mut buf)?),
            player::PLAYER_CONNECT => Event::PlayerConnect(PlayerConnectEvent::parse(&mut buf)?),
            player::PLAYER_DISCONNECT => {
                Event::PlayerDisconnect(PlayerDisconnectEvent::parse(&mut buf)?)
            }
            player::PLAYER_TEAM_CHANGE => {
                Event::PlayerTeamChange(PlayerTeamChangeEvent::parse(&mut buf)?)
            }
            player::PLAYER_CLASS_CHANGE => {
                Event::PlayerClassChange(PlayerClassChangeEvent::parse(&mut buf)?)
            }

            gameplay::PLAYER_HEALED => Event::PlayerHealed(PlayerHealedEvent::parse(&mut buf)?),
            gameplay::PLAYER_HURT => Event::PlayerHurt(PlayerHurtEvent::parse(&mut buf)?),
            gameplay::UBER_DEPLOYED => Event::UberDeployed(UberDeployedEvent::parse(&mut buf)?),
            gameplay::PLAYER_SPAWN => Event::PlayerSpawn(PlayerSpawnEvent::parse(&mut buf)?),

            game::ROUND_START => Event::RoundStart(RoundStartEvent::parse(&mut buf)?),
            game::ROUND_END => Event::RoundEnd(RoundEndEvent::parse(&mut buf)?),
            game::MAP_CHANGE => Event::MapChange(MapChangeEvent::parse(&mut buf)?),

            ghost::PLAYER_SYNC => Event::PlayerSync(PlayerSyncEvent::parse(&mut buf)?),
            ghost::PLAYER_POSITION => Event::PlayerPosition(PlayerPositionEvent::parse(&mut buf)?),
            ghost::GHOST_REMOVE => Event::GhostRemove(GhostRemoveEvent::parse(&mut buf)?),
            ghost::HEAL_REQUEST => Event::HealRequest(HealRequestEvent::parse(&mut buf)?),
            ghost::HEAL_CONFIRM => Event::HealConfirm(HealConfirmEvent::parse(&mut buf)?),
            ghost::DAMAGE_REQUEST => Event::DamageRequest(DamageRequestEvent::parse(&mut buf)?),
            ghost::DAMAGE_CONFIRM => Event::DamageConfirm(DamageConfirmEvent::parse(&mut buf)?),

            _ => Event::Unknown {
                event_type,
                payload,
            },
        };

        Ok(event)
    }

    /// Serialize the event to bytes.
    pub fn serialize(&self) -> Bytes {
        let mut buf = BytesMut::new();

        match self {
            Event::Handshake(e) => e.serialize(&mut buf),
            Event::HandshakeAck(e) => e.serialize(&mut buf),
            Event::Heartbeat(e) => e.serialize(&mut buf),
            Event::HeartbeatAck(e) => e.serialize(&mut buf),
            Event::ServerConnect(e) => e.serialize(&mut buf),
            Event::ServerDisconnect(e) => e.serialize(&mut buf),
            Event::Error(e) => e.serialize(&mut buf),
            Event::ChatMessage(e) => e.serialize(&mut buf),
            Event::AdminMessage(e) => e.serialize(&mut buf),
            Event::PlayerDeath(e) => e.serialize(&mut buf),
            Event::PlayerConnect(e) => e.serialize(&mut buf),
            Event::PlayerDisconnect(e) => e.serialize(&mut buf),
            Event::PlayerTeamChange(e) => e.serialize(&mut buf),
            Event::PlayerClassChange(e) => e.serialize(&mut buf),
            Event::PlayerHealed(e) => e.serialize(&mut buf),
            Event::PlayerHurt(e) => e.serialize(&mut buf),
            Event::UberDeployed(e) => e.serialize(&mut buf),
            Event::PlayerSpawn(e) => e.serialize(&mut buf),
            Event::RoundStart(e) => e.serialize(&mut buf),
            Event::RoundEnd(e) => e.serialize(&mut buf),
            Event::MapChange(e) => e.serialize(&mut buf),
            Event::PlayerSync(e) => e.serialize(&mut buf),
            Event::PlayerPosition(e) => e.serialize(&mut buf),
            Event::GhostRemove(e) => e.serialize(&mut buf),
            Event::HealRequest(e) => e.serialize(&mut buf),
            Event::HealConfirm(e) => e.serialize(&mut buf),
            Event::DamageRequest(e) => e.serialize(&mut buf),
            Event::DamageConfirm(e) => e.serialize(&mut buf),
            Event::Unknown { payload, .. } => buf.extend_from_slice(payload),
        }

        buf.freeze()
    }

    /// Get a human-readable description of the event.
    pub fn description(&self) -> String {
        match self {
            Event::Handshake(e) => {
                format!("Handshake from server {} '{}'", e.server_id, e.server_name)
            }
            Event::HandshakeAck(e) => format!("Handshake ACK (success: {})", e.success),
            Event::Heartbeat(_) => "Heartbeat".to_string(),
            Event::HeartbeatAck(_) => "Heartbeat ACK".to_string(),
            Event::ServerConnect(e) => {
                format!("Server {} '{}' connected", e.server_id, e.server_name)
            }
            Event::ServerDisconnect(e) => {
                format!("Server {} disconnected: {}", e.server_id, e.reason)
            }
            Event::Error(e) => format!("Error {}: {}", e.error_code, e.error_message),
            Event::ChatMessage(e) => format!(
                "[{}] {}: {}",
                Team::from_u8(e.team),
                e.player_name,
                e.message
            ),
            Event::AdminMessage(e) => format!("[ADMIN] {}: {}", e.admin_name, e.message),
            Event::PlayerDeath(e) => format!(
                "{} killed {} with {}",
                e.attacker_name, e.victim_name, e.weapon
            ),
            Event::PlayerConnect(e) => format!("{} connected", e.player_name),
            Event::PlayerDisconnect(e) => format!("{} disconnected: {}", e.player_name, e.reason),
            Event::PlayerTeamChange(e) => {
                format!("{} joined {}", e.player_name, Team::from_u8(e.new_team))
            }
            Event::PlayerClassChange(e) => format!(
                "{} changed class to {}",
                e.player_name,
                TfClass::from_u8(e.new_class)
            ),
            Event::PlayerHealed(e) => format!("Player healed for {} HP", e.amount),
            Event::PlayerHurt(e) => format!("Player took {} damage", e.damage_amount),
            Event::UberDeployed(_) => "Über deployed!".to_string(),
            Event::PlayerSpawn(e) => format!("Player spawned as {}", TfClass::from_u8(e.class)),
            Event::RoundStart(e) => format!("Round {} started on {}", e.round_number, e.map_name),
            Event::RoundEnd(e) => format!("Round ended - {} wins!", Team::from_u8(e.winning_team)),
            Event::MapChange(e) => format!("Map change: {} → {}", e.old_map, e.new_map),
            Event::PlayerSync(_) => "Player sync".to_string(),
            Event::PlayerPosition(_) => "Position update".to_string(),
            Event::GhostRemove(_) => "Ghost removed".to_string(),
            Event::HealRequest(e) => format!("Heal request: {} HP", e.heal_amount),
            Event::HealConfirm(e) => format!("Heal confirmed: {} HP applied", e.heal_applied),
            Event::DamageRequest(e) => format!("Damage request: {} HP", e.damage_amount),
            Event::DamageConfirm(e) => format!("Damage confirmed: {} HP applied", e.damage_applied),
            Event::Unknown { event_type, .. } => format!("Unknown event 0x{:02X}", event_type),
        }
    }

    /// Check if this event should be broadcast to other servers.
    pub fn is_broadcastable(&self) -> bool {
        !matches!(
            self,
            Event::Handshake(_)
                | Event::HandshakeAck(_)
                | Event::Heartbeat(_)
                | Event::HeartbeatAck(_)
                | Event::Error(_)
                | Event::Unknown { .. }
        )
    }
}

// ============================================================================
// System Event Structures
// ============================================================================

#[derive(Debug, Clone)]
pub struct HandshakeEvent {
    pub server_id: u8,
    pub server_name: String,
    pub map_name: String,
    pub max_players: u8,
    pub current_players: u8,
}

impl HandshakeEvent {
    pub fn parse(buf: &mut impl Buf) -> Result<Self> {
        if buf.remaining() < 3 {
            return Err(RelayError::MalformedPacket {
                reason: "Handshake too short".to_string(),
            });
        }

        let server_id = buf.get_u8();
        let server_name = read_string(buf)?;
        let map_name = read_string(buf)?;
        let max_players = buf.get_u8();
        let current_players = buf.get_u8();

        Ok(Self {
            server_id,
            server_name,
            map_name,
            max_players,
            current_players,
        })
    }

    pub fn serialize(&self, buf: &mut impl BufMut) {
        buf.put_u8(self.server_id);
        write_string(buf, &self.server_name);
        write_string(buf, &self.map_name);
        buf.put_u8(self.max_players);
        buf.put_u8(self.current_players);
    }
}

#[derive(Debug, Clone)]
pub struct HandshakeAckEvent {
    pub success: bool,
    pub assigned_id: u8,
    pub connected_servers: u8,
}

impl HandshakeAckEvent {
    pub fn parse(buf: &mut impl Buf) -> Result<Self> {
        if buf.remaining() < 3 {
            return Err(RelayError::MalformedPacket {
                reason: "HandshakeAck too short".to_string(),
            });
        }

        Ok(Self {
            success: buf.get_u8() != 0,
            assigned_id: buf.get_u8(),
            connected_servers: buf.get_u8(),
        })
    }

    pub fn serialize(&self, buf: &mut impl BufMut) {
        buf.put_u8(if self.success { 1 } else { 0 });
        buf.put_u8(self.assigned_id);
        buf.put_u8(self.connected_servers);
    }
}

#[derive(Debug, Clone)]
pub struct HeartbeatEvent {
    pub timestamp: u32,
    pub current_players: u8,
}

impl HeartbeatEvent {
    pub fn parse(buf: &mut impl Buf) -> Result<Self> {
        if buf.remaining() < 5 {
            return Err(RelayError::MalformedPacket {
                reason: "Heartbeat too short".to_string(),
            });
        }

        Ok(Self {
            timestamp: buf.get_u32_le(),
            current_players: buf.get_u8(),
        })
    }

    pub fn serialize(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.timestamp);
        buf.put_u8(self.current_players);
    }
}

#[derive(Debug, Clone)]
pub struct HeartbeatAckEvent {
    pub timestamp: u32,
    pub connected_servers: u8,
}

impl HeartbeatAckEvent {
    pub fn parse(buf: &mut impl Buf) -> Result<Self> {
        if buf.remaining() < 5 {
            return Err(RelayError::MalformedPacket {
                reason: "HeartbeatAck too short".to_string(),
            });
        }

        Ok(Self {
            timestamp: buf.get_u32_le(),
            connected_servers: buf.get_u8(),
        })
    }

    pub fn serialize(&self, buf: &mut impl BufMut) {
        buf.put_u32_le(self.timestamp);
        buf.put_u8(self.connected_servers);
    }
}

#[derive(Debug, Clone)]
pub struct ServerConnectEvent {
    pub server_id: u8,
    pub server_name: String,
}

impl ServerConnectEvent {
    pub fn parse(buf: &mut impl Buf) -> Result<Self> {
        if buf.remaining() < 2 {
            return Err(RelayError::MalformedPacket {
                reason: "ServerConnect too short".to_string(),
            });
        }

        let server_id = buf.get_u8();
        let server_name = read_string(buf)?;

        Ok(Self {
            server_id,
            server_name,
        })
    }

    pub fn serialize(&self, buf: &mut impl BufMut) {
        buf.put_u8(self.server_id);
        write_string(buf, &self.server_name);
    }
}

#[derive(Debug, Clone)]
pub struct ServerDisconnectEvent {
    pub server_id: u8,
    pub reason: String,
}

impl ServerDisconnectEvent {
    pub fn parse(buf: &mut impl Buf) -> Result<Self> {
        if buf.remaining() < 2 {
            return Err(RelayError::MalformedPacket {
                reason: "ServerDisconnect too short".to_string(),
            });
        }

        let server_id = buf.get_u8();
        let reason = read_string(buf)?;

        Ok(Self { server_id, reason })
    }

    pub fn serialize(&self, buf: &mut impl BufMut) {
        buf.put_u8(self.server_id);
        write_string(buf, &self.reason);
    }
}

#[derive(Debug, Clone)]
pub struct ErrorEvent {
    pub error_code: u16,
    pub error_message: String,
}

impl ErrorEvent {
    pub fn parse(buf: &mut impl Buf) -> Result<Self> {
        if buf.remaining() < 3 {
            return Err(RelayError::MalformedPacket {
                reason: "Error event too short".to_string(),
            });
        }

        let error_code = buf.get_u16_le();
        let error_message = read_string(buf)?;

        Ok(Self {
            error_code,
            error_message,
        })
    }

    pub fn serialize(&self, buf: &mut impl BufMut) {
        buf.put_u16_le(self.error_code);
        write_string(buf, &self.error_message);
    }
}

// ============================================================================
// Chat Event Structures
// ============================================================================

#[derive(Debug, Clone)]
pub struct ChatMessageEvent {
    pub player_name: String,
    pub steam_id: u64,
    pub team: u8,
    pub chat_type: u8, // 0 = all, 1 = team
    pub message: String,
}

impl ChatMessageEvent {
    pub fn parse(buf: &mut impl Buf) -> Result<Self> {
        let player_name = read_string(buf)?;

        if buf.remaining() < 10 {
            return Err(RelayError::MalformedPacket {
                reason: "ChatMessage too short".to_string(),
            });
        }

        let steam_id = buf.get_u64_le();
        let team = buf.get_u8();
        let chat_type = buf.get_u8();
        let message = read_string(buf)?;

        Ok(Self {
            player_name,
            steam_id,
            team,
            chat_type,
            message,
        })
    }

    pub fn serialize(&self, buf: &mut impl BufMut) {
        write_string(buf, &self.player_name);
        buf.put_u64_le(self.steam_id);
        buf.put_u8(self.team);
        buf.put_u8(self.chat_type);
        write_string(buf, &self.message);
    }
}

#[derive(Debug, Clone)]
pub struct AdminMessageEvent {
    pub admin_name: String,
    pub message: String,
    pub color: u32,
}

impl AdminMessageEvent {
    pub fn parse(buf: &mut impl Buf) -> Result<Self> {
        let admin_name = read_string(buf)?;
        let message = read_string(buf)?;

        if buf.remaining() < 4 {
            return Err(RelayError::MalformedPacket {
                reason: "AdminMessage too short".to_string(),
            });
        }

        let color = buf.get_u32_le();

        Ok(Self {
            admin_name,
            message,
            color,
        })
    }

    pub fn serialize(&self, buf: &mut impl BufMut) {
        write_string(buf, &self.admin_name);
        write_string(buf, &self.message);
        buf.put_u32_le(self.color);
    }
}

// ============================================================================
// Player Event Structures
// ============================================================================

#[derive(Debug, Clone)]
pub struct PlayerDeathEvent {
    pub victim_name: String,
    pub victim_steam_id: u64,
    pub victim_team: u8,
    pub victim_class: u8,
    pub attacker_name: String,
    pub attacker_steam_id: u64,
    pub attacker_team: u8,
    pub attacker_class: u8,
    pub weapon: String,
    pub crit_type: u8, // 0 = normal, 1 = mini-crit, 2 = crit
    pub death_flags: u16,
}

impl PlayerDeathEvent {
    pub fn parse(buf: &mut impl Buf) -> Result<Self> {
        let victim_name = read_string(buf)?;

        if buf.remaining() < 10 {
            return Err(RelayError::MalformedPacket {
                reason: "PlayerDeath too short".to_string(),
            });
        }

        let victim_steam_id = buf.get_u64_le();
        let victim_team = buf.get_u8();
        let victim_class = buf.get_u8();
        let attacker_name = read_string(buf)?;

        if buf.remaining() < 10 {
            return Err(RelayError::MalformedPacket {
                reason: "PlayerDeath too short".to_string(),
            });
        }

        let attacker_steam_id = buf.get_u64_le();
        let attacker_team = buf.get_u8();
        let attacker_class = buf.get_u8();
        let weapon = read_string(buf)?;

        if buf.remaining() < 3 {
            return Err(RelayError::MalformedPacket {
                reason: "PlayerDeath too short".to_string(),
            });
        }

        let crit_type = buf.get_u8();
        let death_flags = buf.get_u16_le();

        Ok(Self {
            victim_name,
            victim_steam_id,
            victim_team,
            victim_class,
            attacker_name,
            attacker_steam_id,
            attacker_team,
            attacker_class,
            weapon,
            crit_type,
            death_flags,
        })
    }

    pub fn serialize(&self, buf: &mut impl BufMut) {
        write_string(buf, &self.victim_name);
        buf.put_u64_le(self.victim_steam_id);
        buf.put_u8(self.victim_team);
        buf.put_u8(self.victim_class);
        write_string(buf, &self.attacker_name);
        buf.put_u64_le(self.attacker_steam_id);
        buf.put_u8(self.attacker_team);
        buf.put_u8(self.attacker_class);
        write_string(buf, &self.weapon);
        buf.put_u8(self.crit_type);
        buf.put_u16_le(self.death_flags);
    }

    /// Check if death was a headshot.
    pub fn is_headshot(&self) -> bool {
        self.death_flags & 0x0001 != 0
    }

    /// Check if death was a backstab.
    pub fn is_backstab(&self) -> bool {
        self.death_flags & 0x0002 != 0
    }

    /// Check if death was a domination.
    pub fn is_domination(&self) -> bool {
        self.death_flags & 0x0008 != 0
    }

    /// Check if death was a revenge.
    pub fn is_revenge(&self) -> bool {
        self.death_flags & 0x0010 != 0
    }
}

#[derive(Debug, Clone)]
pub struct PlayerConnectEvent {
    pub player_name: String,
    pub steam_id: u64,
    pub ip_hash: u32,
}

impl PlayerConnectEvent {
    pub fn parse(buf: &mut impl Buf) -> Result<Self> {
        let player_name = read_string(buf)?;

        if buf.remaining() < 12 {
            return Err(RelayError::MalformedPacket {
                reason: "PlayerConnect too short".to_string(),
            });
        }

        let steam_id = buf.get_u64_le();
        let ip_hash = buf.get_u32_le();

        Ok(Self {
            player_name,
            steam_id,
            ip_hash,
        })
    }

    pub fn serialize(&self, buf: &mut impl BufMut) {
        write_string(buf, &self.player_name);
        buf.put_u64_le(self.steam_id);
        buf.put_u32_le(self.ip_hash);
    }
}

#[derive(Debug, Clone)]
pub struct PlayerDisconnectEvent {
    pub player_name: String,
    pub steam_id: u64,
    pub reason: String,
}

impl PlayerDisconnectEvent {
    pub fn parse(buf: &mut impl Buf) -> Result<Self> {
        let player_name = read_string(buf)?;

        if buf.remaining() < 8 {
            return Err(RelayError::MalformedPacket {
                reason: "PlayerDisconnect too short".to_string(),
            });
        }

        let steam_id = buf.get_u64_le();
        let reason = read_string(buf)?;

        Ok(Self {
            player_name,
            steam_id,
            reason,
        })
    }

    pub fn serialize(&self, buf: &mut impl BufMut) {
        write_string(buf, &self.player_name);
        buf.put_u64_le(self.steam_id);
        write_string(buf, &self.reason);
    }
}

#[derive(Debug, Clone)]
pub struct PlayerTeamChangeEvent {
    pub player_name: String,
    pub steam_id: u64,
    pub old_team: u8,
    pub new_team: u8,
}

impl PlayerTeamChangeEvent {
    pub fn parse(buf: &mut impl Buf) -> Result<Self> {
        let player_name = read_string(buf)?;

        if buf.remaining() < 10 {
            return Err(RelayError::MalformedPacket {
                reason: "PlayerTeamChange too short".to_string(),
            });
        }

        let steam_id = buf.get_u64_le();
        let old_team = buf.get_u8();
        let new_team = buf.get_u8();

        Ok(Self {
            player_name,
            steam_id,
            old_team,
            new_team,
        })
    }

    pub fn serialize(&self, buf: &mut impl BufMut) {
        write_string(buf, &self.player_name);
        buf.put_u64_le(self.steam_id);
        buf.put_u8(self.old_team);
        buf.put_u8(self.new_team);
    }
}

#[derive(Debug, Clone)]
pub struct PlayerClassChangeEvent {
    pub player_name: String,
    pub steam_id: u64,
    pub new_class: u8,
}

impl PlayerClassChangeEvent {
    pub fn parse(buf: &mut impl Buf) -> Result<Self> {
        let player_name = read_string(buf)?;

        if buf.remaining() < 9 {
            return Err(RelayError::MalformedPacket {
                reason: "PlayerClassChange too short".to_string(),
            });
        }

        let steam_id = buf.get_u64_le();
        let new_class = buf.get_u8();

        Ok(Self {
            player_name,
            steam_id,
            new_class,
        })
    }

    pub fn serialize(&self, buf: &mut impl BufMut) {
        write_string(buf, &self.player_name);
        buf.put_u64_le(self.steam_id);
        buf.put_u8(self.new_class);
    }
}

// ============================================================================
// Gameplay Event Structures
// ============================================================================

#[derive(Debug, Clone)]
pub struct PlayerHealedEvent {
    pub patient_steam_id: u64,
    pub healer_steam_id: u64,
    pub amount: u16,
    pub heal_source: u8,
}

impl PlayerHealedEvent {
    pub fn parse(buf: &mut impl Buf) -> Result<Self> {
        if buf.remaining() < 19 {
            return Err(RelayError::MalformedPacket {
                reason: "PlayerHealed too short".to_string(),
            });
        }

        Ok(Self {
            patient_steam_id: buf.get_u64_le(),
            healer_steam_id: buf.get_u64_le(),
            amount: buf.get_u16_le(),
            heal_source: buf.get_u8(),
        })
    }

    pub fn serialize(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.patient_steam_id);
        buf.put_u64_le(self.healer_steam_id);
        buf.put_u16_le(self.amount);
        buf.put_u8(self.heal_source);
    }
}

#[derive(Debug, Clone)]
pub struct PlayerHurtEvent {
    pub victim_steam_id: u64,
    pub attacker_steam_id: u64,
    pub damage_amount: u16,
    pub health_remaining: u16,
    pub weapon_id: u16,
    pub damage_type: u8,
    pub crit_type: u8,
    pub hitgroup: u8,
}

impl PlayerHurtEvent {
    pub fn parse(buf: &mut impl Buf) -> Result<Self> {
        if buf.remaining() < 23 {
            return Err(RelayError::MalformedPacket {
                reason: "PlayerHurt too short".to_string(),
            });
        }

        Ok(Self {
            victim_steam_id: buf.get_u64_le(),
            attacker_steam_id: buf.get_u64_le(),
            damage_amount: buf.get_u16_le(),
            health_remaining: buf.get_u16_le(),
            weapon_id: buf.get_u16_le(),
            damage_type: buf.get_u8(),
            crit_type: buf.get_u8(),
            hitgroup: buf.get_u8(),
        })
    }

    pub fn serialize(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.victim_steam_id);
        buf.put_u64_le(self.attacker_steam_id);
        buf.put_u16_le(self.damage_amount);
        buf.put_u16_le(self.health_remaining);
        buf.put_u16_le(self.weapon_id);
        buf.put_u8(self.damage_type);
        buf.put_u8(self.crit_type);
        buf.put_u8(self.hitgroup);
    }
}

#[derive(Debug, Clone)]
pub struct UberDeployedEvent {
    pub medic_steam_id: u64,
    pub target_steam_id: u64,
    pub uber_type: u8,
}

impl UberDeployedEvent {
    pub fn parse(buf: &mut impl Buf) -> Result<Self> {
        if buf.remaining() < 17 {
            return Err(RelayError::MalformedPacket {
                reason: "UberDeployed too short".to_string(),
            });
        }

        Ok(Self {
            medic_steam_id: buf.get_u64_le(),
            target_steam_id: buf.get_u64_le(),
            uber_type: buf.get_u8(),
        })
    }

    pub fn serialize(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.medic_steam_id);
        buf.put_u64_le(self.target_steam_id);
        buf.put_u8(self.uber_type);
    }
}

#[derive(Debug, Clone)]
pub struct PlayerSpawnEvent {
    pub player_steam_id: u64,
    pub team: u8,
    pub class: u8,
}

impl PlayerSpawnEvent {
    pub fn parse(buf: &mut impl Buf) -> Result<Self> {
        if buf.remaining() < 10 {
            return Err(RelayError::MalformedPacket {
                reason: "PlayerSpawn too short".to_string(),
            });
        }

        Ok(Self {
            player_steam_id: buf.get_u64_le(),
            team: buf.get_u8(),
            class: buf.get_u8(),
        })
    }

    pub fn serialize(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.player_steam_id);
        buf.put_u8(self.team);
        buf.put_u8(self.class);
    }
}

// ============================================================================
// Game Event Structures
// ============================================================================

#[derive(Debug, Clone)]
pub struct RoundStartEvent {
    pub map_name: String,
    pub round_number: u8,
    pub time_limit: u16,
}

impl RoundStartEvent {
    pub fn parse(buf: &mut impl Buf) -> Result<Self> {
        let map_name = read_string(buf)?;

        if buf.remaining() < 3 {
            return Err(RelayError::MalformedPacket {
                reason: "RoundStart too short".to_string(),
            });
        }

        let round_number = buf.get_u8();
        let time_limit = buf.get_u16_le();

        Ok(Self {
            map_name,
            round_number,
            time_limit,
        })
    }

    pub fn serialize(&self, buf: &mut impl BufMut) {
        write_string(buf, &self.map_name);
        buf.put_u8(self.round_number);
        buf.put_u16_le(self.time_limit);
    }
}

#[derive(Debug, Clone)]
pub struct RoundEndEvent {
    pub winning_team: u8,
    pub reason: u8,
    pub round_time: u16,
    pub red_score: u16,
    pub blu_score: u16,
}

impl RoundEndEvent {
    pub fn parse(buf: &mut impl Buf) -> Result<Self> {
        if buf.remaining() < 8 {
            return Err(RelayError::MalformedPacket {
                reason: "RoundEnd too short".to_string(),
            });
        }

        Ok(Self {
            winning_team: buf.get_u8(),
            reason: buf.get_u8(),
            round_time: buf.get_u16_le(),
            red_score: buf.get_u16_le(),
            blu_score: buf.get_u16_le(),
        })
    }

    pub fn serialize(&self, buf: &mut impl BufMut) {
        buf.put_u8(self.winning_team);
        buf.put_u8(self.reason);
        buf.put_u16_le(self.round_time);
        buf.put_u16_le(self.red_score);
        buf.put_u16_le(self.blu_score);
    }
}

#[derive(Debug, Clone)]
pub struct MapChangeEvent {
    pub old_map: String,
    pub new_map: String,
}

impl MapChangeEvent {
    pub fn parse(buf: &mut impl Buf) -> Result<Self> {
        let old_map = read_string(buf)?;
        let new_map = read_string(buf)?;

        Ok(Self { old_map, new_map })
    }

    pub fn serialize(&self, buf: &mut impl BufMut) {
        write_string(buf, &self.old_map);
        write_string(buf, &self.new_map);
    }
}

// ============================================================================
// Ghost/Cross-Server Event Structures
// ============================================================================

#[derive(Debug, Clone)]
pub struct PlayerSyncEvent {
    pub steam_id: u64,
    pub player_name: String,
    pub team: u8,
    pub class: u8,
    pub health: u16,
    pub max_health: u16,
    pub is_alive: bool,
    pub position_x: f32,
    pub position_y: f32,
    pub position_z: f32,
    pub angle_yaw: f32,
}

impl PlayerSyncEvent {
    pub fn parse(buf: &mut impl Buf) -> Result<Self> {
        if buf.remaining() < 8 {
            return Err(RelayError::MalformedPacket {
                reason: "PlayerSync too short".to_string(),
            });
        }

        let steam_id = buf.get_u64_le();
        let player_name = read_string(buf)?;

        if buf.remaining() < 23 {
            return Err(RelayError::MalformedPacket {
                reason: "PlayerSync too short".to_string(),
            });
        }

        Ok(Self {
            steam_id,
            player_name,
            team: buf.get_u8(),
            class: buf.get_u8(),
            health: buf.get_u16_le(),
            max_health: buf.get_u16_le(),
            is_alive: buf.get_u8() != 0,
            position_x: buf.get_f32_le(),
            position_y: buf.get_f32_le(),
            position_z: buf.get_f32_le(),
            angle_yaw: buf.get_f32_le(),
        })
    }

    pub fn serialize(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.steam_id);
        write_string(buf, &self.player_name);
        buf.put_u8(self.team);
        buf.put_u8(self.class);
        buf.put_u16_le(self.health);
        buf.put_u16_le(self.max_health);
        buf.put_u8(if self.is_alive { 1 } else { 0 });
        buf.put_f32_le(self.position_x);
        buf.put_f32_le(self.position_y);
        buf.put_f32_le(self.position_z);
        buf.put_f32_le(self.angle_yaw);
    }
}

#[derive(Debug, Clone)]
pub struct PlayerPositionEvent {
    pub steam_id: u64,
    pub position_x: f32,
    pub position_y: f32,
    pub position_z: f32,
    pub angle_yaw: f32,
    pub velocity_x: f32,
    pub velocity_y: f32,
    pub velocity_z: f32,
}

impl PlayerPositionEvent {
    pub fn parse(buf: &mut impl Buf) -> Result<Self> {
        if buf.remaining() < 36 {
            return Err(RelayError::MalformedPacket {
                reason: "PlayerPosition too short".to_string(),
            });
        }

        Ok(Self {
            steam_id: buf.get_u64_le(),
            position_x: buf.get_f32_le(),
            position_y: buf.get_f32_le(),
            position_z: buf.get_f32_le(),
            angle_yaw: buf.get_f32_le(),
            velocity_x: buf.get_f32_le(),
            velocity_y: buf.get_f32_le(),
            velocity_z: buf.get_f32_le(),
        })
    }

    pub fn serialize(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.steam_id);
        buf.put_f32_le(self.position_x);
        buf.put_f32_le(self.position_y);
        buf.put_f32_le(self.position_z);
        buf.put_f32_le(self.angle_yaw);
        buf.put_f32_le(self.velocity_x);
        buf.put_f32_le(self.velocity_y);
        buf.put_f32_le(self.velocity_z);
    }
}

#[derive(Debug, Clone)]
pub struct GhostRemoveEvent {
    pub steam_id: u64,
    pub reason: String,
}

impl GhostRemoveEvent {
    pub fn parse(buf: &mut impl Buf) -> Result<Self> {
        if buf.remaining() < 8 {
            return Err(RelayError::MalformedPacket {
                reason: "GhostRemove too short".to_string(),
            });
        }

        let steam_id = buf.get_u64_le();
        let reason = read_string(buf)?;

        Ok(Self { steam_id, reason })
    }

    pub fn serialize(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.steam_id);
        write_string(buf, &self.reason);
    }
}

#[derive(Debug, Clone)]
pub struct HealRequestEvent {
    pub healer_steam_id: u64,
    pub target_steam_id: u64,
    pub heal_amount: u16,
    pub heal_rate: u8,
    pub medigun_type: u8,
    pub is_healing: bool,
}

impl HealRequestEvent {
    pub fn parse(buf: &mut impl Buf) -> Result<Self> {
        if buf.remaining() < 21 {
            return Err(RelayError::MalformedPacket {
                reason: "HealRequest too short".to_string(),
            });
        }

        Ok(Self {
            healer_steam_id: buf.get_u64_le(),
            target_steam_id: buf.get_u64_le(),
            heal_amount: buf.get_u16_le(),
            heal_rate: buf.get_u8(),
            medigun_type: buf.get_u8(),
            is_healing: buf.get_u8() != 0,
        })
    }

    pub fn serialize(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.healer_steam_id);
        buf.put_u64_le(self.target_steam_id);
        buf.put_u16_le(self.heal_amount);
        buf.put_u8(self.heal_rate);
        buf.put_u8(self.medigun_type);
        buf.put_u8(if self.is_healing { 1 } else { 0 });
    }
}

#[derive(Debug, Clone)]
pub struct HealConfirmEvent {
    pub target_steam_id: u64,
    pub new_health: u16,
    pub max_health: u16,
    pub overheal_amount: u16,
    pub heal_applied: u16,
}

impl HealConfirmEvent {
    pub fn parse(buf: &mut impl Buf) -> Result<Self> {
        if buf.remaining() < 16 {
            return Err(RelayError::MalformedPacket {
                reason: "HealConfirm too short".to_string(),
            });
        }

        Ok(Self {
            target_steam_id: buf.get_u64_le(),
            new_health: buf.get_u16_le(),
            max_health: buf.get_u16_le(),
            overheal_amount: buf.get_u16_le(),
            heal_applied: buf.get_u16_le(),
        })
    }

    pub fn serialize(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.target_steam_id);
        buf.put_u16_le(self.new_health);
        buf.put_u16_le(self.max_health);
        buf.put_u16_le(self.overheal_amount);
        buf.put_u16_le(self.heal_applied);
    }
}

#[derive(Debug, Clone)]
pub struct DamageRequestEvent {
    pub attacker_steam_id: u64,
    pub victim_steam_id: u64,
    pub damage_amount: u16,
    pub weapon_id: u16,
    pub damage_type: u8,
    pub crit_type: u8,
    pub hitgroup: u8,
}

impl DamageRequestEvent {
    pub fn parse(buf: &mut impl Buf) -> Result<Self> {
        if buf.remaining() < 23 {
            return Err(RelayError::MalformedPacket {
                reason: "DamageRequest too short".to_string(),
            });
        }

        Ok(Self {
            attacker_steam_id: buf.get_u64_le(),
            victim_steam_id: buf.get_u64_le(),
            damage_amount: buf.get_u16_le(),
            weapon_id: buf.get_u16_le(),
            damage_type: buf.get_u8(),
            crit_type: buf.get_u8(),
            hitgroup: buf.get_u8(),
        })
    }

    pub fn serialize(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.attacker_steam_id);
        buf.put_u64_le(self.victim_steam_id);
        buf.put_u16_le(self.damage_amount);
        buf.put_u16_le(self.weapon_id);
        buf.put_u8(self.damage_type);
        buf.put_u8(self.crit_type);
        buf.put_u8(self.hitgroup);
    }
}

#[derive(Debug, Clone)]
pub struct DamageConfirmEvent {
    pub victim_steam_id: u64,
    pub new_health: u16,
    pub damage_applied: u16,
    pub is_dead: bool,
}

impl DamageConfirmEvent {
    pub fn parse(buf: &mut impl Buf) -> Result<Self> {
        if buf.remaining() < 13 {
            return Err(RelayError::MalformedPacket {
                reason: "DamageConfirm too short".to_string(),
            });
        }

        Ok(Self {
            victim_steam_id: buf.get_u64_le(),
            new_health: buf.get_u16_le(),
            damage_applied: buf.get_u16_le(),
            is_dead: buf.get_u8() != 0,
        })
    }

    pub fn serialize(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.victim_steam_id);
        buf.put_u16_le(self.new_health);
        buf.put_u16_le(self.damage_applied);
        buf.put_u8(if self.is_dead { 1 } else { 0 });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_message_roundtrip() {
        let event = ChatMessageEvent {
            player_name: "TestPlayer".to_string(),
            steam_id: 76561198000000000,
            team: 2,
            chat_type: 0,
            message: "Hello, world!".to_string(),
        };

        let mut buf = BytesMut::new();
        event.serialize(&mut buf);

        let parsed = ChatMessageEvent::parse(&mut buf.freeze()).unwrap();
        assert_eq!(parsed.player_name, event.player_name);
        assert_eq!(parsed.steam_id, event.steam_id);
        assert_eq!(parsed.message, event.message);
    }

    #[test]
    fn test_player_death_flags() {
        let event = PlayerDeathEvent {
            victim_name: "Victim".to_string(),
            victim_steam_id: 1,
            victim_team: 2,
            victim_class: 1,
            attacker_name: "Attacker".to_string(),
            attacker_steam_id: 2,
            attacker_team: 3,
            attacker_class: 2,
            weapon: "sniperrifle".to_string(),
            crit_type: 0,
            death_flags: 0x0001, // HEADSHOT
        };

        assert!(event.is_headshot());
        assert!(!event.is_backstab());
    }

    #[test]
    fn test_team_display() {
        assert_eq!(Team::Red.to_string(), "RED");
        assert_eq!(Team::Blu.to_string(), "BLU");
    }

    #[test]
    fn test_class_display() {
        assert_eq!(TfClass::Medic.to_string(), "Medic");
        assert_eq!(TfClass::from_u8(5), TfClass::Medic);
    }
}
