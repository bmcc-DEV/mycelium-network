//! CandidateRelay — quebra o ponto fixo CGNAT sem violar `/esporocarp`.
//!
//! ```text
//! /esporocarp      ⇔ MYCELIUM_REACHABLE   (permanente, proof)
//! /candidate-relay ⇔ Nostr outbound       (temporário, expira, sem proof)
//! ```
//!
//! Kind 39401: anúncio + handshake. Relays públicos = rendezvous (potencial-vetor A).

use crate::nip94::{seal_event, NostrEvent};
use crate::relay_pool::RelayPool;
use crate::NostrError;
use mycelium_ghostid::GhostId;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

/// QEL_PRESENCE — anúncio de presença.
pub const KIND_QEL_PRESENCE: u16 = 39400;
/// QEL_CANDIDATE_RELAY — relay temporário via Nostr.
pub const KIND_QEL_CANDIDATE: u16 = 39401;
/// QEL_BACKCHANNEL — mensagem NIP-44 dirigida (`#p`).
pub const KIND_QEL_BACKCHANNEL: u16 = 39406;

/// TTL da candidatura (5 min).
pub const CANDIDATE_TTL_SECS: u64 = 300;
/// Intervalo base de re-anúncio (ruído branco aplica jitter).
pub const CANDIDATE_INTERVAL_SECS: u64 = 60;
/// TTL da sessão de backchannel (1 h) — ghost estável para `#p`.
pub const SESSION_TTL_SECS: u64 = 3600;

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn extract_tag<'a>(tags: &'a [Vec<String>], name: &str) -> Option<&'a str> {
    tags.iter()
        .find(|t| t.len() >= 2 && t[0] == name)
        .map(|t| t[1].as_str())
}

/// Estado de um peer CandidateRelay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CandidateState {
    Searching,
    Handshaking,
    Connected,
    Expired,
}

/// Peer CGNAT descoberto.
#[derive(Debug, Clone)]
pub struct CandidatePeer {
    pub ghost_id: String,
    pub relay_url: String,
    pub discovered_at: u64,
    pub expires_at: u64,
    pub state: CandidateState,
    pub ecdh_public_hex: String,
    pub shared_secret: Option<[u8; 32]>,
}

/// Motor CandidateRelay (sessão actual).
pub struct CandidateRelay {
    pub ghost: GhostId,
    pub peers: Vec<CandidatePeer>,
    pub backchannel_relay: String,
    pub state: CandidateState,
}

impl CandidateRelay {
    pub fn new(relay_url: &str) -> Result<Self, NostrError> {
        Ok(Self {
            ghost: GhostId::spawn_quick(CANDIDATE_TTL_SECS + 60)?,
            peers: Vec::new(),
            backchannel_relay: relay_url.to_string(),
            state: CandidateState::Searching,
        })
    }

