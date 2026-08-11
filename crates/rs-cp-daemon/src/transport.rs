use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::time::Duration;

use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit},
};
use rs_cp_core::DeviceId;

use crate::crypto::{NONCE_BYTES, decode_hex, encode_hex, random_nonce, shared_key};

pub const TEXT_PORT: u16 = 46793;
const MAGIC: &str = "RSCP_TEXT_V2";
const MAX_TEXT_BYTES: usize = 64 * 1024;
const KEY_INFO: &[u8] = b"rs-cp-text-v2";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncryptedTextPacket {
    pub from: DeviceId,
    pub nonce: [u8; NONCE_BYTES],
    pub ciphertext: Vec<u8>,
}

pub fn encrypt_text(
    from: &DeviceId,
    sender_private_key: &[u8; 32],
    recipient_public_key: &str,
    text: &str,
) -> io::Result<EncryptedTextPacket> {
    let key_bytes = shared_key(sender_private_key, recipient_public_key, KEY_INFO)?;
    let cipher = XChaCha20Poly1305::new_from_slice(&key_bytes)
        .map_err(|_| io::Error::other("cipher init failed"))?;
    let nonce = random_nonce();
    let ciphertext = cipher
        .encrypt(XNonce::from_slice(&nonce), text.as_bytes())
        .map_err(|_| io::Error::other("encrypt failed"))?;

    Ok(EncryptedTextPacket {
        from: from.clone(),
        nonce,
        ciphertext,
    })
}

pub fn decrypt_text(
    packet: &EncryptedTextPacket,
    receiver_private_key: &[u8; 32],
    sender_public_key: &str,
) -> io::Result<String> {
    let key_bytes = shared_key(receiver_private_key, sender_public_key, KEY_INFO)?;
    let cipher = XChaCha20Poly1305::new_from_slice(&key_bytes)
        .map_err(|_| io::Error::other("cipher init failed"))?;
    let plaintext = cipher
        .decrypt(
            XNonce::from_slice(&packet.nonce),
            packet.ciphertext.as_ref(),
        )
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "decrypt failed"))?;

    String::from_utf8(plaintext)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "plaintext is not utf-8"))
}

pub fn send_text(addr: impl ToSocketAddrs, packet: &EncryptedTextPacket) -> io::Result<()> {
    let mut stream = TcpStream::connect(addr)?;
    stream.set_nodelay(true)?;
    write_packet(&mut stream, packet)
}

pub fn receive_text_once(timeout: Duration) -> io::Result<Option<EncryptedTextPacket>> {
    let listener = TcpListener::bind(("0.0.0.0", TEXT_PORT))?;
    listener.set_nonblocking(true)?;
    let deadline = std::time::Instant::now() + timeout;

    while std::time::Instant::now() < deadline {
        match listener.accept() {
            Ok((stream, _addr)) => return read_packet(stream).map(Some),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(error) => return Err(error),
        }
    }

    Ok(None)
}

pub fn receive_text_forever(
    mut on_packet: impl FnMut(EncryptedTextPacket) -> io::Result<()>,
) -> io::Result<()> {
    let listener = TcpListener::bind(("0.0.0.0", TEXT_PORT))?;

    for stream in listener.incoming() {
        match stream.and_then(read_packet) {
            Ok(packet) => on_packet(packet)?,
            Err(error) => eprintln!("receive error: {error}"),
        }
    }

    Ok(())
}

fn write_packet(mut writer: impl Write, packet: &EncryptedTextPacket) -> io::Result<()> {
    if packet.ciphertext.len() > MAX_TEXT_BYTES + 32 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "ciphertext too large",
        ));
    }

    writeln!(
        writer,
        "{}\t{}\t{}\t{}",
        MAGIC,
        packet.from,
        encode_hex(&packet.nonce),
        encode_hex(&packet.ciphertext)
    )?;
    Ok(())
}

fn read_packet(reader: impl Read) -> io::Result<EncryptedTextPacket> {
    let mut reader = BufReader::new(reader);
    let mut header = String::new();
    reader.read_line(&mut header)?;

    let mut fields = header.trim_end().split('\t');
    if fields.next() != Some(MAGIC) {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "bad magic"));
    }

    let from = fields
        .next()
        .and_then(|value| DeviceId::new(value.to_string()))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "bad device id"))?;
    let nonce = fields
        .next()
        .and_then(parse_nonce)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "bad nonce"))?;
    let ciphertext = fields
        .next()
        .and_then(|value| decode_hex(value))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "bad ciphertext"))?;

    if ciphertext.len() > MAX_TEXT_BYTES + 32 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "ciphertext too large",
        ));
    }

    Ok(EncryptedTextPacket {
        from,
        nonce,
        ciphertext,
    })
}

fn parse_nonce(value: &str) -> Option<[u8; NONCE_BYTES]> {
    let bytes = decode_hex(value)?;
    if bytes.len() != NONCE_BYTES {
        return None;
    }

    let mut nonce = [0_u8; NONCE_BYTES];
    nonce.copy_from_slice(&bytes);
    Some(nonce)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;
    use crate::crypto::generate_keypair;

    #[test]
    fn encrypted_packet_round_trip_preserves_header() {
        let packet = EncryptedTextPacket {
            from: DeviceId::new("macbook").unwrap(),
            nonce: [7_u8; NONCE_BYTES],
            ciphertext: b"hello".to_vec(),
        };
        let mut bytes = Vec::new();

        write_packet(&mut bytes, &packet).unwrap();
        let decoded = read_packet(Cursor::new(bytes)).unwrap();

        assert_eq!(decoded, packet);
    }

    #[test]
    fn encrypt_then_decrypt_round_trip() {
        let (sender_private, sender_public) = generate_keypair();
        let (receiver_private, receiver_public) = generate_keypair();

        let encrypted = encrypt_text(
            &DeviceId::new("sender").unwrap(),
            &sender_private,
            &crate::crypto::encode_hex(&receiver_public),
            "hello from rs-cp",
        )
        .unwrap();

        let decrypted = decrypt_text(
            &encrypted,
            &receiver_private,
            &crate::crypto::encode_hex(&sender_public),
        )
        .unwrap();

        assert_eq!(decrypted, "hello from rs-cp");
    }
}
