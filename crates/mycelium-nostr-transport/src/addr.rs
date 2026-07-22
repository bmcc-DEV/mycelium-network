//! Multiaddr Nostr: `/unix/mycelium-nostr/<relay_hex>/<ghost_hex>`
//!
//! O multicodec `/nostr` ainda não existe no crate `multiaddr`; codificamos
//! no protocolo `unix` (string livre). Forma lógica documentada: `/nostr/<relay>/<ghost>`.

use libp2p::multiaddr::{Multiaddr, Protocol};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AddrError {
    #[error("multiaddr não é Nostr Mycelium: {0}")]
    NotNostr(String),
    #[error("ghost inválido (esperado 64 hex): {0}")]
    BadGhost(String),
}

/// Codifica relay URL + ghost hex num multiaddr dialável.
pub fn encode_nostr_multiaddr(relay_url: &str, ghost_hex: &str) -> Multiaddr {
    let path = format!(
        "mycelium-nostr/{}/{}",
        hex::encode(relay_url.as_bytes()),
        ghost_hex.to_lowercase()
    );
    Multiaddr::empty().with(Protocol::Unix(path.into()))
}

/// Extrai `(relay_url, ghost_hex)` de um multiaddr Nostr.
pub fn parse_nostr_multiaddr(addr: &Multiaddr) -> Result<(String, String), AddrError> {
    let mut it = addr.iter();
    match it.next() {
        Some(Protocol::Unix(path)) => {
            let path = path.as_ref();
            let rest = path
                .strip_prefix("mycelium-nostr/")
                .ok_or_else(|| AddrError::NotNostr(addr.to_string()))?;
            let (relay_hex, ghost) = rest
                .split_once('/')
                .ok_or_else(|| AddrError::NotNostr(addr.to_string()))?;
            let relay_bytes = hex::decode(relay_hex)
                .map_err(|_| AddrError::NotNostr(addr.to_string()))?;
            let relay = String::from_utf8(relay_bytes)
                .map_err(|_| AddrError::NotNostr(addr.to_string()))?;
            if ghost.len() != 64 || !ghost.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(AddrError::BadGhost(ghost.to_string()));
            }
            if it.next().is_some() {
                // permitir trailing /p2p/<peer> opcional — ignorar
            }
            Ok((relay, ghost.to_lowercase()))
        }
        _ => Err(AddrError::NotNostr(addr.to_string())),
    }
}

/// `true` se o multiaddr é Nostr Mycelium.
pub fn is_nostr_multiaddr(addr: &Multiaddr) -> bool {
    parse_nostr_multiaddr(addr).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let a = encode_nostr_multiaddr(
            "wss://relay.damus.io",
            "aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899",
        );
        let (r, g) = parse_nostr_multiaddr(&a).unwrap();
        assert_eq!(r, "wss://relay.damus.io");
        assert_eq!(g.len(), 64);
    }
}
