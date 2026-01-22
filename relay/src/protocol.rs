//! Binary protocol implementation for TF2 Server Relay.
//!
//! Packet structure (Little-Endian):
//! ```text
//! [MAGIC:2][VER:1][TYPE:1][LEN:2][SRV:1][SEQ:2][PAYLOAD:var][CRC:1]
//! ```
//!
//! - Magic: 0x5446 ('TF')
//! - Version: Protocol version (currently 1)
//! - Type: Packet type ID
//! - Length: Payload length in bytes
//! - Server ID: Origin server (1-4, 0 for relay)
//! - Sequence: Packet sequence number
//! - Payload: Variable-length data
//! - CRC: CRC-8 checksum

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::error::{RelayError, Result};

/// Protocol magic bytes ('TF' = 0x54, 0x46).
pub const MAGIC: u16 = 0x5446;

/// Current protocol version.
pub const PROTOCOL_VERSION: u8 = 1;

/// Fixed header size (magic:2 + version:1 + type:1 + length:2 + server_id:1 + sequence:2 = 9).
pub const HEADER_SIZE: usize = 9;

/// Checksum size.
pub const CHECKSUM_SIZE: usize = 1;

/// Maximum payload size.
pub const MAX_PAYLOAD_SIZE: usize = 4096;

/// Maximum packet size (header + max payload + checksum).
pub const MAX_PACKET_SIZE: usize = HEADER_SIZE + MAX_PAYLOAD_SIZE + CHECKSUM_SIZE;

/// Packet header structure.
#[derive(Debug, Clone, PartialEq)]
pub struct PacketHeader {
    /// Protocol version.
    pub version: u8,
    /// Packet type ID.
    pub packet_type: u8,
    /// Payload length in bytes.
    pub payload_length: u16,
    /// Origin server ID (1-4, 0 for relay).
    pub server_id: u8,
    /// Packet sequence number.
    pub sequence: u16,
}

impl PacketHeader {
    /// Create a new packet header.
    pub fn new(packet_type: u8, payload_length: u16, server_id: u8, sequence: u16) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            packet_type,
            payload_length,
            server_id,
            sequence,
        }
    }

    /// Parse a header from bytes.
    pub fn parse(data: &mut impl Buf) -> Result<Self> {
        if data.remaining() < HEADER_SIZE {
            return Err(RelayError::MalformedPacket {
                reason: format!(
                    "Header too short: {} bytes, need {}",
                    data.remaining(),
                    HEADER_SIZE
                ),
            });
        }

        // Read and validate magic bytes
        let magic = data.get_u16_le();
        if magic != MAGIC {
            return Err(RelayError::InvalidMagic { got: magic });
        }

        let version = data.get_u8();
        if version != PROTOCOL_VERSION {
            return Err(RelayError::UnsupportedVersion { version });
        }

        let packet_type = data.get_u8();
        let payload_length = data.get_u16_le();
        let server_id = data.get_u8();
        let sequence = data.get_u16_le();

        // Validate payload length
        if payload_length as usize > MAX_PAYLOAD_SIZE {
            return Err(RelayError::PayloadTooLarge {
                size: payload_length as usize,
                max: MAX_PAYLOAD_SIZE,
            });
        }

        Ok(Self {
            version,
            packet_type,
            payload_length,
            server_id,
            sequence,
        })
    }

    /// Serialize the header to bytes.
    pub fn serialize(&self, buf: &mut impl BufMut) {
        buf.put_u16_le(MAGIC);
        buf.put_u8(self.version);
        buf.put_u8(self.packet_type);
        buf.put_u16_le(self.payload_length);
        buf.put_u8(self.server_id);
        buf.put_u16_le(self.sequence);
    }
}

/// Complete packet structure.
#[derive(Debug, Clone)]
pub struct Packet {
    /// Packet header.
    pub header: PacketHeader,
    /// Packet payload.
    pub payload: Bytes,
}