    /// Novo GhostID para o próximo ciclo (ruído / não-linkabilidade).
    pub fn rotate_ghost(&mut self) -> Result<(), NostrError> {
        self.ghost = GhostId::spawn_quick(CANDIDATE_TTL_SECS + 60)?;
        Ok(())
    }

    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }

    pub fn has_peers(&self) -> bool {
        !self.peers.is_empty()
    }

    /// Evento kind 39401 de anúncio.
    pub fn build_announcement(&self) -> Result<NostrEvent, NostrError> {
        let expires = now_secs() + CANDIDATE_TTL_SECS;
        let tags = vec![
            vec!["qel".into(), "candidate-relay".into()],
            vec!["qel-ghost".into(), self.ghost.nostr_pubkey_hex()],
            vec!["expires".into(), expires.to_string()],
            vec!["qel-backchannel".into(), self.backchannel_relay.clone()],
            vec!["qel-transports".into(), "nostr-ws".into()],
            vec![
                "d".into(),
                format!("candidate:{}", self.ghost.nostr_pubkey_hex()),
            ],
        ];
        let content = json!({
            "type": "candidate-relay",
            "version": 1,
            "capacity_bytes": 4096,
            "ecdh_public": self.ghost.nostr_pubkey_hex(),
        })
        .to_string();
        seal_event(
            &self.ghost,
            now_secs(),
            KIND_QEL_CANDIDATE,
            tags,
            content,
        )
    }

    /// Handshake dirigido a um peer (`#p`).
    pub fn build_handshake(&self, peer: &CandidatePeer) -> Result<NostrEvent, NostrError> {
        let tags = vec![
            vec!["qel".into(), "handshake".into()],
            vec!["p".into(), peer.ghost_id.clone()],
            vec![
                "expires".into(),
                (now_secs() + 60).to_string(),
            ],
            vec!["qel-backchannel".into(), self.backchannel_relay.clone()],
            vec![
                "d".into(),
                format!("hs:{}:{}", self.ghost.nostr_pubkey_hex(), peer.ghost_id),
            ],
        ];
        let content = json!({
            "type": "handshake-ack",
            "ecdh_public": self.ghost.nostr_pubkey_hex(),
        })
        .to_string();
        seal_event(
            &self.ghost,
            now_secs(),
            KIND_QEL_CANDIDATE,
            tags,
            content,
        )
    }

    /// Processa anúncio/handshake recebido (ignora self / expirados).
    pub fn process_announcement(&mut self, event: &NostrEvent) -> Option<CandidatePeer> {
        if event.kind != KIND_QEL_CANDIDATE {
            return None;
        }
        if event.pubkey == self.ghost.nostr_pubkey_hex() {
            return None;
        }
        // Verifica Schnorr NIP-01
        let pk = hex::decode(&event.pubkey).ok()?;
        let id = hex::decode(&event.id).ok()?;
        let sig = hex::decode(&event.sig).ok()?;
        if pk.len() != 32 || id.len() != 32 || sig.len() != 64 {
            return None;
        }
        let mut pk_arr = [0u8; 32];
        let mut id_arr = [0u8; 32];
        let mut sig_arr = [0u8; 64];
        pk_arr.copy_from_slice(&pk);
        id_arr.copy_from_slice(&id);
        sig_arr.copy_from_slice(&sig);
        if GhostId::verify_nostr_event(&pk_arr, &id_arr, &sig_arr).is_err() {
            return None;
        }

        let expires = extract_tag(&event.tags, "expires")
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
        if expires <= now_secs() {
            return None;
        }

        let relay_url = extract_tag(&event.tags, "qel-backchannel")
            .unwrap_or(self.backchannel_relay.as_str())
            .to_string();

        let ecdh_public_hex = extract_tag(&event.tags, "qel-ghost")
            .map(|s| s.to_string())
            .or_else(|| {
                serde_json::from_str::<serde_json::Value>(&event.content)
                    .ok()
                    .and_then(|v| {
                        v.get("ecdh_public")
                            .and_then(|x| x.as_str())
                            .map(|s| s.to_string())
                    })
            })
            .unwrap_or_else(|| event.pubkey.clone());

        let shared = derive_session_secret(
            &self.ghost.secret_key_bytes(),
            &self.ghost.nostr_pubkey_hex(),
            &event.pubkey,
        );

        let peer = CandidatePeer {
            ghost_id: event.pubkey.clone(),
            relay_url,
            discovered_at: now_secs(),
            expires_at: expires,
            state: CandidateState::Connected,
            ecdh_public_hex,
            shared_secret: Some(shared),
        };

        if self.peers.iter().any(|p| p.ghost_id == peer.ghost_id) {
            return None;
        }
        self.peers.push(peer.clone());
        self.state = CandidateState::Connected;
        Some(peer)
    }

    pub fn prune_expired(&mut self) {
        let now = now_secs();
        self.peers.retain(|p| p.expires_at > now);
        if self.peers.is_empty() {
            self.state = CandidateState::Searching;
        }
    }
}

/// Segredo de sessão (P0): SHA-256 ordenado — suficiente para binding; ECDH real = P1.
fn derive_session_secret(local_sk: &[u8; 32], local_pk_hex: &str, peer_pk_hex: &str) -> [u8; 32] {
    let (a, b) = if local_pk_hex < peer_pk_hex {
        (local_pk_hex, peer_pk_hex)
    } else {
        (peer_pk_hex, local_pk_hex)
    };
    let mut h = Sha256::new();
    h.update(b"mycelium-candidate-v1");
    h.update(a.as_bytes());
    h.update(b"|");
    h.update(b.as_bytes());
    h.update(local_sk);
    h.finalize().into()
}

