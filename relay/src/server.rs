//! TCP server listener for the TF2 Server Relay.
//!
//! Accepts connections from TF2 game servers and spawns connection handlers.

use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::broadcast;

use crate::config::Config;
use crate::connection::Connection;
use crate::error::Result;
use crate::relay::Relay;

/// TCP server that accepts connections from TF2 game servers.
pub struct Server {
    /// Server configuration.
    config: Arc<Config>,
    /// TCP listener.
    listener: TcpListener,
    /// Shared relay instance.
    relay: Arc<Relay>,
    /// Shutdown signal sender.
    shutdown_tx: broadcast::Sender<()>,
}

impl Server {
    /// Create a new server instance.
    pub async fn new(config: Config, relay: Arc<Relay>) -> Result<Self> {
        let listener = TcpListener::bind(&config.server.bind_address).await?;

        tracing::info!("Server listening on {}", config.server.bind_address);

        // Create shutdown channel
        let (shutdown_tx, _) = broadcast::channel(1);

        Ok(Self {
            config: Arc::new(config),
            listener,
            relay,
            shutdown_tx,
        })
    }

    /// Get a shutdown signal receiver.
    pub fn shutdown_signal(&self) -> broadcast::Receiver<()> {
        self.shutdown_tx.subscribe()
    }

    /// Send shutdown signal to all connections.
    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.send(());
    }

    /// Run the server, accepting connections until shutdown.
    pub async fn run(&self) -> Result<()> {
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        loop {
            tokio::select! {
                // Accept new connection
                result = self.listener.accept() => {
                    match result {
                        Ok((stream, addr)) => {
                            tracing::info!("New connection from {}", addr);

                            // Check if we have room for another server
                            let connected_count = self.relay.connected_count();
                            if connected_count >= self.config.server.max_servers as usize {
                                tracing::warn!(
                                    "Rejecting connection from {} - server full ({}/{})",
                                    addr,
                                    connected_count,
                                    self.config.server.max_servers
                                );
                                // Connection will be dropped, closing the socket
                                continue;
                            }

                            // Create and spawn connection handler
                            let connection = Connection::new(
                                stream,
                                addr,
                                self.config.clone(),
                                self.relay.clone(),
                                self.shutdown_tx.subscribe(),
                            );

                            tokio::spawn(async move {
                                if let Err(e) = connection.run().await {
                                    tracing::error!("Connection {} error: {}", addr, e);
                                }
                            });
                        }
                        Err(e) => {
                            tracing::error!("Failed to accept connection: {}", e);
                        }
                    }
                }

                // Shutdown signal received
                _ = shutdown_rx.recv() => {
                    tracing::info!("Server shutdown signal received");
                    break;
                }
            }
        }

        Ok(())
    }

    /// Get the bound address.
    pub fn local_addr(&self) -> std::io::Result<std::net::SocketAddr> {
        self.listener.local_addr()
    }
}
