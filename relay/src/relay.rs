//! Event relay and routing hub for TF2 Server Relay.
//!
//! Central hub that manages all server connections and routes events between them.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use bytes::Bytes;
use tokio::sync::{mpsc, RwLock};

use crate::config::Config;
use crate::connection::ServerInfo;
use crate::error::{RelayError, Result};
use crate::events::{system, Event, ServerConnectEvent, ServerDisconnectEvent};
use crate::protocol::Packet;

/// Recorded event with metadata for TUI display.
#[derive(Debug, Clone)]
pub struct RecordedEvent {
    /// Server ID that generated the event.
    pub server_id: u8,
    /// The event itself.
    pub event: Event,
    /// Timestamp when the event was recorded.
    pub timestamp: Instant,
}

/// Statistics for the relay.
#[derive(Debug, Clone, Default)]
pub struct RelayStats {
    /// Total events relayed.
    pub total_events: u64,
    /// Total bytes transferred.
    pub bytes_transferred: u64,
    /// Events in the last second.
    pub events_per_second: f64,
    /// Start time.
    pub start_time: Option<Instant>,
}

/// Connected server entry.
struct ConnectedServer {
    /// Server information.
    info: ServerInfo,
    /// Outbound channel to send packets to this server.
    sender: mpsc::Sender<Bytes>,
}

/// Central relay hub.
pub struct Relay {
    /// Configuration.
    config: Arc<Config>,
    /// Connected servers (server_id -> server).
    servers: RwLock<HashMap<u8, ConnectedServer>>,
    /// Player to server mapping (steam_id -> server_id).
    player_map: RwLock<HashMap<u64, u8>>,
    /// Event history for TUI.
    event_history: RwLock<Vec<RecordedEvent>>,
    /// Statistics.
    stats: RwLock<RelayStats>,
    /// Event channel for TUI updates.
    event_tx: mpsc::Sender<RecordedEvent>,
    /// Event channel receiver (for TUI).
    event_rx: RwLock<Option<mpsc::Receiver<RecordedEvent>>>,
}

impl Relay {
    /// Create a new relay instance.
    pub fn new(config: Arc<Config>) -> Self {
        let (event_tx, event_rx) = mpsc::channel(1000);

        Self {
            config,
            servers: RwLock::new(HashMap::new()),
            player_map: RwLock::new(HashMap::new()),
            event_history: RwLock::new(Vec::new()),
            stats: RwLock::new(RelayStats {
                start_time: Some(Instant::now()),
                ..Default::default()
            }),
            event_tx,
            event_rx: RwLock::new(Some(event_rx)),
        }
    }

    /// Take the event receiver (for TUI).
    pub async fn take_event_receiver(&self) -> Option<mpsc::Receiver<RecordedEvent>> {
        self.event_rx.write().await.take()
    }

    /// Register a new server connection.
    pub async fn register_server(
        &self,
        server_id: u8,
        info: ServerInfo,
        sender: mpsc::Sender<Bytes>,
    ) -> Result<()> {
        let mut servers = self.servers.write().await;

        if servers.contains_key(&server_id) {
            return Err(RelayError::DuplicateId { id: server_id });
        }

        tracing::info!(
            "Server {} '{}' registered (map: {}, players: {}/{})",
            server_id,
            info.server_name,
            info.map_name,
            info.current_players,
            info.max_players
        );

        // Notify other servers
        let connect_event = Event::ServerConnect(ServerConnectEvent {
            server_id,
            server_name: info.server_name.clone(),
        });

        // Send to all existing servers
        for (&id, server) in servers.iter() {
            if id != server_id {
                let payload = connect_event.serialize();
                let packet = Packet::new(system::SERVER_CONNECT, 0, 0, payload);
                let _ = server.sender.send(packet.serialize()).await;
            }
        }

        servers.insert(server_id, ConnectedServer { info, sender });

        Ok(())
    }

    /// Unregister a server connection.
    pub async fn unregister_server(&self, server_id: u8) {
        let mut servers = self.servers.write().await;

        if let Some(server) = servers.remove(&server_id) {
            tracing::info!("Server {} '{}' unregistered", server_id, server.info.server_name);

            // Notify other servers
            let disconnect_event = Event::ServerDisconnect(ServerDisconnectEvent {
                server_id,
                reason: "Disconnected".to_string(),
            });

            for (&id, other_server) in servers.iter() {
                if id != server_id {
                    let payload = disconnect_event.serialize();
                    let packet = Packet::new(system::SERVER_DISCONNECT, 0, 0, payload);
                    let _ = other_server.sender.send(packet.serialize()).await;
                }
            }
        }

        // Remove players from this server
        let mut player_map = self.player_map.write().await;
        player_map.retain(|_, &mut sid| sid != server_id);
    }

    /// Check if a server ID is connected.
    pub fn is_server_connected(&self, server_id: u8) -> bool {
        // Use try_read to avoid blocking
        if let Ok(servers) = self.servers.try_read() {
            servers.contains_key(&server_id)
        } else {
            false
        }
    }

