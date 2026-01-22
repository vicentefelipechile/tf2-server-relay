//! Error types for the TF2 Server Relay.
//!
//! Error codes follow the BLUEPRINT.xml specification:
//! - 1000-1099: Connection errors
//! - 2000-2099: Protocol errors
//! - 3000-3099: Server errors

use thiserror::Error;

/// Main error type for the relay server.
#[derive(Error, Debug)]
pub enum RelayError {
    // ========================================
    // Connection Errors (1000-1099)
    // ========================================
    #[error("Connection refused (1000)")]
    ConnectionRefused,

    #[error("Server full - maximum {max} servers already connected (1001)")]
    ServerFull { max: u8 },

    #[error("Duplicate server ID {id} already in use (1002)")]
    DuplicateId { id: u8 },

    #[error("Handshake timeout - no handshake received in {timeout_ms}ms (1003)")]
    HandshakeTimeout { timeout_ms: u64 },

    #[error("Heartbeat timeout - no heartbeat received in {timeout_ms}ms (1004)")]
    HeartbeatTimeout { timeout_ms: u64 },

    // ========================================
    // Protocol Errors (2000-2099)
    // ========================================
    #[error("Invalid magic bytes: expected 0x5446, got 0x{got:04X} (2000)")]
    InvalidMagic { got: u16 },

    #[error("Unsupported protocol version {version} (2001)")]
    UnsupportedVersion { version: u8 },

    #[error("Unknown packet type 0x{packet_type:02X} (2002)")]
    UnknownPacketType { packet_type: u8 },

    #[error("Checksum mismatch: expected 0x{expected:02X}, got 0x{got:02X} (2003)")]
    ChecksumMismatch { expected: u8, got: u8 },

    #[error("Payload too large: {size} bytes exceeds maximum {max} (2004)")]
    PayloadTooLarge { size: usize, max: usize },

    #[error("Malformed packet: {reason} (2005)")]
    MalformedPacket { reason: String },

    // ========================================
    // Server Errors (3000-3099)
    // ========================================
    #[error("Relay server shutting down (3000)")]
    RelayShutdown,

    #[error("Internal relay error: {message} (3001)")]
    InternalError { message: String },

    // ========================================
    // IO and Config Errors
    // ========================================
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Parse error: {0}")]
    Parse(String),
}

impl RelayError {
    /// Get the numeric error code for this error.
    pub fn code(&self) -> u16 {
        match self {
            // Connection errors
            RelayError::ConnectionRefused => 1000,
            RelayError::ServerFull { .. } => 1001,
            RelayError::DuplicateId { .. } => 1002,
            RelayError::HandshakeTimeout { .. } => 1003,
            RelayError::HeartbeatTimeout { .. } => 1004,

            // Protocol errors
            RelayError::InvalidMagic { .. } => 2000,
            RelayError::UnsupportedVersion { .. } => 2001,
            RelayError::UnknownPacketType { .. } => 2002,
            RelayError::ChecksumMismatch { .. } => 2003,
            RelayError::PayloadTooLarge { .. } => 2004,
            RelayError::MalformedPacket { .. } => 2005,

            // Server errors
            RelayError::RelayShutdown => 3000,
            RelayError::InternalError { .. } => 3001,

            // Non-coded errors
            RelayError::Io(_) => 3001,
            RelayError::Config(_) => 3001,
            RelayError::Parse(_) => 2005,
        }
    }

    /// Check if this error is recoverable (connection can continue).
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            RelayError::ChecksumMismatch { .. }
                | RelayError::UnknownPacketType { .. }
                | RelayError::MalformedPacket { .. }
        )
    }
}

/// Result type alias using RelayError.
pub type Result<T> = std::result::Result<T, RelayError>;
