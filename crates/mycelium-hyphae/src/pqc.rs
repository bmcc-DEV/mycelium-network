//! PQC (ML-KEM-1024) — handshake híbrido pós-Noise.
//!
//! Após o Noise, peers trocam chaves ML-KEM sobre o canal seguro e
//! derivam `combined = blake3(noise_secret || pqc_secret)`.
//!
//! Isto **não** substitui o Noise — adiciona uma camada PQC por cima,
//! garantindo segurança híbrida mesmo que X25519 seja quebrado um dia.
//!
//! Feature: `pqc-transport`
//!
//! Multiaddr: `/unix/mycelium-pqc/<pk_hex>`

use libp2p::identity::Keypair;
use libp2p::multiaddr::{Multiaddr, Protocol};
use libp2p::PeerId;
use mycelium_pqc::{mlkem_decapsulate, mlkem_encapsulate, mlkem_keygen, KemKeyPair};

/// Segredo híbrido combinado.
pub struct HybridSecret {
    pub combined: [u8; 32],
    pub pqc_raw: Vec<u8>,
}

/// Gera par de chaves ML-KEM.
pub fn generate_pqc_keypair() -> KemKeyPair {
    mlkem_keygen()
}

/// Lado cliente: encapsula para a chave pública do servidor.
pub fn client_handshake(
    server_pk: &[u8],
    noise_secret: &[u8; 32],
) -> Result<(HybridSecret, Vec<u8>), String> {
    let enc = mlkem_encapsulate(server_pk).map_err(|e| e.to_string())?;
    let combined = blake3::hash(&[noise_secret.as_slice(), &enc.shared_secret].concat());
    let ct = enc.ciphertext.clone();
    Ok((HybridSecret { combined: *combined.as_bytes(), pqc_raw: enc.shared_secret.clone() }, ct))
}

/// Lado servidor: decapsula o ciphertext do cliente.
pub fn server_handshake(
    private_key: &[u8],
    ciphertext: &[u8],
    noise_secret: &[u8; 32],
) -> Result<HybridSecret, String> {
    let shared = mlkem_decapsulate(private_key, ciphertext).map_err(|e| e.to_string())?;
    let combined = blake3::hash(&[noise_secret.as_slice(), &shared].concat());
    Ok(HybridSecret { combined: *combined.as_bytes(), pqc_raw: shared })
}

/// PeerId derivado deterministicamente de uma chave pública ML-KEM.
pub fn peer_id_from_pqc_pubkey(pk: &[u8]) -> PeerId {
    let hash = blake3::hash(pk);
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&hash.as_bytes()[..32]);
    let kp = Keypair::ed25519_from_bytes(&mut seed)
        .unwrap_or_else(|_| Keypair::generate_ed25519());
    PeerId::from_public_key(&kp.public())
}

/// Codifica multiaddr PQC.
pub fn encode_pqc_multiaddr(pk_hex: &str) -> Multiaddr {
    let path = format!("mycelium-pqc/{pk_hex}");
    Multiaddr::empty().with(Protocol::Unix(path.into()))
}

/// Decodifica multiaddr PQC.
pub fn parse_pqc_multiaddr(addr: &Multiaddr) -> Option<String> {
    match addr.iter().next()? {
        Protocol::Unix(path) => path.strip_prefix("mycelium-pqc/").map(|s| s.to_string()),
        _ => None,
    }
}

pub const MLKEM_CIPHERTEXT_LEN: usize = 1568;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hybrid_handshake_roundtrip() {
        let server_kp = generate_pqc_keypair();
        let noise = [42u8; 32];
        let (client_result, ct) = client_handshake(&server_kp.public_key, &noise).unwrap();
        assert_eq!(ct.len(), MLKEM_CIPHERTEXT_LEN);
        let server_result = server_handshake(server_kp.private_bytes(), &ct, &noise).unwrap();
        assert_eq!(client_result.combined, server_result.combined);
    }

    #[test]
    fn different_noise_gives_different_combined() {
        let server_kp = generate_pqc_keypair();
        let (a, _) = client_handshake(&server_kp.public_key, &[1u8; 32]).unwrap();
        let (b, _) = client_handshake(&server_kp.public_key, &[2u8; 32]).unwrap();
        assert_ne!(a.combined, b.combined);
    }

    #[test]
    fn peer_id_is_deterministic() {
        let pk = b"test-public-key-32-bytes-long!!";
        let a = peer_id_from_pqc_pubkey(pk);
        let b = peer_id_from_pqc_pubkey(pk);
        assert_eq!(a, b);
    }

    #[test]
    fn pqc_multiaddr_roundtrip() {
        let addr = encode_pqc_multiaddr("aabbccdd");
        let pk = parse_pqc_multiaddr(&addr).unwrap();
        assert_eq!(pk, "aabbccdd");
    }
}
