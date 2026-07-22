//! # mycelium-nostr-transport
//!
//! Transporte libp2p sobre CandidateRelay (39401) + backchannel (39406).
//! Multiaddr: `/unix/mycelium-nostr/<relay_hex>/<ghost_hex>` (forma lógica `/nostr/...`).

mod addr;
mod connection;
mod framing;
mod transport;
mod ws;

pub use addr::{encode_nostr_multiaddr, is_nostr_multiaddr, parse_nostr_multiaddr};
pub use framing::{Frame, ReliableState, WINDOW_SIZE};
pub use transport::{build, listen_multiaddr, NostrTransport, NostrTransportError};
pub use connection::NostrConnection;

/// Relay Nostr default para listen/dial.
pub const DEFAULT_NOSTR_RELAY: &str = "wss://nos.lol";
