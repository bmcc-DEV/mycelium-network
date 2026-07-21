//! # mycelium-nostr
//!
//! Transporte Nostr outbound (`wss://`) — funciona atrás de CGNAT/firewall.
//! Relays públicos são mailbox/discovery; não substituem bitswap de plots grandes.

mod nip94;
mod relay_pool;
mod shard_event;

pub use nip94::{announce_plot, NostrEvent};
pub use relay_pool::{RelayPool, PUBLIC_RELAYS};
pub use shard_event::{
    create_shard_event, decrypt_shard_content, fetch_shards, publish_shards, KIND_QEL_SHARD,
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