/// Relatório de uma ronda CandidateRelay.
#[derive(Debug, Clone)]
pub struct CandidateRoundReport {
    pub published: usize,
    pub discovered: usize,
    pub peer_count: usize,
    pub self_ghost: String,
    pub peers: Vec<String>,
}

/// Uma ronda: rotate → announce → subscribe kind 39401 → process → handshake.
pub async fn run_candidate_round(pool: &RelayPool) -> Result<CandidateRoundReport, NostrError> {
    let relay_url = pool
        .relays()
        .first()
        .cloned()
        .unwrap_or_else(|| "wss://relay.damus.io".into());
    let mut engine = CandidateRelay::new(&relay_url)?;
    engine.rotate_ghost()?;

    let ann = engine.build_announcement()?;
    let published = pool.publish(&ann).await.unwrap_or(0);

    let since = now_secs().saturating_sub(600);
    let filter = json!({
        "kinds": [KIND_QEL_CANDIDATE],
        "since": since,
        "limit": 40
    });
    let events = pool.subscribe(filter).await?;

    let mut discovered = 0usize;
    let mut handshakes = Vec::new();
    for ev in &events {
        if let Some(peer) = engine.process_announcement(ev) {
            discovered += 1;
            if let Ok(hs) = engine.build_handshake(&peer) {
                handshakes.push(hs);
            }
        }
    }
    for hs in handshakes {
        let _ = pool.publish(&hs).await;
    }

    engine.prune_expired();

    Ok(CandidateRoundReport {
        published,
        discovered,
        peer_count: engine.peer_count(),
        self_ghost: engine.ghost.nostr_pubkey_hex(),
        peers: engine.peers.iter().map(|p| p.ghost_id.clone()).collect(),
    })
}

/// Jitter 30..=300s (ruído branco / Reflex).
pub fn candidate_sleep_secs() -> u64 {
    use rand::Rng;
    rand::thread_rng().gen_range(30..=300)
}

// ── Sessão persistente + backchannel ──────────────────────────────────────

/// Sessão CandidateRelay (ghost estável para send/listen).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CandidateSession {
    pub sk_hex: String,
    pub pk_hex: String,
    pub created_at: u64,
    pub ttl_secs: u64,
}

impl CandidateSession {
    pub fn path(home: &std::path::Path) -> std::path::PathBuf {
        home.join("candidate.session")
    }

    pub fn load_or_create(home: &std::path::Path) -> Result<(Self, GhostId), NostrError> {
        let path = Self::path(home);
        if path.exists() {
            let raw = std::fs::read_to_string(&path)
                .map_err(|e| NostrError::Msg(format!("ler sessão: {e}")))?;
            let sess: Self = serde_json::from_str(&raw)?;
            let sk = hex::decode(&sess.sk_hex)
                .map_err(|e| NostrError::InvalidHex(e.to_string()))?;
            if sk.len() != 32 {
                return Err(NostrError::InvalidHex("sk sessão ≠ 32 bytes".into()));
            }
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&sk);
            let age = now_secs().saturating_sub(sess.created_at);
            if age < sess.ttl_secs {
                let ghost = GhostId::from_secret_bytes(arr, sess.ttl_secs.saturating_sub(age))?;
                return Ok((sess, ghost));
            }
        }
        let ghost = GhostId::spawn_quick(SESSION_TTL_SECS)?;
        let sess = Self {
            sk_hex: hex::encode(ghost.secret_key_bytes()),
            pk_hex: ghost.nostr_pubkey_hex(),
            created_at: now_secs(),
            ttl_secs: SESSION_TTL_SECS,
        };
        sess.save(home)?;
        Ok((sess, ghost))
    }

    pub fn save(&self, home: &std::path::Path) -> Result<(), NostrError> {
        std::fs::create_dir_all(home)
            .map_err(|e| NostrError::Msg(format!("mkdir home: {e}")))?;
        let path = Self::path(home);
        let raw = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, raw).map_err(|e| NostrError::Msg(format!("gravar sessão: {e}")))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    pub fn clear(home: &std::path::Path) -> Result<(), NostrError> {
        let path = Self::path(home);
        if path.exists() {
            std::fs::remove_file(&path)
                .map_err(|e| NostrError::Msg(format!("apagar sessão: {e}")))?;
        }
        Ok(())
    }
}

