//! Per-connection handler for TF2 game servers.
//!
//! Manages the connection lifecycle: handshake, heartbeat, packet processing.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{broadcast, mpsc};
use tokio::time;

use crate::config::Config;
use crate::error::{RelayError, Result};
use crate::events::{system, Event, HandshakeAckEvent, HeartbeatAckEvent};
use crate::protocol::{Packet, PacketDecoder};
use crate::relay::Relay;

/// Connection state machine states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Waiting for handshake.
    Pending,
    /// Handshake received, processing.
    Handshaking,
    /// Fully connected and operational.
    Connected,
    /// Connection terminated.
    Disconnected,
}

/// Information about a connected server.
#[derive(Debug, Clone)]
pub struct ServerInfo {
    /// Server ID (1-4).
    pub server_id: u8,
    /// Human-readable server name.
    pub server_name: String,
    /// Current map name.
    pub map_name: String,
    /// Maximum player slots.
    pub max_players: u8,
    /// Current player count.
    pub current_players: u8,
    /// Connection time.
    pub connected_at: Instant,
    /// Last heartbeat time.
    pub last_heartbeat: Instant,
}

/// Per-connection handler.
pub struct Connection {
    /// TCP stream.
    stream: TcpStream,
    /// Remote address.
    addr: SocketAddr,
    /// Configuration.
    config: Arc<Config>,
    /// Relay instance.
    relay: Arc<Relay>,
    /// Shutdown signal receiver.
    shutdown_rx: broadcast::Receiver<()>,
    /// Connection state.
    state: ConnectionState,
    /// Server info (set after handshake).
    server_info: Option<ServerInfo>,
    /// Packet sequence number.
    sequence: u16,
    /// Packet decoder.
    decoder: PacketDecoder,
    /// Outbound packet channel receiver.
    outbound_rx: Option<mpsc::Receiver<Bytes>>,
    /// Outbound packet channel sender (stored for relay).
    outbound_tx: mpsc::Sender<Bytes>,
}

impl Connection {
    /// Create a new connection handler.
    pub fn new(
        stream: TcpStream,
        addr: SocketAddr,
        config: Arc<Config>,
        relay: Arc<Relay>,
        shutdown_rx: broadcast::Receiver<()>,
    ) -> Self {
        // Create outbound channel
        let (outbound_tx, outbound_rx) = mpsc::channel(256);

        Self {
            stream,
            addr,
            config,
            relay,
            shutdown_rx,
            state: ConnectionState::Pending,
            server_info: None,
            sequence: 0,
            decoder: PacketDecoder::new(),
            outbound_rx: Some(outbound_rx),
            outbound_tx,
        }
    }

    /// Run the connection handler.
    pub async fn run(mut self) -> Result<()> {
        // Set up timeouts
        let handshake_timeout = Duration::from_millis(self.config.server.connection_timeout_ms);
        let heartbeat_interval = Duration::from_millis(self.config.server.heartbeat_interval_ms);
        let heartbeat_timeout = Duration::from_millis(self.config.server.connection_timeout_ms);

        // Wait for handshake with timeout
        let handshake_result = time::timeout(handshake_timeout, self.wait_for_handshake()).await;

        match handshake_result {
            Ok(Ok(())) => {
                tracing::info!(
                    "Handshake completed for server {} '{}'",
                    self.server_info.as_ref().unwrap().server_id,
                    self.server_info.as_ref().unwrap().server_name
                );
            }
            Ok(Err(e)) => {
                tracing::error!("Handshake failed for {}: {}", self.addr, e);
                return Err(e);
            }
            Err(_) => {
                tracing::error!("Handshake timeout for {}", self.addr);
                return Err(RelayError::HandshakeTimeout {
                    timeout_ms: self.config.server.connection_timeout_ms,
                });
            }
        }

        // Take outbound receiver
        let mut outbound_rx = self.outbound_rx.take().unwrap();

        // Register with relay
        let server_id = self.server_info.as_ref().unwrap().server_id;
        self.relay
            .register_server(
                server_id,
                self.server_info.clone().unwrap(),
                self.outbound_tx.clone(),
            )
            .await?;

        // Take decoder out of self for use in loop
        let mut decoder = std::mem::take(&mut self.decoder);

        // Read buffer
        let mut read_buf = vec![0u8; 4096];

        // Heartbeat timer
        let mut heartbeat_check = time::interval(heartbeat_interval);
        let mut last_heartbeat = Instant::now();

        self.state = ConnectionState::Connected;

        // Clone references we need inside the loop
        let relay = self.relay.clone();
        let config = self.config.clone();

        loop {
            tokio::select! {
                // Read from socket
                result = self.stream.read(&mut read_buf) => {
                    match result {
                        Ok(0) => {
                            tracing::info!("Connection closed by server {}", server_id);
                            break;
                        }
                        Ok(n) => {
                            decoder.push(&read_buf[..n]);
                        }
                        Err(e) => {
                            tracing::error!("Read error for server {}: {}", server_id, e);
                            return Err(e.into());
                        }
                    }
                }

                // Write outbound packets
                Some(data) = outbound_rx.recv() => {
                    if let Err(e) = self.stream.write_all(&data).await {
                        tracing::error!("Write error for server {}: {}", server_id, e);
                        return Err(e.into());
                    }
                }

                // Heartbeat check
                _ = heartbeat_check.tick() => {
                    if last_heartbeat.elapsed() > heartbeat_timeout {
                        tracing::warn!("Heartbeat timeout for server {}", server_id);
                        return Err(RelayError::HeartbeatTimeout {
                            timeout_ms: config.server.connection_timeout_ms,
                        });
                    }
                }

                // Shutdown signal
                _ = self.shutdown_rx.recv() => {
                    tracing::info!("Shutdown signal received for server {}", server_id);
                    break;
                }
            }

            // Process all complete packets
            while let Some(packet) = decoder.decode()? {
                if let Err(e) = Connection::process_packet(
                    &relay,
                    &config,
                    &mut self.server_info,
                    &mut self.stream,
                    &mut self.sequence,
                    packet,
                    &mut last_heartbeat,
                )
                .await
                {
                    if e.is_recoverable() {
                        tracing::warn!("Recoverable packet error: {}", e);
                    } else {
                        return Err(e);
                    }
                }
            }
        }

        // Unregister from relay
        relay.unregister_server(server_id).await;
        self.state = ConnectionState::Disconnected;

        Ok(())
    }

