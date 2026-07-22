//! # mycelium-nostr
//!
//! Transporte Nostr outbound (`wss://`) — funciona atrás de CGNAT/firewall.
//! Relays públicos são mailbox/discovery; não substituem bitswap de plots grandes.

mod candidate_relay;
mod nip94;
mod relay_pool;
mod shard_event;

pub use candidate_relay::{
    announce_and_discover_session, announce_session, build_backchannel_event, candidate_sleep_secs,
    discover_relay_pool, filter_fresh_peers, recv_backchannel, run_candidate_round,
    run_listen_round, send_backchannel, BackchannelMessage, CandidatePeer, CandidateRelay,
    CandidateRoundReport, CandidateSession, CandidateState, DiscoveredPeer,
    CANDIDATE_INTERVAL_SECS, CANDIDATE_TTL_SECS, KIND_QEL_BACKCHANNEL, KIND_QEL_CANDIDATE,
    KIND_QEL_PRESENCE, SESSION_TTL_SECS,
};
pub use nip94::{announce_plot, seal_event, NostrEvent};
pub use relay_pool::{RelayPool, PUBLIC_RELAYS};
pub use shard_event::{
    create_shard_event, decrypt_nip44_to_string, decrypt_shard_content, encrypt_nip44,
    fetch_shards, publish_shards, KIND_QEL_SHARD,
};

use thiserror::Error;

/// Erros do transporte Nostr.
#[derive(Debug, Error)]
pub enum NostrError {
    #[error("todos os relays falharam")]
    AllRelaysFailed,
    #[error("hex inválido: {0}")]
    InvalidHex(String),
    #[error("websocket: {0}")]
    WebSocket(String),
    #[error("timeout")]
    Timeout,
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("nip44: {0}")]
    Nip44(String),
    #[error("qel: {0}")]
    Qel(#[from] mycelium_qel::QelError),
    #[error("ghostid: {0}")]
    Ghost(#[from] mycelium_ghostid::GhostError),
    #[error("{0}")]
    Msg(String),
}