/// Mensagem recebida no backchannel.
#[derive(Debug, Clone)]
pub struct BackchannelMessage {
    pub from: String,
    pub text: String,
    pub event_id: String,
    pub created_at: u64,
}

/// Constrói evento kind 39406 (NIP-44, tag `#p`).
pub fn build_backchannel_event(
    ghost: &GhostId,
    peer_pk_hex: &str,
    text: &str,
) -> Result<NostrEvent, NostrError> {
    let payload = json!({
        "type": "backchannel",
        "version": 1,
        "text": text,
    })
    .to_string();
    let content = crate::shard_event::encrypt_nip44(ghost, peer_pk_hex, &payload)?;
    let nonce: u64 = rand::random();
    let tags = vec![
        vec!["qel".into(), "backchannel".into()],
        vec!["p".into(), peer_pk_hex.to_string()],
        vec![
            "d".into(),
            format!("bc:{}:{nonce}", ghost.nostr_pubkey_hex()),
        ],
    ];
    seal_event(ghost, now_secs(), KIND_QEL_BACKCHANNEL, tags, content)
}

/// Envia texto cifrado a um peer CandidateRelay.
pub async fn send_backchannel(
    pool: &RelayPool,
    ghost: &GhostId,
    peer_pk_hex: &str,
    text: &str,
) -> Result<String, NostrError> {
    if peer_pk_hex.len() != 64 {
        return Err(NostrError::InvalidHex(
            "pubkey peer deve ter 64 hex chars".into(),
        ));
    }
    let ev = build_backchannel_event(ghost, peer_pk_hex, text)?;
    let id = ev.id.clone();
    pool.publish(&ev).await?;
    Ok(id)
}

/// Escuta mensagens `#p` = ghost local (kind 39406).
pub async fn recv_backchannel(
    pool: &RelayPool,
    ghost: &GhostId,
    since_secs_ago: u64,
) -> Result<Vec<BackchannelMessage>, NostrError> {
    let pk = ghost.nostr_pubkey_hex();
    let since = now_secs().saturating_sub(since_secs_ago);
    let filter = json!({
        "kinds": [KIND_QEL_BACKCHANNEL],
        "#p": [pk],
        "since": since,
        "limit": 50
    });
    let events = pool.subscribe(filter).await?;
    let sk = ghost.secret_key_bytes();
    let mut out = Vec::new();
    for ev in events {
        if ev.kind != KIND_QEL_BACKCHANNEL {
            continue;
        }
        let plain =
            match crate::shard_event::decrypt_nip44_to_string(&sk, &ev.pubkey, &ev.content) {
                Ok(p) => p,
                Err(_) => continue,
            };
        let text = serde_json::from_str::<serde_json::Value>(&plain)
            .ok()
            .and_then(|v| {
                v.get("text")
                    .and_then(|t| t.as_str())
                    .map(|s| s.to_string())
            })
            .unwrap_or(plain);
        out.push(BackchannelMessage {
            from: ev.pubkey,
            text,
            event_id: ev.id,
            created_at: ev.created_at,
        });
    }
    out.sort_by_key(|m| m.created_at);
    Ok(out)
}

