//! Eventos de shard QEL (kind 31234) + NIP-44 opcional.

use crate::nip94::{seal_event, NostrEvent};
use crate::relay_pool::RelayPool;
use crate::NostrError;
use mycelium_ghostid::GhostId;
use mycelium_qel::QelShard;
use secp256k1::{SecretKey, XOnlyPublicKey};
use serde_json::json;

/// Kind custom Mycelium para shards QEL.
///
/// Usa a faixa addressable (30000–39999, NIP-33): cada shard precisa de tag `d`
/// única — sem isso os relays substituem eventos do mesmo autor e só fica 1 shard.
pub const KIND_QEL_SHARD: u16 = 31234;

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Cria evento kind 31234 com shard (NIP-44 se recipient, senão JSON plaintext).
pub fn create_shard_event(
    ghost: &GhostId,
    shard: &QelShard,
    recipient_pubkey_hex: Option<&str>,
) -> Result<NostrEvent, NostrError> {
    let created_at = now_secs();
    let plain = serde_json::to_string(shard)?;

    let content = if let Some(recip_hex) = recipient_pubkey_hex {
        encrypt_nip44(ghost, recip_hex, &plain)?
    } else {
        plain
    };

    let mut tags = vec![
        // NIP-33: `d` único por shard evita replace entre irmãos do mesmo GhostId
        vec![
            "d".into(),
            format!("{}:{}", shard.content_id, shard.index),
        ],
        vec!["i".into(), shard.content_id.clone()],
        vec![
            "shard".into(),
            format!("{}/{}", shard.index, shard.total),
        ],
        vec![
            "qel".into(),
            format!("{},{}", shard.threshold, shard.total),
        ],
        vec!["transport".into(), "nostr".into()],
    ];
    if let Some(r) = recipient_pubkey_hex {
        tags.push(vec!["p".into(), r.to_string()]);
    }

    seal_event(ghost, created_at, KIND_QEL_SHARD, tags, content)
}

/// NIP-44 encrypt (pubkey x-only hex do destinatário).
pub fn encrypt_nip44(ghost: &GhostId, recipient_hex: &str, plaintext: &str) -> Result<String, NostrError> {
    let sk_bytes = ghost.secret_key_bytes();
    let sk = SecretKey::from_slice(&sk_bytes).map_err(|e| NostrError::Nip44(e.to_string()))?;
    let recip_bytes =
        hex::decode(recipient_hex).map_err(|e| NostrError::InvalidHex(e.to_string()))?;
    if recip_bytes.len() != 32 {
        return Err(NostrError::InvalidHex(
            "pubkey destinatário deve ter 32 bytes".into(),
        ));
    }
    let xonly = XOnlyPublicKey::from_slice(&recip_bytes)
        .map_err(|e| NostrError::Nip44(e.to_string()))?;
    let convo = nip44::get_conversation_key(sk, xonly);
    nip44::encrypt(&convo, plaintext).map_err(|e| NostrError::Nip44(e.to_string()))
}

/// Decifra content NIP-44 para string (ou devolve plaintext se já for texto/JSON).
pub fn decrypt_nip44_to_string(
    recipient_secret: &[u8; 32],
    sender_pubkey_hex: &str,
    content: &str,
) -> Result<String, NostrError> {
    if content.trim_start().starts_with('{') {
        return Ok(content.to_string());
    }
    let sk = SecretKey::from_slice(recipient_secret).map_err(|e| NostrError::Nip44(e.to_string()))?;
    let sender_bytes =
        hex::decode(sender_pubkey_hex).map_err(|e| NostrError::InvalidHex(e.to_string()))?;
    let xonly = XOnlyPublicKey::from_slice(&sender_bytes)
        .map_err(|e| NostrError::Nip44(e.to_string()))?;
    let convo = nip44::get_conversation_key(sk, xonly);
    nip44::decrypt(&convo, content).map_err(|e| NostrError::Nip44(e.to_string()))
}

/// Decifra content NIP-44 (ou passa plaintext JSON).
pub fn decrypt_shard_content(
    recipient_secret: Option<&[u8; 32]>,
    sender_pubkey_hex: &str,
    content: &str,
) -> Result<QelShard, NostrError> {
    let json_str = if content.trim_start().starts_with('{') {
        content.to_string()
    } else if let Some(sec) = recipient_secret {
        decrypt_nip44_to_string(sec, sender_pubkey_hex, content)?
    } else {
        return Err(NostrError::Msg(
            "conteúdo cifrado sem chave do destinatário".into(),
        ));
    };
    Ok(serde_json::from_str(&json_str)?)
}

