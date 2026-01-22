//! TF2 Server Relay - Main Entry Point
//!
//! High-performance relay server for cross-server TF2 communication.

mod config;
mod connection;
mod error;
mod events;
mod protocol;
mod relay;
mod server;
mod tui;

use std::sync::Arc;

use clap::Parser;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use config::Config;
use error::Result;
use relay::Relay;
use server::Server;

/// TF2 Server Relay - Cross-server communication for TF2 game servers.
#[derive(Parser, Debug)]
#[command(name = "tf2-server-relay")]
#[command(author = "TF2 Server Relay Team")]
#[command(version = "1.0.0")]
#[command(about = "High-performance relay server for cross-server TF2 communication")]
struct Args {
    /// Run in CLI mode (no TUI). Logs to stdout.
    #[arg(short = 'c', long = "cli")]
    cli_mode: bool,

    /// Path to configuration file.
    #[arg(short = 'C', long = "config", default_value = "settings.toml")]
    config_path: String,

    /// Override bind address (e.g., 0.0.0.0:27050).
    #[arg(short = 'b', long = "bind")]
    bind_address: Option<String>,

    /// Log level: trace, debug, info, warn, error.
    #[arg(short = 'l', long = "log-level", default_value = "info")]
    log_level: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Load configuration
    let mut config = Config::load(&args.config_path)?;

    // Override bind address if provided
    if let Some(ref bind) = args.bind_address {
        config.server.bind_address = bind.clone();
    }

    // Override log level
    config.logging.level = args.log_level.clone();

    // Initialize logging
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&config.logging.level));

    if args.cli_mode {
        // CLI mode: log to stdout
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer().with_target(false))
            .init();
    } else {
        // TUI mode: log to file only (if configured)
        if let Some(ref log_file) = config.logging.file {
            let file = std::fs::File::create(log_file)?;
            tracing_subscriber::registry()
                .with(filter)
                .with(fmt::layer().with_writer(file).with_ansi(false))
                .init();
        }
    }

    tracing::info!("TF2 Server Relay v1.0.0 starting...");
    tracing::info!("Configuration loaded from: {}", args.config_path);
    tracing::info!("Bind address: {}", config.server.bind_address);
    tracing::info!("Max servers: {}", config.server.max_servers);
    tracing::info!("Mode: {}", if args.cli_mode { "CLI" } else { "TUI" });

    // Create shared relay instance
    let config = Arc::new(config);
    let relay = Arc::new(Relay::new(config.clone()));

    // Create server
    let server = Server::new((*config).clone(), relay.clone()).await?;

    if args.cli_mode {
        // CLI mode: just run the server
        tracing::info!("Running in CLI mode. Press Ctrl+C to quit.");

        tokio::select! {
            result = server.run() => {
                if let Err(e) = result {
                    tracing::error!("Server error: {}", e);
                }
            }
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("Shutting down...");
                server.shutdown();
            }
        }
    } else {
        // TUI mode: run server and TUI concurrently
        let mut app = tui::App::new(config.clone(), relay.clone());

        // Spawn server task
        let server_task = tokio::spawn(async move {
            if let Err(e) = server.run().await {
                tracing::error!("Server error: {}", e);
            }
        });

        // Run TUI (blocking)
        if let Err(e) = app.run().await {
            tracing::error!("TUI error: {}", e);
        }

        // TUI exited, shut down server
        server_task.abort();
    }

    tracing::info!("TF2 Server Relay shutdown complete.");
    Ok(())
}
