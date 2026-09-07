//! TOTP generation/verification (RFC 6238 / RFC 4226, HMAC-SHA1).
//!
//! Secrets are RFC 4648 base32 (uppercase, no padding) encoding 160 random
//! bits. Codes are 6 digits over a 30-second time step; [`verify_code`]
//! accepts ±1 step of clock skew. Secrets that are not valid base32 (e.g.
//! legacy uuid-hex secrets from the previous placeholder) fall back to using
//! their raw bytes as the HMAC key, so existing enrollments keep producing
//! deterministic codes.
//!
//! /// SECURITY gaps vs. a production MFA deployment: no replay protection
//! (a code is valid for its whole time window — callers must track used
//! counters), the system clock is trusted, and secrets must be stored
//! encrypted at rest. A dedicated OTP crate should replace the hand-rolled
//! base32 below if one becomes available.

use chrono::Utc;
use hmac::{Hmac, Mac};
use sha1::Sha1;

/// Seconds per TOTP time step (RFC 6238 default).
const TIME_STEP: i64 = 30;
/// 10^6: codes are zero-padded 6-digit strings (RFC 4226 truncation).
const MODULUS: u32 = 1_000_000;
/// 160-bit secrets, as recommended by RFC 4226.
const SECRET_BYTES: usize = 20;
/// Clock-skew tolerance in time steps, checked on both sides of now.
const SKEW_STEPS: i64 = 1;

/// Generate a fresh random base32 (no padding) TOTP secret.
pub fn generate_secret() -> String {
    let a = uuid::Uuid::new_v4().into_bytes();
    let b = uuid::Uuid::new_v4().into_bytes();
    let mut raw = [0u8; 32];
    raw[..16].copy_from_slice(&a);
    raw[16..].copy_from_slice(&b);
    base32_encode(&raw[..SECRET_BYTES])
}

/// Compute the current 6-digit code for `secret`.
pub fn current_code(secret: &str) -> String {
    code_at(secret, Utc::now().timestamp())
}

/// Verify a 6-digit `code` against `secret`, allowing ±1 time step of skew.
pub fn verify_code(secret: &str, code: &str) -> bool {
    if code.len() != 6 || !code.bytes().all(|c| c.is_ascii_digit()) {
        return false;
    }
    let now = Utc::now().timestamp();
    (-SKEW_STEPS..=SKEW_STEPS).any(|skew| {
        let expected = code_at(secret, now + skew * TIME_STEP);
        constant_time_eq(expected.as_bytes(), code.as_bytes())
    })
}

/// Compute the 6-digit code for `secret` at `unix_seconds` (RFC 4226 §5.3–5.4).
fn code_at(secret: &str, unix_seconds: i64) -> String {
    let key = decode_secret(secret);
    let counter = unix_seconds.div_euclid(TIME_STEP) as u64;
    let mut mac = Hmac::<Sha1>::new_from_slice(&key)
        .expect("HMAC accepts a key of any length");
    mac.update(&counter.to_be_bytes());
    let sum = mac.finalize().into_bytes();
    let offset = (sum[19] & 0x0f) as usize;
    let bin: u32 = ((u32::from(sum[offset]) & 0x7f) << 24)
        | (u32::from(sum[offset + 1]) << 16)
        | (u32::from(sum[offset + 2]) << 8)
        | u32::from(sum[offset + 3]);
    format!("{:06}", bin % MODULUS)
}

/// Decode a base32 secret (padding-tolerant); fall back to raw secret bytes.
fn decode_secret(secret: &str) -> Vec<u8> {
    let normalized = secret.trim().trim_end_matches('=').to_ascii_uppercase();
    base32_decode(&normalized).unwrap_or_else(|_| secret.trim().as_bytes().to_vec())
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// RFC 4648 §6 base32 encoding (uppercase, no padding).
fn base32_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let mut out = String::with_capacity((bytes.len() * 8).div_ceil(5));
    let mut buffer: u32 = 0;
    let mut bits: u8 = 0;
    for &b in bytes {
        buffer = (buffer << 8) | u32::from(b);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(ALPHABET[((buffer >> bits) & 0x1f) as usize] as char);
        }
    }
    if bits > 0 {
        out.push(ALPHABET[((buffer << (5 - bits)) & 0x1f) as usize] as char);
    }
    out
}

/// RFC 4648 §6 base32 decoding. `Err(())` on any invalid character or
/// non-canonical trailing bits.
fn base32_decode(s: &str) -> Result<Vec<u8>, ()> {
    if s.is_empty() {
        return Err(());
    }
    let mut out = Vec::with_capacity(s.len() * 5 / 8);
    let mut buffer: u32 = 0;
    let mut bits: u8 = 0;
    for c in s.bytes() {
        let v: u32 = match c {
            b'A'..=b'Z' => u32::from(c - b'A'),
            b'2'..=b'7' => u32::from(c - b'2') + 26,
            _ => return Err(()),
        };
        buffer = (buffer << 5) | v;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push(((buffer >> bits) & 0xff) as u8);
        }
    }
    // Trailing bits must be zero padding, and fewer than 5 must remain.
    if bits >= 5 || (buffer & ((1u32 << bits) - 1)) != 0 {
        return Err(());
    }
    Ok(out)
}
