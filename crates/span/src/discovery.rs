use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use span_core::{DeviceId, DeviceInfo, Platform, TrustState};

use crate::config::{LocalDevice, parse_platform, platform_name, sanitize};
use crate::crypto::decode_hex;

pub const DISCOVERY_PORT: u16 = 46792;
const MAGIC: &str = "SPAN_DISCOVERY_V2";
const PROBE_MAGIC: &str = "SPAN_DISCOVERY_PROBE_V1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiscoveryMessage {
    Announcement(DiscoveryPacket),
    Probe,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveryPacket {
    pub id: DeviceId,
    pub name: String,
    pub platform: Platform,
    pub public_key: String,
}

impl DiscoveryPacket {
    pub fn from_local(device: &LocalDevice) -> Self {
        Self {
            id: device.id.clone(),
            name: device.name.clone(),
            platform: device.platform,
            public_key: device.public_key_hex(),
        }
    }

    pub fn into_device_info(self) -> DeviceInfo {
        DeviceInfo {
            id: self.id,
            name: self.name,
            platform: self.platform,
            trust_state: TrustState::Discovered,
            endpoint: None,
            public_key: Some(self.public_key),
        }
    }
}

pub fn broadcast_once(device: &LocalDevice) -> io::Result<()> {
    let socket = UdpSocket::bind(("0.0.0.0", 0))?;
    socket.set_broadcast(true)?;
    let packet = encode_packet(&DiscoveryPacket::from_local(device));
    socket.send_to(packet.as_bytes(), ("255.255.255.255", DISCOVERY_PORT))?;
    Ok(())
}

pub fn discover_once(
    _device: &LocalDevice,
    timeout: Duration,
) -> io::Result<Vec<(DiscoveryPacket, SocketAddr)>> {
    // Use an ephemeral source port. The daemon owns UDP 46792, so a manual
    // scan must not compete with it for the discovery socket.
    let socket = UdpSocket::bind(("0.0.0.0", 0))?;
    socket.set_broadcast(true)?;
    socket.set_read_timeout(Some(Duration::from_millis(150)))?;
    socket.send_to(PROBE_MAGIC.as_bytes(), ("255.255.255.255", DISCOVERY_PORT))?;

    let deadline = Instant::now() + timeout;
    let mut buffer = [0_u8; 1024];
    let mut packets = Vec::new();

    while Instant::now() < deadline {
        match socket.recv_from(&mut buffer) {
            Ok((len, addr)) => {
                if let Ok(value) = std::str::from_utf8(&buffer[..len]) {
                    if let Some(DiscoveryMessage::Announcement(packet)) = decode_message(value) {
                        packets.push((packet, addr));
                    }
                }
            }
            Err(error)
                if error.kind() == io::ErrorKind::WouldBlock
                    || error.kind() == io::ErrorKind::TimedOut => {}
            Err(error) => return Err(error),
        }
    }

    Ok(packets)
}

pub fn respond_to_probe(device: &LocalDevice, target: SocketAddr) -> io::Result<()> {
    let socket = UdpSocket::bind(("0.0.0.0", 0))?;
    let packet = encode_packet(&DiscoveryPacket::from_local(device));
    socket.send_to(packet.as_bytes(), target)?;
    Ok(())
}

pub fn listen_forever(
    mut on_message: impl FnMut(DiscoveryMessage, SocketAddr) -> io::Result<()>,
) -> io::Result<()> {
    let socket = UdpSocket::bind(("0.0.0.0", DISCOVERY_PORT))?;
    let mut buffer = [0_u8; 1024];

    loop {
        let (len, addr) = socket.recv_from(&mut buffer)?;
        if let Ok(value) = std::str::from_utf8(&buffer[..len]) {
            if let Some(message) = decode_message(value) {
                on_message(message, addr)?;
            }
        }
    }
}

fn encode_packet(packet: &DiscoveryPacket) -> String {
    format!(
        "{}\t{}\t{}\t{}\t{}",
        MAGIC,
        packet.id,
        sanitize(&packet.name),
        platform_name(packet.platform),
        packet.public_key
    )
}

fn decode_message(value: &str) -> Option<DiscoveryMessage> {
    if value.trim() == PROBE_MAGIC {
        return Some(DiscoveryMessage::Probe);
    }

    let mut fields = value.trim().split('\t');
    if fields.next()? != MAGIC {
        return None;
    }

    Some(DiscoveryMessage::Announcement(DiscoveryPacket {
        id: DeviceId::new(fields.next()?.to_string())?,
        name: fields.next()?.to_string(),
        platform: parse_platform(fields.next()?),
        public_key: fields.next().and_then(validate_public_key)?,
    }))
}

fn validate_public_key(value: &str) -> Option<String> {
    let bytes = decode_hex(value)?;
    (bytes.len() == 32).then(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packet_round_trip_preserves_device() {
        let packet = DiscoveryPacket {
            id: DeviceId::new("abc").unwrap(),
            name: "MacBook".to_string(),
            platform: Platform::MacOs,
            public_key: "aa".repeat(32),
        };

        assert_eq!(
            decode_message(&encode_packet(&packet)),
            Some(DiscoveryMessage::Announcement(packet))
        );
        assert_eq!(decode_message(PROBE_MAGIC), Some(DiscoveryMessage::Probe));
    }
}
