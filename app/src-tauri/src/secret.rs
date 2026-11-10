//! Built-in server addresses. build.rs encrypts FOREVER_SERVER_ADDRESS / FOREVER_SERVER_LAN_ADDRESS with AES-256-GCM
//! (random key and nonces, constants in the binary), so the exe never holds them as plain text. They are decrypted
//! in memory only when the launcher connects, and are never logged or shown.
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};

include!(concat!(env!("OUT_DIR"), "/server_blob.rs"));

fn open(nonce: &[u8; 12], sealed: &[u8]) -> String {
    if sealed.is_empty() {
        return String::new();
    }
    let Ok(cipher) = Aes256Gcm::new_from_slice(&KEY) else { return String::new() };
    cipher
        .decrypt(Nonce::from_slice(nonce), sealed)
        .ok()
        .and_then(|p| String::from_utf8(p).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// Public address first, then the LAN fallback (decrypted now, dropped by the caller after use).
pub fn addresses() -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for a in [open(&NONCE_A, SEALED_A), open(&NONCE_L, SEALED_L)] {
        if !a.is_empty() && !v.contains(&a) {
            v.push(a);
        }
    }
    v
}