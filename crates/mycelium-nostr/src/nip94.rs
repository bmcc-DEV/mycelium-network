//! NIP-94 (kind 1063) — anúncio de Plot Mycelium.
//!
//! `i` = ContentId (`Qm…` Blake3 cosmético, NÃO CID IPFS real).
//! `x` = hex Blake3 do conteúdo.

use crate::NostrError;
use mycelium_ghostid::GhostId;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

/// Evento Nostr NIP-01.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NostrEvent {
    pub id: String,
    pub pubkey: String,
    pub created_at: u64,
    pub kind: u16,
    pub tags: Vec<Vec<String>>,
    pub content: String,
    pub sig: String,
}

/// Calcula event id NIP-01: sha256([0, pubkey, created_at, kind, tags, content]).
pub fn compute_event_id(
    pubkey_hex: &str,
    created_at: u64,
    kind: u16,
    tags: &[Vec<String>],
    content: &str,
) -> [u8; 32] {
    let serialized = json!([0, pubkey_hex, created_at, kind, tags, content]);
    // Canonical compact JSON (serde_json default, no spaces)
    let bytes = serialized.to_string();
    Sha256::digest(bytes.as_bytes()).into()
}

/// Assina e completa um evento parcial.
pub fn seal_event(
    ghost: &GhostId,
    created_at: u64,
    kind: u16,
    tags: Vec<Vec<String>>,
    content: String,
) -> Result<NostrEvent, NostrError> {
    let pubkey = ghost.nostr_pubkey_hex();
    let event_id = compute_event_id(&pubkey, created_at, kind, &tags, &content);
    let sig = ghost.sign_nostr_event(&event_id);
    Ok(NostrEvent {
        id: hex::encode(event_id),
        pubkey,
        created_at,
        kind,
        tags,
        content,
        sig: hex::encode(sig),
    })
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Cria evento NIP-94 (kind 1063) para um Plot.
pub fn announce_plot(
    ghost: &GhostId,
    content_id: &str,
    blake3_hex: &str,
    size: usize,
    qel_params: Option<(u8, u8)>,
    lattice_type: &str,
) -> Result<NostrEvent, NostrError> {
    let created_at = now_secs();
    let mut tags = vec![
        vec!["url".into(), format!("mycelium://{content_id}")],
        vec!["m".into(), "application/x-mycelium-plot".into()],
        vec!["x".into(), blake3_hex.into()],
        vec!["size".into(), size.to_string()],
        vec!["i".into(), content_id.into()],
        vec!["lattice".into(), lattice_type.into()],
    ];
    if let Some((k, n)) = qel_params {
        tags.push(vec!["qel".into(), format!("{k},{n}")]);
    }
    seal_event(ghost, created_at, 1063, tags, String::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nip94_event_has_expected_tags() {
        let ghost = GhostId::spawn_quick(3600).unwrap();
        let ev = announce_plot(
            &ghost,
            "Qmabcd",
            "aa".repeat(32).as_str(),
            42,
            Some((3, 7)),
            "giggs/plot",
        )
        .unwrap();
        assert_eq!(ev.kind, 1063);
        assert_eq!(ev.pubkey.len(), 64);
        assert_eq!(ev.id.len(), 64);
        assert_eq!(ev.sig.len(), 128);
        assert!(ev.tags.iter().any(|t| t.get(0).map(|s| s.as_str()) == Some("i")));
        assert!(ev
            .tags
            .iter()
            .any(|t| t.as_slice() == ["qel", "3,7"]));
        GhostId::verify_nostr_event(
            &ghost.nostr_pubkey(),
            &hex::decode(&ev.id).unwrap().try_into().unwrap(),
            &hex::decode(&ev.sig).unwrap().try_into().unwrap(),
        )
        .unwrap();
    }
}
