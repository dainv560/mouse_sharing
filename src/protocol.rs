use std::io::{Read, Write};

pub const DEFAULT_PORT: u16 = 8421;
pub const PACKET_SIZE: usize = 16;
pub const MAX_CLIPBOARD_TEXT_LEN: usize = 10 * 1024 * 1024; // 10 MB
pub const MAX_CLIPBOARD_FILE_LEN: usize = 500 * 1024 * 1024; // 500 MB

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Packet {
    Handshake { width: i32, height: i32 },
    HandshakeAck { width: i32, height: i32 },
    MouseMoveAbs { x: i32, y: i32 },
    MouseButton { button: u8, is_down: bool },
    MouseWheel { delta: i16 },
    Key { vk: u16, scan: u16, flags: u32, is_down: bool },
    Heartbeat,
    ReleaseControl,
    ClipboardText { text: String },
    ClipboardFile { filename: String, data: Vec<u8> },
}

impl Packet {
    pub fn to_bytes_fixed(&self) -> Option<[u8; PACKET_SIZE]> {
        let mut buf = [0u8; PACKET_SIZE];
        match self {
            Packet::Handshake { width, height } => {
                buf[0] = 1;
                buf[1..5].copy_from_slice(&width.to_le_bytes());
                buf[5..9].copy_from_slice(&height.to_le_bytes());
                Some(buf)
            }
            Packet::HandshakeAck { width, height } => {
                buf[0] = 2;
                buf[1..5].copy_from_slice(&width.to_le_bytes());
                buf[5..9].copy_from_slice(&height.to_le_bytes());
                Some(buf)
            }
            Packet::MouseMoveAbs { x, y } => {
                buf[0] = 3;
                buf[1..5].copy_from_slice(&x.to_le_bytes());
                buf[5..9].copy_from_slice(&y.to_le_bytes());
                Some(buf)
            }
            Packet::MouseButton { button, is_down } => {
                buf[0] = 4;
                buf[1] = *button;
                buf[2] = if *is_down { 1 } else { 0 };
                Some(buf)
            }
            Packet::MouseWheel { delta } => {
                buf[0] = 5;
                buf[1..3].copy_from_slice(&delta.to_le_bytes());
                Some(buf)
            }
            Packet::Key { vk, scan, flags, is_down } => {
                buf[0] = 6;
                buf[1..3].copy_from_slice(&vk.to_le_bytes());
                buf[3..5].copy_from_slice(&scan.to_le_bytes());
                buf[5..9].copy_from_slice(&flags.to_le_bytes());
                buf[9] = if *is_down { 1 } else { 0 };
                Some(buf)
            }
            Packet::Heartbeat => {
                buf[0] = 7;
                Some(buf)
            }
            Packet::ReleaseControl => {
                buf[0] = 8;
                Some(buf)
            }
            _ => None,
        }
    }

    pub fn from_bytes_fixed(buf: &[u8; PACKET_SIZE]) -> Option<Self> {
        match buf[0] {
            1 => {
                let width = i32::from_le_bytes(buf[1..5].try_into().ok()?);
                let height = i32::from_le_bytes(buf[5..9].try_into().ok()?);
                Some(Packet::Handshake { width, height })
            }
            2 => {
                let width = i32::from_le_bytes(buf[1..5].try_into().ok()?);
                let height = i32::from_le_bytes(buf[5..9].try_into().ok()?);
                Some(Packet::HandshakeAck { width, height })
            }
            3 => {
                let x = i32::from_le_bytes(buf[1..5].try_into().ok()?);
                let y = i32::from_le_bytes(buf[5..9].try_into().ok()?);
                Some(Packet::MouseMoveAbs { x, y })
            }
            4 => {
                let button = buf[1];
                let is_down = buf[2] != 0;
                Some(Packet::MouseButton { button, is_down })
            }
            5 => {
                let delta = i16::from_le_bytes(buf[1..3].try_into().ok()?);
                Some(Packet::MouseWheel { delta })
            }
            6 => {
                let vk = u16::from_le_bytes(buf[1..3].try_into().ok()?);
                let scan = u16::from_le_bytes(buf[3..5].try_into().ok()?);
                let flags = u32::from_le_bytes(buf[5..9].try_into().ok()?);
                let is_down = buf[9] != 0;
                Some(Packet::Key { vk, scan, flags, is_down })
            }
            7 => Some(Packet::Heartbeat),
            8 => Some(Packet::ReleaseControl),
            _ => None,
        }
    }