impl Packet {
    /// Create a new packet.
    pub fn new(packet_type: u8, server_id: u8, sequence: u16, payload: Bytes) -> Self {
        let header = PacketHeader::new(packet_type, payload.len() as u16, server_id, sequence);
        Self { header, payload }
    }

    /// Parse a complete packet from bytes.
    pub fn parse(data: &mut impl Buf) -> Result<Self> {
        let header = PacketHeader::parse(data)?;

        let total_remaining = header.payload_length as usize + CHECKSUM_SIZE;
        if data.remaining() < total_remaining {
            return Err(RelayError::MalformedPacket {
                reason: format!(
                    "Incomplete packet: expected {} more bytes, have {}",
                    total_remaining,
                    data.remaining()
                ),
            });
        }

        // Copy payload
        let payload = data.copy_to_bytes(header.payload_length as usize);

        // Read and validate checksum
        let received_crc = data.get_u8();

        // Calculate expected CRC over header + payload
        let mut check_buf = BytesMut::with_capacity(HEADER_SIZE + payload.len());
        header.serialize(&mut check_buf);
        check_buf.extend_from_slice(&payload);
        let expected_crc = crc8(&check_buf);

        if received_crc != expected_crc {
            return Err(RelayError::ChecksumMismatch {
                expected: expected_crc,
                got: received_crc,
            });
        }

        Ok(Self { header, payload })
    }

    /// Serialize the complete packet to bytes.
    pub fn serialize(&self) -> Bytes {
        let total_size = HEADER_SIZE + self.payload.len() + CHECKSUM_SIZE;
        let mut buf = BytesMut::with_capacity(total_size);

        // Write header
        self.header.serialize(&mut buf);

        // Write payload
        buf.extend_from_slice(&self.payload);

        // Calculate and write CRC
        let crc = crc8(&buf);
        buf.put_u8(crc);

        buf.freeze()
    }

    /// Get the packet type.
    pub fn packet_type(&self) -> u8 {
        self.header.packet_type
    }

    /// Get the server ID.
    pub fn server_id(&self) -> u8 {
        self.header.server_id
    }

    /// Create a packet with a different server ID (for forwarding).
    pub fn with_server_id(&self, server_id: u8) -> Self {
        let mut new_header = self.header.clone();
        new_header.server_id = server_id;
        Self {
            header: new_header,
            payload: self.payload.clone(),
        }
    }
}

// ============================================================================
// String encoding utilities
// ============================================================================

/// Read a length-prefixed string from a buffer.
///
/// Format: [length:1][utf8_data:length]
pub fn read_string(buf: &mut impl Buf) -> Result<String> {
    if buf.remaining() < 1 {
        return Err(RelayError::MalformedPacket {
            reason: "Cannot read string length".to_string(),
        });
    }

    let length = buf.get_u8() as usize;

    if buf.remaining() < length {
        return Err(RelayError::MalformedPacket {
            reason: format!(
                "String length {} exceeds remaining buffer {}",
                length,
                buf.remaining()
            ),
        });
    }

    let data = buf.copy_to_bytes(length);
    String::from_utf8(data.to_vec()).map_err(|e| RelayError::Parse(e.to_string()))
}

/// Write a length-prefixed string to a buffer.
///
/// Format: [length:1][utf8_data:length]
pub fn write_string(buf: &mut impl BufMut, s: &str) {
    let bytes = s.as_bytes();
    let len = bytes.len().min(255) as u8;
    buf.put_u8(len);
    buf.put_slice(&bytes[..len as usize]);
}

/// Calculate the size of a length-prefixed string.
pub fn string_size(s: &str) -> usize {
    1 + s.len().min(255)
}

// ============================================================================
// CRC-8 checksum (polynomial 0x07)
// ============================================================================

