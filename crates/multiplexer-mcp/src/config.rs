//! Server identity and the reuse key (config hash).

use std::fmt;

/// Configured server name. Distinct from the reuse key: two configs can share
/// a name after a command/args/env change, and then have different hashes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ServerId(String);

impl ServerId {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for ServerId {
    fn from(name: &str) -> Self {
        Self::new(name)
    }
}

impl From<String> for ServerId {
    fn from(name: String) -> Self {
        Self::new(name)
    }
}

impl AsRef<str> for ServerId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for ServerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Opaque 32-byte reuse key. Non-cryptographic identity only: same canonical
/// config fields produce the same hash, and it is stable across processes.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConfigHash([u8; 32]);

impl ConfigHash {
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(&self) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut out = String::with_capacity(64);
        for byte in self.0 {
            out.push(HEX[(byte >> 4) as usize] as char);
            out.push(HEX[(byte & 0x0f) as usize] as char);
        }
        out
    }
}

impl fmt::Debug for ConfigHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ConfigHash").field(&self.to_hex()).finish()
    }
}

impl fmt::Display for ConfigHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

/// Spawn description with secret *keys* only, never secret values (D23).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerConfig {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env_keys: Vec<String>,
}

impl ServerConfig {
    pub fn new(
        name: impl Into<String>,
        command: impl Into<String>,
        args: Vec<String>,
        env_keys: Vec<String>,
    ) -> Self {
        Self {
            name: name.into(),
            command: command.into(),
            args,
            env_keys,
        }
    }

    pub fn server_id(&self) -> ServerId {
        ServerId::new(self.name.clone())
    }
}

/// Length-prefixed canonical encoding so field boundaries cannot collide.
fn canonicalize(cfg: &ServerConfig) -> Vec<u8> {
    let mut buf = Vec::new();
    write_str(&mut buf, &cfg.name);
    write_str(&mut buf, &cfg.command);
    write_u32(&mut buf, u32::try_from(cfg.args.len()).unwrap_or(u32::MAX));
    for arg in &cfg.args {
        write_str(&mut buf, arg);
    }
    let mut keys = cfg.env_keys.clone();
    keys.sort();
    write_u32(&mut buf, u32::try_from(keys.len()).unwrap_or(u32::MAX));
    for key in &keys {
        write_str(&mut buf, key);
    }
    buf
}

fn write_u32(buf: &mut Vec<u8>, n: u32) {
    buf.extend_from_slice(&n.to_le_bytes());
}

fn write_str(buf: &mut Vec<u8>, s: &str) {
    write_u32(buf, u32::try_from(s.len()).unwrap_or(u32::MAX));
    buf.extend_from_slice(s.as_bytes());
}

const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

fn fnv1a_64(data: &[u8], offset: u64) -> u64 {
    let mut hash = offset;
    for byte in data {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Identity hash of [`ServerConfig`]. Not for security or integrity.
///
/// `env_keys` are sorted before hashing so key order does not change identity.
/// `args` keep their given order (command-line order is significant).
pub fn config_hash(cfg: &ServerConfig) -> ConfigHash {
    let canonical = canonicalize(cfg);
    let mut bytes = [0u8; 32];
    // Four FNV-1a lanes with distinct IVs expand the digest to 32 bytes.
    const IVS: [u64; 4] = [
        FNV_OFFSET,
        0x6c62_272e_07bb_0142,
        0x9ae1_6a3b_2f90_404f,
        0x517c_c1b7_2722_0a95,
    ];
    for (i, iv) in IVS.iter().enumerate() {
        let lane = fnv1a_64(&canonical, *iv);
        bytes[i * 8..(i + 1) * 8].copy_from_slice(&lane.to_le_bytes());
    }
    ConfigHash(bytes)
}