    pub fn send<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        match self {
            Packet::ClipboardText { text } => {
                let bytes = text.as_bytes();
                let len = bytes.len() as u32;
                writer.write_all(&[10])?;
                writer.write_all(&len.to_le_bytes())?;
                writer.write_all(bytes)?;
            }
            Packet::ClipboardFile { filename, data } => {
                let name_bytes = filename.as_bytes();
                let name_len = name_bytes.len() as u16;
                let data_len = data.len() as u32;
                writer.write_all(&[11])?;
                writer.write_all(&name_len.to_le_bytes())?;
                writer.write_all(&data_len.to_le_bytes())?;
                writer.write_all(name_bytes)?;
                writer.write_all(data)?;
            }
            _ => {
                if let Some(buf) = self.to_bytes_fixed() {
                    writer.write_all(&buf)?;
                }
            }
        }
        writer.flush()
    }

    pub fn recv<R: Read>(reader: &mut R) -> std::io::Result<Self> {
        let mut id_buf = [0u8; 1];
        reader.read_exact(&mut id_buf)?;
        let id = id_buf[0];

        match id {
            10 => {
                let mut len_buf = [0u8; 4];
                reader.read_exact(&mut len_buf)?;
                let len = u32::from_le_bytes(len_buf) as usize;
                if len > MAX_CLIPBOARD_TEXT_LEN {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Clipboard text quá lớn",
                    ));
                }
                let mut buf = vec![0u8; len];
                reader.read_exact(&mut buf)?;
                let text = String::from_utf8_lossy(&buf).to_string();
                Ok(Packet::ClipboardText { text })
            }
            11 => {
                let mut hdr_buf = [0u8; 6];
                reader.read_exact(&mut hdr_buf)?;
                let name_len = u16::from_le_bytes(hdr_buf[0..2].try_into().unwrap()) as usize;
                let data_len = u32::from_le_bytes(hdr_buf[2..6].try_into().unwrap()) as usize;
                if data_len > MAX_CLIPBOARD_FILE_LEN {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "File clipboard quá lớn (> 500 MB)",
                    ));
                }
                let mut name_buf = vec![0u8; name_len];
                reader.read_exact(&mut name_buf)?;
                let filename = String::from_utf8_lossy(&name_buf).to_string();

                let mut data = vec![0u8; data_len];
                reader.read_exact(&mut data)?;
                Ok(Packet::ClipboardFile { filename, data })
            }
            1..=8 => {
                let mut rest = [0u8; PACKET_SIZE - 1];
                reader.read_exact(&mut rest)?;
                let mut full_buf = [0u8; PACKET_SIZE];
                full_buf[0] = id;
                full_buf[1..].copy_from_slice(&rest);
                Self::from_bytes_fixed(&full_buf).ok_or_else(|| {
                    std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid packet body")
                })
            }
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Unknown packet id: {}", id),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packet_roundtrip() {
        let packets = vec![
            Packet::Handshake { width: 1920, height: 1080 },
            Packet::HandshakeAck { width: 2560, height: 1440 },
            Packet::MouseMoveAbs { x: 500, y: 300 },
            Packet::MouseButton { button: 1, is_down: true },
            Packet::MouseButton { button: 2, is_down: false },
            Packet::MouseWheel { delta: -120 },
            Packet::Key { vk: 0x41, scan: 0x1E, flags: 0, is_down: true },
            Packet::Heartbeat,
            Packet::ReleaseControl,
            Packet::ClipboardText { text: "Xin chào từ Rust!".to_string() },
            Packet::ClipboardFile {
                filename: "test.txt".to_string(),
                data: b"Noi dung tap tin".to_vec(),
            },
        ];

        for pkt in packets {
            let mut buf = Vec::new();
            pkt.send(&mut buf).unwrap();

            let mut cursor = std::io::Cursor::new(buf);
            let parsed = Packet::recv(&mut cursor).unwrap();
            assert_eq!(pkt, parsed);
        }
    }
}