/// CRC-8 lookup table for polynomial 0x07.
const CRC8_TABLE: [u8; 256] = [
    0x00, 0x07, 0x0E, 0x09, 0x1C, 0x1B, 0x12, 0x15, 0x38, 0x3F, 0x36, 0x31, 0x24, 0x23, 0x2A, 0x2D,
    0x70, 0x77, 0x7E, 0x79, 0x6C, 0x6B, 0x62, 0x65, 0x48, 0x4F, 0x46, 0x41, 0x54, 0x53, 0x5A, 0x5D,
    0xE0, 0xE7, 0xEE, 0xE9, 0xFC, 0xFB, 0xF2, 0xF5, 0xD8, 0xDF, 0xD6, 0xD1, 0xC4, 0xC3, 0xCA, 0xCD,
    0x90, 0x97, 0x9E, 0x99, 0x8C, 0x8B, 0x82, 0x85, 0xA8, 0xAF, 0xA6, 0xA1, 0xB4, 0xB3, 0xBA, 0xBD,
    0xC7, 0xC0, 0xC9, 0xCE, 0xDB, 0xDC, 0xD5, 0xD2, 0xFF, 0xF8, 0xF1, 0xF6, 0xE3, 0xE4, 0xED, 0xEA,
    0xB7, 0xB0, 0xB9, 0xBE, 0xAB, 0xAC, 0xA5, 0xA2, 0x8F, 0x88, 0x81, 0x86, 0x93, 0x94, 0x9D, 0x9A,
    0x27, 0x20, 0x29, 0x2E, 0x3B, 0x3C, 0x35, 0x32, 0x1F, 0x18, 0x11, 0x16, 0x03, 0x04, 0x0D, 0x0A,
    0x57, 0x50, 0x59, 0x5E, 0x4B, 0x4C, 0x45, 0x42, 0x6F, 0x68, 0x61, 0x66, 0x73, 0x74, 0x7D, 0x7A,
    0x89, 0x8E, 0x87, 0x80, 0x95, 0x92, 0x9B, 0x9C, 0xB1, 0xB6, 0xBF, 0xB8, 0xAD, 0xAA, 0xA3, 0xA4,
    0xF9, 0xFE, 0xF7, 0xF0, 0xE5, 0xE2, 0xEB, 0xEC, 0xC1, 0xC6, 0xCF, 0xC8, 0xDD, 0xDA, 0xD3, 0xD4,
    0x69, 0x6E, 0x67, 0x60, 0x75, 0x72, 0x7B, 0x7C, 0x51, 0x56, 0x5F, 0x58, 0x4D, 0x4A, 0x43, 0x44,
    0x19, 0x1E, 0x17, 0x10, 0x05, 0x02, 0x0B, 0x0C, 0x21, 0x26, 0x2F, 0x28, 0x3D, 0x3A, 0x33, 0x34,
    0x4E, 0x49, 0x40, 0x47, 0x52, 0x55, 0x5C, 0x5B, 0x76, 0x71, 0x78, 0x7F, 0x6A, 0x6D, 0x64, 0x63,
    0x3E, 0x39, 0x30, 0x37, 0x22, 0x25, 0x2C, 0x2B, 0x06, 0x01, 0x08, 0x0F, 0x1A, 0x1D, 0x14, 0x13,
    0xAE, 0xA9, 0xA0, 0xA7, 0xB2, 0xB5, 0xBC, 0xBB, 0x96, 0x91, 0x98, 0x9F, 0x8A, 0x8D, 0x84, 0x83,
    0xDE, 0xD9, 0xD0, 0xD7, 0xC2, 0xC5, 0xCC, 0xCB, 0xE6, 0xE1, 0xE8, 0xEF, 0xFA, 0xFD, 0xF4, 0xF3,
];

/// Calculate CRC-8 checksum for the given data.
pub fn crc8(data: &[u8]) -> u8 {
    let mut crc: u8 = 0;
    for byte in data {
        crc = CRC8_TABLE[(crc ^ byte) as usize];
    }
    crc
}

// ============================================================================
// Packet frame decoder for streaming reads
// ============================================================================

/// Framed packet decoder for reading packets from a stream.
pub struct PacketDecoder {
    buffer: BytesMut,
}

