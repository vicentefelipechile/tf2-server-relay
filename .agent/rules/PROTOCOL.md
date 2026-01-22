# Binary Protocol Specification

> TF2RelayProtocol v1 - Raw TCP binary packet format.

## Design Principles

- **Minimal overhead**: No JSON, no HTTP, raw binary
- **Little-endian**: All multi-byte integers
- **Length-prefixed strings**: 1-byte length + UTF-8 data (max 255 bytes)
- **CRC-8 checksum**: Data integrity validation

## Packet Structure

```
┌────────────────────────────────────────────────────────────────┐
│                          HEADER (9 bytes)                      │
├────────┬────────┬────────┬────────┬─────────┬────────┬─────────┤
│ MAGIC  │ MAGIC  │VERSION │  TYPE  │PAYLOAD  │PAYLOAD │ SERVER  │
│  'T'   │  'F'   │  (1)   │        │LEN LOW  │LEN HIGH│   ID    │
│ 0x54   │ 0x46   │ 0x01   │  0xXX  │  0xXX   │  0xXX  │  0xXX   │
├────────┴────────┴────────┴────────┴─────────┴────────┴─────────┤
│ SEQUENCE LOW │ SEQUENCE HIGH │                                 │
│     0xXX     │     0xXX      │                                 │
├──────────────┴───────────────┴─────────────────────────────────┤
│                     PAYLOAD (variable length)                  │
│                          0 to 4086 bytes                       │
├────────────────────────────────────────────────────────────────┤
│                        CHECKSUM (1 byte)                       │
│                          CRC-8 of all                          │
└────────────────────────────────────────────────────────────────┘
```

## Header Fields

| Offset | Size | Field | Description |
|--------|------|-------|-------------|
| 0 | 2 | magic | 0x5446 ("TF" in ASCII) |
| 2 | 1 | version | Protocol version (currently 1) |
| 3 | 1 | packet_type | Event type ID (see Events) |
| 4 | 2 | payload_length | Payload size in bytes |
| 6 | 1 | server_id | Origin server (1-4), 0 = relay |
| 7 | 2 | sequence | Packet sequence number |

**Total header size**: 9 bytes
**Max packet size**: 4096 bytes (header + payload + checksum)
**Max payload size**: 4086 bytes

## Data Types

| Type | Size | Description |
|------|------|-------------|
| `u8` | 1 | Unsigned 8-bit integer |
| `u16` | 2 | Unsigned 16-bit integer (little-endian) |
| `u32` | 4 | Unsigned 32-bit integer (little-endian) |
| `u64` | 8 | Unsigned 64-bit integer (little-endian) |
| `i32` | 4 | Signed 32-bit integer (little-endian) |
| `f32` | 4 | 32-bit float (IEEE 754, little-endian) |
| `string` | 1+N | Length byte + UTF-8 data |
| `bool` | 1 | 0 = false, 1 = true |

## String Encoding

```
┌────────┬────────────────────────────────┐
│ LENGTH │            DATA                │
│  (u8)  │         (UTF-8 bytes)          │
└────────┴────────────────────────────────┘
```

Example: "Hello" → `0x05 0x48 0x65 0x6C 0x6C 0x6F`

## Packet Type Ranges

| Range | Category | Description |
|-------|----------|-------------|
| 0x00-0x0F | System | Handshake, heartbeat, errors |
| 0x10-0x1F | Chat | Text messages, admin broadcasts |
| 0x20-0x2F | Player | Death, connect, team change |
| 0x30-0x3F | Gameplay | Heal, damage, buildings, status |
| 0x40-0x5F | Game | Round, map, mode |
| 0x60-0x6F | Custom | User-defined events |
| 0x70-0x7F | Ghost | Player sync, position, cross-server |

## System Packets (0x00-0x0F)

### HANDSHAKE (0x00)
```
Direction: Server → Relay
┌──────────┬─────────────┬──────────┬─────────────┬────────────────┐
│server_id │ server_name │ map_name │ max_players │ current_players│
│   u8     │   string    │  string  │     u8      │       u8       │
└──────────┴─────────────┴──────────┴─────────────┴────────────────┘
```

### HANDSHAKE_ACK (0x01)
```
Direction: Relay → Server
┌─────────┬─────────────┬───────────────────┐
│ success │ assigned_id │ connected_servers │
│   u8    │     u8      │        u8         │
└─────────┴─────────────┴───────────────────┘
```

### HEARTBEAT (0x02)
```
Direction: Server → Relay
┌───────────┬────────────────┐
│ timestamp │ current_players│
│    u32    │       u8       │
└───────────┴────────────────┘
```

### HEARTBEAT_ACK (0x03)
```
Direction: Relay → Server
┌───────────┬───────────────────┐
│ timestamp │ connected_servers │
│    u32    │        u8         │
└───────────┴───────────────────┘
```

### ERROR (0x06)
```
Direction: Bidirectional
┌────────────┬───────────────┐
│ error_code │ error_message │
│    u16     │    string     │
└────────────┴───────────────┘
```

## CRC-8 Calculation

```rust
fn crc8(data: &[u8]) -> u8 {
    let mut crc: u8 = 0x00;
    for byte in data {
        crc ^= byte;
        for _ in 0..8 {
            if crc & 0x80 != 0 {
                crc = (crc << 1) ^ 0x07;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}
```

## Parsing Example (Rust)

```rust
use bytes::{Buf, BytesMut};

struct PacketHeader {
    magic: u16,
    version: u8,
    packet_type: u8,
    payload_length: u16,
    server_id: u8,
    sequence: u16,
}

fn parse_header(buf: &mut BytesMut) -> Option<PacketHeader> {
    if buf.len() < 9 {
        return None;
    }
    
    let magic = buf.get_u16_le();
    if magic != 0x5446 {
        return None; // Invalid magic
    }
    
    Some(PacketHeader {
        magic,
        version: buf.get_u8(),
        packet_type: buf.get_u8(),
        payload_length: buf.get_u16_le(),
        server_id: buf.get_u8(),
        sequence: buf.get_u16_le(),
    })
}

fn read_string(buf: &mut BytesMut) -> String {
    let len = buf.get_u8() as usize;
    let bytes = buf.split_to(len);
    String::from_utf8_lossy(&bytes).to_string()
}
```

## Error Codes

| Code | Name | Description |
|------|------|-------------|
| 1000 | CONNECTION_REFUSED | Relay refused connection |
| 1001 | SERVER_FULL | Max servers connected |
| 1002 | DUPLICATE_ID | Server ID already in use |
| 1003 | HANDSHAKE_TIMEOUT | Handshake timed out |
| 1004 | HEARTBEAT_TIMEOUT | No heartbeat received |
| 2000 | INVALID_MAGIC | Invalid magic bytes |
| 2001 | UNSUPPORTED_VERSION | Unknown protocol version |
| 2002 | UNKNOWN_PACKET_TYPE | Invalid packet type |
| 2003 | CHECKSUM_MISMATCH | CRC validation failed |
| 2004 | PAYLOAD_TOO_LARGE | Payload exceeds max |
| 2005 | MALFORMED_PACKET | Invalid packet structure |
