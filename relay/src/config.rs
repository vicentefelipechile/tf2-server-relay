//! Configuration handling for the TF2 Server Relay.
//!
//! Configuration is loaded from `settings.toml`. Settings are classified as:
//! - STATIC: Can only be changed at startup (requires restart)
//! - DYNAMIC: Can be changed at runtime via TUI

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::error::{RelayError, Result};

/// Main configuration structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Core relay server settings.
    pub server: ServerConfig,

    /// Cross-server synchronization settings.
    pub sync: SyncConfig,

    /// Logging configuration.
    pub logging: LoggingConfig,

    /// Terminal UI settings.
    pub tui: TuiConfig,

    /// Event relay behavior settings.
    pub relay: RelayConfig,
}

/// Server configuration section.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    /// IP and port to bind the relay server. [STATIC]
    pub bind_address: String,

    /// Maximum number of TF2 servers (1-4). [STATIC]
    pub max_servers: u8,

    /// Timeout before considering a connection dead (ms). [DYNAMIC]
    pub connection_timeout_ms: u64,

    /// Interval between heartbeat packets (ms). [DYNAMIC]
    pub heartbeat_interval_ms: u64,
}

/// Synchronization settings for cross-server features.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SyncConfig {
    /// How often to sync ghost positions per second. [DYNAMIC]
    pub position_sync_rate_hz: u8,

    /// Enable optimistic healing for lower perceived latency. [DYNAMIC]
    pub predictive_healing: bool,

    /// Smooth ghost movement between position updates. [DYNAMIC]
    pub ghost_interpolation: bool,

    /// Maximum acceptable latency for ghost updates (ms). [DYNAMIC]
    pub max_ghost_latency_ms: u64,
}

/// Logging configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LoggingConfig {
    /// Log verbosity: trace, debug, info, warn, error. [DYNAMIC]
    pub level: String,

    /// Optional file path to write logs. [STATIC]
    pub file: Option<String>,

    /// Log all relayed events. [DYNAMIC]
    pub log_events: bool,

    /// Log raw packet data (debug only, high volume). [DYNAMIC]
    pub log_packets: bool,
}

/// TUI configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TuiConfig {
    /// UI refresh interval in milliseconds. [DYNAMIC]
    pub refresh_rate_ms: u64,

    /// Maximum events to keep in history. [DYNAMIC]
    pub max_event_history: usize,

    /// TUI color theme. [DYNAMIC]
    pub color_scheme: String,

    /// Show timestamps in event feed. [DYNAMIC]
    pub show_timestamps: bool,

    /// Use compact layout for smaller terminals. [DYNAMIC]
    pub compact_mode: bool,
}

/// Relay behavior configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RelayConfig {
    /// Relay chat messages between servers. [DYNAMIC]
    pub broadcast_chat: bool,

    /// Relay death events between servers. [DYNAMIC]
    pub broadcast_deaths: bool,

    /// Relay player connect/disconnect events. [DYNAMIC]
    pub broadcast_connect: bool,

    /// Allow Medics to heal players on other servers. [DYNAMIC]
    pub enable_cross_healing: bool,

    /// Allow damage between players on different servers. [DYNAMIC]
    pub enable_cross_damage: bool,
}

// ============================================================================
// Default implementations
// ============================================================================

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            sync: SyncConfig::default(),
            logging: LoggingConfig::default(),
            tui: TuiConfig::default(),
            relay: RelayConfig::default(),
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_address: "0.0.0.0:27050".to_string(),
            max_servers: 4,
            connection_timeout_ms: 5000,
            heartbeat_interval_ms: 1000,
        }
    }
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            position_sync_rate_hz: 20,
            predictive_healing: true,
            ghost_interpolation: true,
            max_ghost_latency_ms: 100,
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            file: None,
            log_events: true,
            log_packets: false,
        }
    }
}

impl Default for TuiConfig {
    fn default() -> Self {
        Self {
            refresh_rate_ms: 100,
            max_event_history: 1000,
            color_scheme: "default".to_string(),
            show_timestamps: true,
            compact_mode: false,
        }
    }
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            broadcast_chat: true,
            broadcast_deaths: true,
            broadcast_connect: true,
            enable_cross_healing: true,
            enable_cross_damage: true,
        }
    }
}

// ============================================================================
// Config loading and saving
// ============================================================================

impl Config {
    /// Load configuration from a TOML file.
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();

        if !path.exists() {
            tracing::warn!("Config file not found at {:?}, using defaults", path);
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(path)?;
        let config: Config =
            toml::from_str(&content).map_err(|e| RelayError::Config(e.to_string()))?;

        config.validate()?;

        tracing::info!("Loaded configuration from {:?}", path);
        Ok(config)
    }

    /// Save configuration to a TOML file.
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content =
            toml::to_string_pretty(self).map_err(|e| RelayError::Config(e.to_string()))?;

        std::fs::write(path, content)?;
        Ok(())
    }

    /// Validate configuration values.
    pub fn validate(&self) -> Result<()> {
        // Validate max_servers
        if self.server.max_servers == 0 || self.server.max_servers > 4 {
            return Err(RelayError::Config(format!(
                "max_servers must be between 1 and 4, got {}",
                self.server.max_servers
            )));
        }

        // Validate connection_timeout_ms
        if self.server.connection_timeout_ms < 1000 || self.server.connection_timeout_ms > 30000 {
            return Err(RelayError::Config(format!(
                "connection_timeout_ms must be between 1000 and 30000, got {}",
                self.server.connection_timeout_ms
            )));
        }

        // Validate heartbeat_interval_ms
        if self.server.heartbeat_interval_ms < 100 || self.server.heartbeat_interval_ms > 5000 {
            return Err(RelayError::Config(format!(
                "heartbeat_interval_ms must be between 100 and 5000, got {}",
                self.server.heartbeat_interval_ms
            )));
        }

        // Validate position_sync_rate_hz
        if self.sync.position_sync_rate_hz < 5 || self.sync.position_sync_rate_hz > 60 {
            return Err(RelayError::Config(format!(
                "position_sync_rate_hz must be between 5 and 60, got {}",
                self.sync.position_sync_rate_hz
            )));
        }

        // Validate log level
        let valid_levels = ["trace", "debug", "info", "warn", "error"];
        if !valid_levels.contains(&self.logging.level.as_str()) {
            return Err(RelayError::Config(format!(
                "Invalid log level '{}', must be one of: {:?}",
                self.logging.level, valid_levels
            )));
        }

        // Validate color scheme
        let valid_schemes = ["default", "dark", "light", "high_contrast"];
        if !valid_schemes.contains(&self.tui.color_scheme.as_str()) {
            return Err(RelayError::Config(format!(
                "Invalid color_scheme '{}', must be one of: {:?}",
                self.tui.color_scheme, valid_schemes
            )));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.server.bind_address, "0.0.0.0:27050");
        assert_eq!(config.server.max_servers, 4);
        assert!(config.relay.broadcast_chat);
    }

    #[test]
    fn test_config_validation() {
        let mut config = Config::default();
        assert!(config.validate().is_ok());

        config.server.max_servers = 5;
        assert!(config.validate().is_err());
    }
}