    /// Wait for and process handshake.
    async fn wait_for_handshake(&mut self) -> Result<()> {
        let mut read_buf = vec![0u8; 1024];

        loop {
            let n = self.stream.read(&mut read_buf).await?;
            if n == 0 {
                return Err(RelayError::MalformedPacket {
                    reason: "Connection closed before handshake".to_string(),
                });
            }

            self.decoder.push(&read_buf[..n]);

            while let Some(packet) = self.decoder.decode()? {
                if packet.packet_type() == system::HANDSHAKE {
                    return self.process_handshake(packet).await;
                } else {
                    tracing::warn!(
                        "Unexpected packet type 0x{:02X} before handshake",
                        packet.packet_type()
                    );
                }
            }
        }
    }

    /// Process handshake packet.
    async fn process_handshake(&mut self, packet: Packet) -> Result<()> {
        self.state = ConnectionState::Handshaking;

        let event = Event::parse(packet.packet_type(), packet.payload)?;

        if let Event::Handshake(handshake) = event {
            // Validate server ID
            if handshake.server_id == 0 || handshake.server_id > self.config.server.max_servers {
                let ack = HandshakeAckEvent {
                    success: false,
                    assigned_id: 0,
                    connected_servers: self.relay.connected_count() as u8,
                };
                self.send_event(Event::HandshakeAck(ack)).await?;

                return Err(RelayError::MalformedPacket {
                    reason: format!("Invalid server ID {}", handshake.server_id),
                });
            }

            // Check for duplicate ID
            if self.relay.is_server_connected(handshake.server_id) {
                let ack = HandshakeAckEvent {
                    success: false,
                    assigned_id: 0,
                    connected_servers: self.relay.connected_count() as u8,
                };
                self.send_event(Event::HandshakeAck(ack)).await?;

                return Err(RelayError::DuplicateId {
                    id: handshake.server_id,
                });
            }

            // Store server info
            self.server_info = Some(ServerInfo {
                server_id: handshake.server_id,
                server_name: handshake.server_name.clone(),
                map_name: handshake.map_name.clone(),
                max_players: handshake.max_players,
                current_players: handshake.current_players,
                connected_at: Instant::now(),
                last_heartbeat: Instant::now(),
            });

            // Send success ACK
            let ack = HandshakeAckEvent {
                success: true,
                assigned_id: handshake.server_id,
                connected_servers: self.relay.connected_count() as u8 + 1,
            };
            self.send_event(Event::HandshakeAck(ack)).await?;

            self.state = ConnectionState::Connected;
            Ok(())
        } else {
            Err(RelayError::MalformedPacket {
                reason: "Expected handshake packet".to_string(),
            })
        }
    }