impl PacketDecoder {
    /// Create a new packet decoder.
    pub fn new() -> Self {
        Self {
            buffer: BytesMut::with_capacity(MAX_PACKET_SIZE),
        }
    }

    /// Add data to the internal buffer.
    pub fn push(&mut self, data: &[u8]) {
        self.buffer.extend_from_slice(data);
    }

    /// Try to decode a complete packet from the buffer.
    pub fn decode(&mut self) -> Result<Option<Packet>> {
        // Need at least header size
        if self.buffer.len() < HEADER_SIZE {
            return Ok(None);
        }

        // Peek at the header to get payload length
        let mut peek = &self.buffer[..];
        let header = match PacketHeader::parse(&mut peek) {
            Ok(h) => h,
            Err(e) => {
                // On invalid magic, try to find next valid magic
                if matches!(e, RelayError::InvalidMagic { .. }) {
                    self.buffer.advance(1);
                    return Ok(None);
                }
                return Err(e);
            }
        };

        let total_packet_size = HEADER_SIZE + header.payload_length as usize + CHECKSUM_SIZE;

        // Check if we have the complete packet
        if self.buffer.len() < total_packet_size {
            return Ok(None);
        }

        // Parse the complete packet
        let mut packet_data = self.buffer.split_to(total_packet_size);
        let packet = Packet::parse(&mut packet_data)?;

        Ok(Some(packet))
    }

    /// Get the current buffer length.
    pub fn buffer_len(&self) -> usize {
        self.buffer.len()
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}

impl Default for PacketDecoder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crc8() {
        assert_eq!(crc8(&[]), 0);
        assert_eq!(crc8(&[0x00]), 0x00);
        assert_eq!(crc8(&[0x01]), 0x07);
        assert_eq!(crc8(&[0x54, 0x46]), 0x6E);
    }

    #[test]
    fn test_header_roundtrip() {
        let header = PacketHeader::new(0x10, 15, 1, 42);
        let mut buf = BytesMut::new();
        header.serialize(&mut buf);

        let parsed = PacketHeader::parse(&mut buf.freeze()).unwrap();
        assert_eq!(parsed, header);
    }

    #[test]
    fn test_packet_roundtrip() {
        let payload = Bytes::from_static(b"Hello, TF2!");
        let packet = Packet::new(0x10, 1, 100, payload.clone());

        let serialized = packet.serialize();
        let mut buf = serialized;

        let parsed = Packet::parse(&mut buf).unwrap();
        assert_eq!(parsed.header.packet_type, 0x10);
        assert_eq!(parsed.header.server_id, 1);
        assert_eq!(parsed.header.sequence, 100);
        assert_eq!(parsed.payload, payload);
    }

    #[test]
    fn test_string_roundtrip() {
        let mut buf = BytesMut::new();
        let test_str = "Hello, World!";

        write_string(&mut buf, test_str);

        let mut read_buf = buf.freeze();
        let result = read_string(&mut read_buf).unwrap();

        assert_eq!(result, test_str);
    }

    #[test]
    fn test_invalid_magic() {
        let mut buf = BytesMut::new();
        buf.put_u16_le(0x1234); // Invalid magic
        buf.put_u8(1);
        buf.put_u8(0x10);
        buf.put_u16_le(0);
        buf.put_u8(1);
        buf.put_u16_le(0);

        let result = PacketHeader::parse(&mut buf.freeze());
        assert!(matches!(
            result,
            Err(RelayError::InvalidMagic { got: 0x1234 })
        ));
    }

    #[test]
    fn test_packet_decoder() {
        let payload = Bytes::from_static(b"Test");
        let packet = Packet::new(0x10, 1, 1, payload);
        let serialized = packet.serialize();

        let mut decoder = PacketDecoder::new();

        // Push data in chunks
        decoder.push(&serialized[..5]);
        assert!(decoder.decode().unwrap().is_none());

        decoder.push(&serialized[5..]);
        let decoded = decoder.decode().unwrap().unwrap();

        assert_eq!(decoded.header.packet_type, 0x10);
    }
}
