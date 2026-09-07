//! Salted password hashing.
//!
//! Stored format: `sha256$<salt-hex>$<hash-hex>`, where `<salt-hex>` is a
//! fresh random 128-bit salt (uuid v4, 32 hex chars) and `<hash-hex>` is the
//! hex-encoded output of an iterated SHA-256 stretch (`N_ITERATIONS` rounds
//! over `salt || password`).
//!
//! /// SECURITY: this is a best-effort salted hash, NOT a memory-hard password
//! KDF. It fixes the previous placeholder (unsalted `DefaultHasher`, which is
//! neither cryptographic nor stable across processes) but it MUST be swapped
//! for `argon2` (or `bcrypt`/`scrypt`) with a per-password random salt and a
//! stored parameter set before production use. The public API
//! (`hash_password` / `verify_password`) is intentionally shaped so that swap
//! is internal to this module.

use sha2::{Digest, Sha256};

/// Algorithm tag embedded in stored hashes.
const ALGORITHM: &str = "sha256";
/// Stretching rounds applied by [`stretch`]. Tunable; raise if logins stay fast.
const N_ITERATIONS: u32 = 100_000;

/// Hash a password for storage.
///
/// Returns a `sha256$salt$hex` string with a fresh random salt.
/// Rejects empty passwords with an `InvalidInput` error.
pub fn hash_password(password: &str) -> anyhow::Result<String> {
    if password.is_empty() {
        return Err(shared::Error::InvalidInput("password must not be empty".into()).into());
    }
    let salt = uuid::Uuid::new_v4().simple().to_string();
    Ok(format!("{ALGORITHM}${salt}${}", stretch(password, &salt)))
}

/// Verify a password against a stored hash.
///
/// Returns `Ok(true)` on match, `Ok(false)` on mismatch. Malformed hashes or
/// unknown algorithm tags yield an `InvalidInput` error. Comparison of the
/// raw digests is constant-time.
pub fn verify_password(password: &str, hash: &str) -> anyhow::Result<bool> {
    let (algo, salt, expected_hex) = split_hash(hash)?;
    if algo != ALGORITHM {
        return Err(
            shared::Error::InvalidInput(format!("unsupported password hash algorithm `{algo}`"))
                .into(),
        );
    }
    let expected = hex_decode(expected_hex)?;
    if expected.len() != 32 {
        return Err(shared::Error::InvalidInput("malformed password hash".into()).into());
    }
    let actual = hex_decode(&stretch(password, salt))?;
    Ok(constant_time_eq(&expected, &actual))
}

/// Iterate SHA-256 over `salt || password`, returning lowercase hex.
fn stretch(password: &str, salt: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(salt.as_bytes());
    hasher.update(password.as_bytes());
    let mut digest = hasher.finalize();
    for _ in 1..N_ITERATIONS {
        let mut h = Sha256::new();
        h.update(&digest);
        h.update(salt.as_bytes());
        h.update(password.as_bytes());
        digest = h.finalize();
    }
    hex_encode(&digest)
}

/// Split a stored hash into `(algorithm, salt, hex)` or report `InvalidInput`.
fn split_hash(hash: &str) -> anyhow::Result<(&str, &str, &str)> {
    let mut parts = hash.splitn(3, '$');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(algo), Some(salt), Some(hex)) if !algo.is_empty() && !salt.is_empty() && !hex.is_empty() => {
            Ok((algo, salt, hex))
        }
        _ => Err(shared::Error::InvalidInput("malformed password hash".into()).into()),
    }
}

/// Constant-time equality for digest comparison.
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

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

fn hex_decode(hex: &str) -> anyhow::Result<Vec<u8>> {
    if hex.len() % 2 != 0 {
        return Err(shared::Error::InvalidInput("malformed password hash".into()).into());
    }
    let mut out = Vec::with_capacity(hex.len() / 2);
    let bytes = hex.as_bytes();
    for pair in bytes.chunks_exact(2) {
        let hi = hex_val(pair[0])?;
        let lo = hex_val(pair[1])?;
        out.push((hi << 4) | lo);
    }
    Ok(out)
}

fn hex_val(c: u8) -> anyhow::Result<u8> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        _ => Err(shared::Error::InvalidInput("malformed password hash".into()).into()),
    }
}