    /// Process an incoming packet (static version to avoid borrow conflicts).
    async fn process_packet(
        relay: &Arc<Relay>,
        config: &Arc<Config>,
        server_info: &mut Option<ServerInfo>,
        stream: &mut TcpStream,
        sequence: &mut u16,
        packet: Packet,
        last_heartbeat: &mut Instant,
    ) -> Result<()> {
        let event = Event::parse(packet.packet_type(), packet.payload.clone())?;

        match &event {
            Event::Heartbeat(hb) => {
                *last_heartbeat = Instant::now();

                // Update player count
                if let Some(ref mut info) = server_info {
                    info.current_players = hb.current_players;
                    info.last_heartbeat = Instant::now();
                }

                // Send heartbeat ACK
                let ack = HeartbeatAckEvent {
                    timestamp: hb.timestamp,
                    connected_servers: relay.connected_count() as u8,
                };
                Self::send_event_static(stream, sequence, Event::HeartbeatAck(ack)).await?;
            }
            Event::MapChange(mc) => {
                // Update map name
                if let Some(ref mut info) = server_info {
                    info.map_name = mc.new_map.clone();
                }

                // Broadcast to other servers
                if event.is_broadcastable() {
                    relay.broadcast(packet.server_id(), event.clone()).await;
                }
            }
            Event::PlayerConnect(pc) => {
                // Register player with this server for targeted routing
                relay.register_player(pc.steam_id, packet.server_id()).await;
                tracing::debug!(
                    "Registered player {} '{}' with server {}",
                    pc.steam_id,
                    pc.player_name,
                    packet.server_id()
                );

                // Broadcast to other servers
                if event.is_broadcastable() {
                    relay.broadcast(packet.server_id(), event.clone()).await;
                }
            }
            Event::PlayerDisconnect(pd) => {
                // Unregister player
                relay.unregister_player(pd.steam_id).await;
                tracing::debug!(
                    "Unregistered player {} '{}' from server {}",
                    pd.steam_id,
                    pd.player_name,
                    packet.server_id()
                );

                // Broadcast to other servers
                if event.is_broadcastable() {
                    relay.broadcast(packet.server_id(), event.clone()).await;
                }
            }
            Event::HealRequest(hr) => {
                // Route heal request to the server that owns the target player
                if config.relay.enable_cross_healing {
                    if let Some(target_server) = relay.get_player_server(hr.target_steam_id).await {
                        if target_server != packet.server_id() {
                            // Target is on a different server - route directly
                            if let Err(e) = relay.send_to_server(target_server, event.clone()).await
                            {
                                tracing::warn!(
                                    "Failed to route HealRequest to server {}: {}",
                                    target_server,
                                    e
                                );
                            } else {
                                tracing::debug!(
                                    "Routed HealRequest from server {} to server {} (target: {})",
                                    packet.server_id(),
                                    target_server,
                                    hr.target_steam_id
                                );
                            }
                        }
                    } else {
                        // Target not found - broadcast to all servers
                        tracing::debug!(
                            "HealRequest target {} not found, broadcasting",
                            hr.target_steam_id
                        );
                        relay.broadcast(packet.server_id(), event.clone()).await;
                    }
                }
            }
            Event::HealConfirm(_hc) => {
                // Route heal confirm back to the healer's server
                if config.relay.enable_cross_healing {
                    // Broadcast to all - the healer will recognize the target
                    relay.broadcast(packet.server_id(), event.clone()).await;
                }
            }
            Event::DamageRequest(dr) => {
                // Route damage request to the server that owns the victim
                if config.relay.enable_cross_damage {
                    if let Some(victim_server) = relay.get_player_server(dr.victim_steam_id).await {
                        if victim_server != packet.server_id() {
                            // Victim is on a different server - route directly
                            if let Err(e) = relay.send_to_server(victim_server, event.clone()).await
                            {
                                tracing::warn!(
                                    "Failed to route DamageRequest to server {}: {}",
                                    victim_server,
                                    e
                                );
                            } else {
                                tracing::debug!(
                                    "Routed DamageRequest from server {} to server {} (victim: {})",
                                    packet.server_id(),
                                    victim_server,
                                    dr.victim_steam_id
                                );
                            }
                        }
                    } else {
                        // Victim not found - broadcast to all servers
                        tracing::debug!(
                            "DamageRequest victim {} not found, broadcasting",
                            dr.victim_steam_id
                        );
                        relay.broadcast(packet.server_id(), event.clone()).await;
                    }
                }
            }
            Event::DamageConfirm(_dc) => {
                // Route damage confirm back to the attacker's server
                if config.relay.enable_cross_damage {
                    // Broadcast to all - the attacker will recognize the victim
                    relay.broadcast(packet.server_id(), event.clone()).await;
                }
            }
            _ => {
                // Log the event
                if config.logging.log_events {
                    tracing::debug!("[Server {}] {}", packet.server_id(), event.description());
                }

                // Broadcast to other servers if applicable
                if event.is_broadcastable() {
                    relay.broadcast(packet.server_id(), event.clone()).await;
                }
            }
        }

        // Notify relay of event for TUI
        relay
            .record_event(
                server_info.as_ref().map(|i| i.server_id).unwrap_or(0),
                event,
            )
            .await;

        Ok(())
    }

    /// Send an event (static version).
    async fn send_event_static(
        stream: &mut TcpStream,
        sequence: &mut u16,
        event: Event,
    ) -> Result<()> {
        let payload = event.serialize();
        let seq = *sequence;
        *sequence = sequence.wrapping_add(1);
        let packet = Packet::new(event.event_type(), 0, seq, payload);
        let data = packet.serialize();

        stream.write_all(&data).await?;
        Ok(())
    }

    /// Send an event to this connection.
    async fn send_event(&mut self, event: Event) -> Result<()> {
        let payload = event.serialize();
        let packet = Packet::new(event.event_type(), 0, self.next_sequence(), payload);
        let data = packet.serialize();

        self.stream.write_all(&data).await?;
        Ok(())
    }

    /// Get the next sequence number.
    fn next_sequence(&mut self) -> u16 {
        let seq = self.sequence;
        self.sequence = self.sequence.wrapping_add(1);
        seq
    }
}
