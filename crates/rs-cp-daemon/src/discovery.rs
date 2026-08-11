use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use rs_cp_core::{DeviceId, DeviceInfo, Platform, TrustState};

use crate::config::{LocalDevice, parse_platform, platform_name, sanitize};
use crate::crypto::decode_hex;

pub const DISCOVERY_PORT: u16 = 46792;
const MAGIC: &str = "RSCP_DISCOVERY_V2";

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

pub fn listen_once(timeout: Duration) -> io::Result<Vec<(DiscoveryPacket, SocketAddr)>> {
    let socket = UdpSocket::bind(("0.0.0.0", DISCOVERY_PORT))?;
    socket.set_read_timeout(Some(Duration::from_millis(150)))?;

    let deadline = Instant::now() + timeout;
    let mut buffer = [0_u8; 1024];
    let mut packets = Vec::new();

    while Instant::now() < deadline {
        match socket.recv_from(&mut buffer) {
            Ok((len, addr)) => {
                if let Ok(value) = std::str::from_utf8(&buffer[..len]) {
                    if let Some(packet) = decode_packet(value) {
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

pub fn listen_forever(
    mut on_packet: impl FnMut(DiscoveryPacket, SocketAddr) -> io::Result<()>,
) -> io::Result<()> {
    let socket = UdpSocket::bind(("0.0.0.0", DISCOVERY_PORT))?;
    let mut buffer = [0_u8; 1024];

    loop {
        let (len, addr) = socket.recv_from(&mut buffer)?;
        if let Ok(value) = std::str::from_utf8(&buffer[..len]) {
            if let Some(packet) = decode_packet(value) {
                on_packet(packet, addr)?;
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

fn decode_packet(value: &str) -> Option<DiscoveryPacket> {
    let mut fields = value.trim().split('\t');
    if fields.next()? != MAGIC {
        return None;
    }

    Some(DiscoveryPacket {
        id: DeviceId::new(fields.next()?.to_string())?,
        name: fields.next()?.to_string(),
        platform: parse_platform(fields.next()?),
        public_key: fields.next().and_then(validate_public_key)?,
    })
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

        assert_eq!(decode_packet(&encode_packet(&packet)), Some(packet));
    }
}