/// Publica anúncio NIP-94 + shards Nos.
pub async fn publish_shards(
    pool: &RelayPool,
    ghost: &GhostId,
    shards: &[QelShard],
    blake3_hex: &str,
    plot_size: usize,
    recipient_pubkey_hex: Option<&str>,
) -> Result<usize, NostrError> {
    if shards.is_empty() {
        return Err(NostrError::Msg("sem shards".into()));
    }
    let cid = &shards[0].content_id;
    let announce = crate::nip94::announce_plot(
        ghost,
        cid,
        blake3_hex,
        plot_size,
        Some((shards[0].threshold, shards[0].total)),
        "giggs/plot",
    )?;
    let mut published = pool.publish(&announce).await.unwrap_or(0);

    // Publicar só `threshold` shards via Nostr (bastam para reconstruir).
    let need = shards[0].threshold as usize;
    for shard in shards.iter().take(need.max(1)) {
        let ev = create_shard_event(ghost, shard, recipient_pubkey_hex)?;
        match pool.publish(&ev).await {
            Ok(n) => published += n,
            Err(e) => tracing::warn!(error = %e, index = shard.index, "shard publish falhou"),
        }
    }
    if published == 0 {
        Err(NostrError::AllRelaysFailed)
    } else {
        Ok(published)
    }
}

/// Busca shards QEL por ContentId até threshold.
pub async fn fetch_shards(
    pool: &RelayPool,
    content_id: &str,
    threshold: u8,
    recipient_secret: Option<&[u8; 32]>,
) -> Result<Vec<QelShard>, NostrError> {
    let since = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().saturating_sub(3600))
        .unwrap_or(0);

    // Filtro por tag `i` + fallback sem tag (alguns relays indexam mal).
    let filters = [
        json!({
            "kinds": [KIND_QEL_SHARD],
            "#i": [content_id],
            "since": since,
            "limit": 50
        }),
        json!({
            "kinds": [KIND_QEL_SHARD],
            "since": since,
            "limit": 50
        }),
    ];

    let mut shards = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for filter in filters {
        let events = pool.subscribe(filter).await?;
        tracing::info!(n = events.len(), cid = %content_id, "eventos Nostr recebidos");
        for ev in events {
            // Confirma tag i se presente
            let tag_ok = ev.tags.iter().any(|t| {
                t.len() >= 2 && (t[0] == "i" || t[0] == "cid") && t[1] == content_id
            }) || ev.content.contains(content_id);

            if !tag_ok {
                continue;
            }
            match decrypt_shard_content(recipient_secret, &ev.pubkey, &ev.content) {
                Ok(shard) => {
                    if shard.content_id == content_id && seen.insert(shard.index) {
                        tracing::info!(index = shard.index, "shard QEL obtido");
                        shards.push(shard);
                    }
                }
                Err(e) => tracing::debug!(error = %e, "ignorar evento shard"),
            }
            if shards.len() >= threshold as usize {
                return Ok(shards);
            }
        }
        if shards.len() >= threshold as usize {
            break;
        }
    }
    Ok(shards)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mycelium_qel::{fragment, QelConfig};

    #[test]
    fn shard_event_roundtrip_plaintext() {
        let ghost = GhostId::spawn_quick(3600).unwrap();
        let shards = fragment(b"hello-qel", "Qmtest", &QelConfig {
            threshold: 2,
            total: 3,
            ttl_secs: 3600,
        })
        .unwrap();
        let ev = create_shard_event(&ghost, &shards[0], None).unwrap();
        assert_eq!(ev.kind, KIND_QEL_SHARD);
        let got = decrypt_shard_content(None, &ev.pubkey, &ev.content).unwrap();
        assert_eq!(got.index, shards[0].index);
        assert_eq!(got.content_id, "Qmtest");
    }

    #[test]
    fn shard_event_nip44_roundtrip() {
        let sender = GhostId::spawn_quick(3600).unwrap();
        let recipient = GhostId::spawn_quick(3600).unwrap();
        let shards = fragment(b"secret-plot", "Qmsec", &QelConfig {
            threshold: 2,
            total: 3,
            ttl_secs: 3600,
        })
        .unwrap();
        let recip_hex = recipient.nostr_pubkey_hex();
        let ev = create_shard_event(&sender, &shards[0], Some(&recip_hex)).unwrap();
        assert!(!ev.content.starts_with('{'));
        let got = decrypt_shard_content(
            Some(&recipient.secret_key_bytes()),
            &ev.pubkey,
            &ev.content,
        )
        .unwrap();
        assert_eq!(got.content_hash, shards[0].content_hash);
    }
}
