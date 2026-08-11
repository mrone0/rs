use std::io;

use hkdf::Hkdf;
use rand_core::{OsRng, RngCore};
use sha2::Sha256;
use x25519_dalek::{PublicKey, StaticSecret};

pub const PUBLIC_KEY_BYTES: usize = 32;
pub const NONCE_BYTES: usize = 24;
pub const SHARED_KEY_BYTES: usize = 32;

pub fn generate_keypair() -> ([u8; PUBLIC_KEY_BYTES], [u8; PUBLIC_KEY_BYTES]) {
    let secret = StaticSecret::random_from_rng(OsRng);
    let public = PublicKey::from(&secret);
    (secret.to_bytes(), public.to_bytes())
}

pub fn shared_key(
    private_key: &[u8; PUBLIC_KEY_BYTES],
    peer_public_key_hex: &str,
    info: &[u8],
) -> io::Result<[u8; SHARED_KEY_BYTES]> {
    let peer_public_key_bytes = decode_hex(peer_public_key_hex)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "bad peer public key"))?;
    if peer_public_key_bytes.len() != PUBLIC_KEY_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "bad peer public key length",
        ));
    }

    let mut peer_bytes = [0_u8; PUBLIC_KEY_BYTES];
    peer_bytes.copy_from_slice(&peer_public_key_bytes);

    let secret = StaticSecret::from(*private_key);
    let shared = secret.diffie_hellman(&PublicKey::from(peer_bytes));
    let hkdf = Hkdf::<Sha256>::new(None, shared.as_bytes());

    let mut derived = [0_u8; SHARED_KEY_BYTES];
    hkdf.expand(info, &mut derived)
        .map_err(|_| io::Error::other("hkdf expand failed"))?;
    Ok(derived)
}

pub fn random_nonce() -> [u8; NONCE_BYTES] {
    let mut nonce = [0_u8; NONCE_BYTES];
    OsRng.fill_bytes(&mut nonce);
    nonce
}

pub fn encode_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(&mut output, "{:02x}", byte);
    }
    output
}

pub fn decode_hex(value: &str) -> Option<Vec<u8>> {
    let value = value.trim();
    if value.len() % 2 != 0 {
        return None;
    }

    let mut bytes = Vec::with_capacity(value.len() / 2);
    let mut index = 0;
    while index < value.len() {
        let byte = u8::from_str_radix(&value[index..index + 2], 16).ok()?;
        bytes.push(byte);
        index += 2;
    }
    Some(bytes)
}