    /// Get the number of connected servers.
    pub fn connected_count(&self) -> usize {
        if let Ok(servers) = self.servers.try_read() {
            servers.len()
        } else {
            0
        }
    }

    /// Get information about all connected servers.
    pub async fn get_servers(&self) -> Vec<ServerInfo> {
        let servers = self.servers.read().await;
        servers.values().map(|s| s.info.clone()).collect()
    }

    /// Broadcast an event to all servers except the origin.
    pub async fn broadcast(&self, origin_server_id: u8, event: Event) {
        let servers = self.servers.read().await;

        // Check config for what to broadcast
        let should_broadcast = match &event {
            Event::ChatMessage(_) | Event::AdminMessage(_) => self.config.relay.broadcast_chat,
            Event::PlayerDeath(_) => self.config.relay.broadcast_deaths,
            Event::PlayerConnect(_) | Event::PlayerDisconnect(_) => {
                self.config.relay.broadcast_connect
            }
            Event::HealRequest(_) | Event::HealConfirm(_) => self.config.relay.enable_cross_healing,
            Event::DamageRequest(_) | Event::DamageConfirm(_) => {
                self.config.relay.enable_cross_damage
            }
            _ => true,
        };

        if !should_broadcast {
            return;
        }

        let payload = event.serialize();
        let packet = Packet::new(event.event_type(), origin_server_id, 0, payload);
        let data = packet.serialize();

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.total_events += 1;
            stats.bytes_transferred += data.len() as u64;
        }

        // Send to all other servers
        for (&id, server) in servers.iter() {
            if id != origin_server_id {
                if let Err(e) = server.sender.send(data.clone()).await {
                    tracing::warn!("Failed to send to server {}: {}", id, e);
                }
            }
        }
    }

    /// Send an event to a specific server.
    pub async fn send_to_server(&self, server_id: u8, event: Event) -> Result<()> {
        let servers = self.servers.read().await;

        if let Some(server) = servers.get(&server_id) {
            let payload = event.serialize();
            let packet = Packet::new(event.event_type(), 0, 0, payload);
            let data = packet.serialize();

            server
                .sender
                .send(data)
                .await
                .map_err(|_| RelayError::InternalError {
                    message: format!("Failed to send to server {}", server_id),
                })?;

            Ok(())
        } else {
            Err(RelayError::InternalError {
                message: format!("Server {} not connected", server_id),
            })
        }
    }

    /// Route an event to the server that owns a specific player.
    pub async fn route_to_player(&self, steam_id: u64, event: Event) -> Result<()> {
        let player_map = self.player_map.read().await;

        if let Some(&server_id) = player_map.get(&steam_id) {
            self.send_to_server(server_id, event).await
        } else {
            Err(RelayError::InternalError {
                message: format!("Player {} not found", steam_id),
            })
        }
    }

    /// Register a player with a server.
    pub async fn register_player(&self, steam_id: u64, server_id: u8) {
        let mut player_map = self.player_map.write().await;
        player_map.insert(steam_id, server_id);
    }

    /// Unregister a player.
    pub async fn unregister_player(&self, steam_id: u64) {
        let mut player_map = self.player_map.write().await;
        player_map.remove(&steam_id);
    }

    /// Get the server ID for a player.
    pub async fn get_player_server(&self, steam_id: u64) -> Option<u8> {
        let player_map = self.player_map.read().await;
        player_map.get(&steam_id).copied()
    }

    /// Record an event for TUI display.
    pub async fn record_event(&self, server_id: u8, event: Event) {
        let recorded = RecordedEvent {
            server_id,
            event,
            timestamp: Instant::now(),
        };

        // Send to TUI channel
        let _ = self.event_tx.send(recorded.clone()).await;

        // Store in history
        let mut history = self.event_history.write().await;
        history.push(recorded);

        // Trim history if needed
        let max_history = self.config.tui.max_event_history;
        if history.len() > max_history {
            let drain_count = history.len() - max_history;
            history.drain(0..drain_count);
        }
    }

    /// Get event history.
    pub async fn get_event_history(&self) -> Vec<RecordedEvent> {
        let history = self.event_history.read().await;
        history.clone()
    }

    /// Clear event history.
    pub async fn clear_event_history(&self) {
        let mut history = self.event_history.write().await;
        history.clear();
    }

    /// Get relay statistics.
    pub async fn get_stats(&self) -> RelayStats {
        let stats = self.stats.read().await;
        stats.clone()
    }

    /// Get uptime in seconds.
    pub async fn uptime_secs(&self) -> u64 {
        let stats = self.stats.read().await;
        stats
            .start_time
            .map(|t| t.elapsed().as_secs())
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_relay_creation() {
        let config = Arc::new(Config::default());
        let relay = Relay::new(config);

        assert_eq!(relay.connected_count(), 0);
    }

    #[tokio::test]
    async fn test_player_registration() {
        let config = Arc::new(Config::default());
        let relay = Relay::new(config);

        relay.register_player(76561198000000000, 1).await;
        assert_eq!(relay.get_player_server(76561198000000000).await, Some(1));

        relay.unregister_player(76561198000000000).await;
        assert_eq!(relay.get_player_server(76561198000000000).await, None);
    }
}
