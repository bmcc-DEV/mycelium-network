//! # mycelium-ghostid
//!
//! Identidade efémera secp256k1 compatível com Nostr (NIP-01 Schnorr).
//! O pheromone ed25519 do nó permanece na camada de aplicação; GhostID
//! serve só para eventos Nostr / shards QEL.

mod decoherence;

pub use decoherence::DecoherenceState;

use rand::rngs::OsRng;
use rand::RngCore;
use secp256k1::schnorr::Signature as SchnorrSignature;
use secp256k1::{Keypair, Message, Secp256k1, SecretKey, XOnlyPublicKey};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};
use zeroize::Zeroize;

/// Erros do GhostID.
#[derive(Debug, thiserror::Error)]
pub enum GhostError {
    #[error("seed inválido para secp256k1")]
    InvalidSeed,
    #[error("assinatura Schnorr inválida")]
    BadSignature,
    #[error("GhostID expirado (TTL)")]
    Expired,
}

/// Coletor de entropia passiva (nunca persistido).
pub struct EntropyCollector {
    buffer: Vec<u8>,
}

impl Default for EntropyCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl EntropyCollector {
    pub fn new() -> Self {
        Self {
            buffer: Vec::with_capacity(256),
        }
    }

    /// Ritmo de digitação (intervalo entre keystrokes em µs). Stub API.
    pub fn add_keystroke_timing(&mut self, interval_us: u64) {
        self.buffer.extend_from_slice(&interval_us.to_le_bytes());
        self.mix();
    }

    /// Giroscópio (3 eixos). Stub API.
    pub fn add_gyro(&mut self, x: f32, y: f32, z: f32) {
        self.buffer.extend_from_slice(&x.to_le_bytes());
        self.buffer.extend_from_slice(&y.to_le_bytes());
        self.buffer.extend_from_slice(&z.to_le_bytes());
        self.mix();
    }

    /// Pressão de toque. Stub API.
    pub fn add_touch_pressure(&mut self, pressure: f32, x: u16, y: u16) {
        self.buffer.extend_from_slice(&pressure.to_le_bytes());
        self.buffer.extend_from_slice(&x.to_le_bytes());
        self.buffer.extend_from_slice(&y.to_le_bytes());
        self.mix();
    }

    /// Entropia do sistema (sempre disponível).
    pub fn add_system_entropy(&mut self) {
        let mut rng = OsRng;
        let mut sys = [0u8; 64];
        rng.fill_bytes(&mut sys);
        self.buffer.extend_from_slice(&sys);
        if let Ok(now) = SystemTime::now().duration_since(UNIX_EPOCH) {
            self.buffer.extend_from_slice(&now.as_nanos().to_le_bytes());
        }
        self.mix();
    }

    fn mix(&mut self) {
        let mut hasher = Sha256::new();
        hasher.update(&self.buffer);
        let mixed = hasher.finalize();
        self.buffer.clear();
        self.buffer.extend_from_slice(&mixed);
    }

    /// Extrai seed de 32 bytes e zero-fill do buffer.
    pub fn extract_seed(mut self) -> [u8; 32] {
        self.mix();
        let mut seed = [0u8; 32];
        let take = self.buffer.len().min(32);
        seed[..take].copy_from_slice(&self.buffer[..take]);
        if take < 32 {
            OsRng.fill_bytes(&mut seed[take..]);
        }
        self.buffer.zeroize();
        seed
    }
}

impl Drop for EntropyCollector {
    fn drop(&mut self) {
        self.buffer.zeroize();
    }
}

/// GhostID: identidade fantasma compatível com Nostr.
pub struct GhostId {
    keypair: Keypair,
    public_xonly: [u8; 32],
    peer_id_bytes: [u8; 32],
    created_at: u64,
    ttl_secs: u64,
}

impl GhostId {
    /// Cria GhostID a partir de entropia coletada.
    pub fn spawn(entropy: EntropyCollector, ttl_secs: u64) -> Result<Self, GhostError> {
        let mut seed = entropy.extract_seed();
        let result = Self::from_seed(seed, ttl_secs);
        seed.zeroize();
        result
    }

    /// GhostID rápido (só entropia do sistema).
    pub fn spawn_quick(ttl_secs: u64) -> Result<Self, GhostError> {
        let mut collector = EntropyCollector::new();
        collector.add_system_entropy();
        Self::spawn(collector, ttl_secs)
    }

    fn from_seed(seed: [u8; 32], ttl_secs: u64) -> Result<Self, GhostError> {
        let secp = Secp256k1::new();
        let secret = SecretKey::from_slice(&seed).map_err(|_| GhostError::InvalidSeed)?;
        let keypair = Keypair::from_secret_key(&secp, &secret);
        let (xonly, _parity) = keypair.x_only_public_key();
        let public_xonly = xonly.serialize();

        let peer_id_bytes: [u8; 32] = {
            let mut hasher = Sha256::new();
            hasher.update(b"mycelium-ghostid-v1");
            hasher.update(public_xonly);
            hasher.finalize().into()
        };

        Ok(Self {
            keypair,
            public_xonly,
            peer_id_bytes,
            created_at: now_secs(),
            ttl_secs,
        })
    }