/// Uma ronda listen: re-anuncia 39401 + recebe 39406.
pub async fn run_listen_round(
    pool: &RelayPool,
    ghost: &GhostId,
) -> Result<(usize, Vec<BackchannelMessage>), NostrError> {
    let relay_url = pool
        .relays()
        .first()
        .cloned()
        .unwrap_or_else(|| "wss://relay.damus.io".into());
    let expires = now_secs() + CANDIDATE_TTL_SECS;
    let tags = vec![
        vec!["qel".into(), "candidate-relay".into()],
        vec!["qel-ghost".into(), ghost.nostr_pubkey_hex()],
        vec!["expires".into(), expires.to_string()],
        vec!["qel-backchannel".into(), relay_url],
        vec!["qel-transports".into(), "nostr-ws".into()],
        vec![
            "d".into(),
            format!("candidate:{}", ghost.nostr_pubkey_hex()),
        ],
    ];
    let content = json!({
        "type": "candidate-relay",
        "version": 1,
        "capacity_bytes": 4096,
        "ecdh_public": ghost.nostr_pubkey_hex(),
    })
    .to_string();
    let ann = seal_event(
        ghost,
        now_secs(),
        KIND_QEL_CANDIDATE,
        tags,
        content,
    )?;
    let published = pool.publish(&ann).await.unwrap_or(0);
    let msgs = recv_backchannel(pool, ghost, 600).await?;
    Ok((published, msgs))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn announcement_is_kind_39401() {
        let cr = CandidateRelay::new("wss://relay.damus.io").unwrap();
        let ev = cr.build_announcement().unwrap();
        assert_eq!(ev.kind, KIND_QEL_CANDIDATE);
        assert!(extract_tag(&ev.tags, "qel") == Some("candidate-relay"));
    }

    #[test]
    fn process_peer_and_ignore_self() {
        let mut a = CandidateRelay::new("wss://relay.damus.io").unwrap();
        let b = CandidateRelay::new("wss://relay.damus.io").unwrap();
        let ann = b.build_announcement().unwrap();
        assert!(a.process_announcement(&ann).is_some());
        assert!(a.has_peers());
        let self_ann = a.build_announcement().unwrap();
        assert!(a.process_announcement(&self_ann).is_none());
    }

    #[test]
    fn prune_clears_expired() {
        let mut a = CandidateRelay::new("wss://relay.damus.io").unwrap();
        let b = CandidateRelay::new("wss://relay.damus.io").unwrap();
        let ann = b.build_announcement().unwrap();
        a.process_announcement(&ann);
        a.peers[0].expires_at = 1;
        a.prune_expired();
        assert!(!a.has_peers());
        assert_eq!(a.state, CandidateState::Searching);
    }

    #[test]
    fn full_cycle_two_leaves() {
        let mut alice = CandidateRelay::new("wss://relay.damus.io").unwrap();
        let mut bob = CandidateRelay::new("wss://relay.damus.io").unwrap();
        let alice_ann = alice.build_announcement().unwrap();
        assert!(bob.process_announcement(&alice_ann).is_some());
        let peer = bob.peers.first().unwrap().clone();
        let hs = bob.build_handshake(&peer).unwrap();
        assert!(alice.process_announcement(&hs).is_some());
        assert_eq!(alice.peer_count(), 1);
        assert_eq!(bob.peer_count(), 1);
    }

    #[test]
    fn backchannel_event_is_kind_39406() {
        let a = GhostId::spawn_quick(3600).unwrap();
        let b = GhostId::spawn_quick(3600).unwrap();
        let bpk = b.nostr_pubkey_hex();
        let ev = build_backchannel_event(&a, &bpk, "ola").unwrap();
        assert_eq!(ev.kind, KIND_QEL_BACKCHANNEL);
        assert_eq!(extract_tag(&ev.tags, "p"), Some(bpk.as_str()));
        assert!(!ev.content.starts_with('{'));
    }

    #[test]
    fn session_roundtrip_tmpdir() {
        let dir = std::env::temp_dir().join(format!("myc-cand-{}", now_secs()));
        let _ = std::fs::remove_dir_all(&dir);
        let (s1, g1) = CandidateSession::load_or_create(&dir).unwrap();
        let (s2, g2) = CandidateSession::load_or_create(&dir).unwrap();
        assert_eq!(s1.pk_hex, s2.pk_hex);
        assert_eq!(g1.nostr_pubkey_hex(), g2.nostr_pubkey_hex());
        CandidateSession::clear(&dir).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