    /// Nostr pubkey (hex, 64 chars).
    pub fn nostr_pubkey_hex(&self) -> String {
        hex::encode(self.public_xonly)
    }

    /// Nostr pubkey (x-only, 32 bytes).
    pub fn nostr_pubkey(&self) -> [u8; 32] {
        self.public_xonly
    }

    /// Assina um event id Nostr (32 bytes) com Schnorr (NIP-01).
    pub fn sign_nostr_event(&self, event_id: &[u8; 32]) -> [u8; 64] {
        let secp = Secp256k1::new();
        let msg = Message::from_digest_slice(event_id).expect("32-byte digest");
        let sig = secp.sign_schnorr(&msg, &self.keypair);
        *sig.as_ref()
    }

    /// Assina dados arbitrários (SHA-256 → Schnorr).
    pub fn sign(&self, data: &[u8]) -> [u8; 64] {
        let hash: [u8; 32] = Sha256::digest(data).into();
        self.sign_nostr_event(&hash)
    }

    /// Verifica assinatura Schnorr sobre um event id.
    pub fn verify_nostr_event(
        pubkey: &[u8; 32],
        event_id: &[u8; 32],
        sig: &[u8; 64],
    ) -> Result<(), GhostError> {
        let secp = Secp256k1::verification_only();
        let xonly = XOnlyPublicKey::from_slice(pubkey).map_err(|_| GhostError::BadSignature)?;
        let msg = Message::from_digest_slice(event_id).map_err(|_| GhostError::BadSignature)?;
        let signature =
            SchnorrSignature::from_slice(sig).map_err(|_| GhostError::BadSignature)?;
        secp.verify_schnorr(&signature, &msg, &xonly)
            .map_err(|_| GhostError::BadSignature)
    }

    pub fn is_expired(&self) -> bool {
        now_secs().saturating_sub(self.created_at) > self.ttl_secs
    }

    pub fn ensure_alive(&self) -> Result<(), GhostError> {
        if self.is_expired() {
            Err(GhostError::Expired)
        } else {
            Ok(())
        }
    }

    pub fn peer_id_bytes(&self) -> [u8; 32] {
        self.peer_id_bytes
    }

    pub fn ttl_secs(&self) -> u64 {
        self.ttl_secs
    }

    pub fn created_at_secs(&self) -> u64 {
        self.created_at
    }

    /// Secret key bytes (uso interno NIP-44). Preferir não exportar.
    pub fn secret_key_bytes(&self) -> [u8; 32] {
        self.keypair.secret_key().secret_bytes()
    }
}

impl Drop for GhostId {
    fn drop(&mut self) {
        // Keypair/SecretKey do secp256k1 0.29 usam zeroize interno no drop;
        // forçamos overwrite do material público derivado.
        self.public_xonly.zeroize();
        self.peer_id_bytes.zeroize();
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Hash SHA-256 de 32 bytes (útil para event ids auxiliares).
pub fn sha256(data: &[u8]) -> [u8; 32] {
    Sha256::digest(data).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_quick_pubkey_hex_is_64_chars() {
        let g = GhostId::spawn_quick(3600).expect("spawn");
        let hex = g.nostr_pubkey_hex();
        assert_eq!(hex.len(), 64);
        assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(!g.is_expired());
    }

    #[test]
    fn sign_verify_schnorr_roundtrip() {
        let g = GhostId::spawn_quick(3600).expect("spawn");
        let event_id = sha256(b"mycelium-test-event");
        let sig = g.sign_nostr_event(&event_id);
        GhostId::verify_nostr_event(&g.nostr_pubkey(), &event_id, &sig).expect("verify");
    }

    #[test]
    fn bad_signature_rejected() {
        let g = GhostId::spawn_quick(3600).expect("spawn");
        let event_id = sha256(b"event-a");
        let mut sig = g.sign_nostr_event(&event_id);
        sig[0] ^= 0xff;
        assert!(GhostId::verify_nostr_event(&g.nostr_pubkey(), &event_id, &sig).is_err());
    }

    #[test]
    fn entropy_collector_extracts_32_bytes() {
        let mut c = EntropyCollector::new();
        c.add_system_entropy();
        c.add_keystroke_timing(12345);
        c.add_gyro(0.1, 0.2, 0.3);
        let seed = c.extract_seed();
        assert_eq!(seed.len(), 32);
        assert!(seed.iter().any(|&b| b != 0));
    }
}
