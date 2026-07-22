//! Transporte libp2p sobre Nostr (Output bruto → upgrade Noise + Yamux no `build`).

use crate::addr::{encode_nostr_multiaddr, is_nostr_multiaddr, parse_nostr_multiaddr};
use crate::connection::{accept_connection, dial_connection, NostrConnection};
use crate::ws::PersistentRelay;
use futures::{
    channel::mpsc,
    future::{ready, Ready},
    prelude::*,
};
use libp2p::core::transport::{DialOpts, ListenerId, TransportError, TransportEvent};
use libp2p::core::muxing::StreamMuxerBox;
use libp2p::core::upgrade::Version;
use libp2p::core::Transport;
use libp2p::identity::Keypair;
use libp2p::multiaddr::Multiaddr;
use libp2p::{noise, yamux, PeerId};
use mycelium_ghostid::GhostId;
use mycelium_nostr::{seal_event, CandidateSession, KIND_QEL_CANDIDATE};
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::pin::Pin;
use std::task::{Context, Poll};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum NostrTransportError {
    #[error("{0}")]
    Msg(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

type IncomingTx = mpsc::UnboundedSender<(NostrConnection, Multiaddr, Multiaddr)>;

struct ListenerState {
    addr: Multiaddr,
    tell_new_addr: bool,
    incoming_rx: mpsc::UnboundedReceiver<(NostrConnection, Multiaddr, Multiaddr)>,
}

/// Transporte Nostr bruto (AsyncRead/AsyncWrite).
pub struct NostrTransport {
    home: PathBuf,
    listeners: HashMap<ListenerId, ListenerState>,
}

impl NostrTransport {
    pub fn new(home: PathBuf) -> Self {
        Self {
            home,
            listeners: HashMap::new(),
        }
    }

    fn load_ghost(&self) -> Result<GhostId, NostrTransportError> {
        let (_, g) = CandidateSession::load_or_create(&self.home)
            .map_err(|e| NostrTransportError::Msg(e.to_string()))?;
        Ok(g)
    }
}

impl Transport for NostrTransport {
    type Output = NostrConnection;
    type Error = NostrTransportError;
    type ListenerUpgrade = Ready<Result<Self::Output, Self::Error>>;
    type Dial = Pin<Box<dyn Future<Output = Result<Self::Output, Self::Error>> + Send>>;

    fn listen_on(
        &mut self,
        id: ListenerId,
        addr: Multiaddr,
    ) -> Result<(), TransportError<Self::Error>> {
        if !is_nostr_multiaddr(&addr) {
            return Err(TransportError::MultiaddrNotSupported(addr));
        }
        let (relay_url, _ghost_in_addr) = parse_nostr_multiaddr(&addr)
            .map_err(|e| TransportError::Other(NostrTransportError::Msg(e.to_string())))?;

        let ghost = self.load_ghost().map_err(TransportError::Other)?;
        let sk = ghost.secret_key_bytes();
        let (tx, rx): (IncomingTx, _) = mpsc::unbounded();
        let relay_url_bg = relay_url.clone();
        let addr_bg = addr.clone();

        tokio::spawn(async move {
            let ghost = match GhostId::from_secret_bytes(sk, 3600) {
                Ok(g) => g,
                Err(e) => {
                    tracing::warn!(error = %e, "ghost restore");
                    return;
                }
            };
            let relay = match PersistentRelay::connect(&relay_url_bg).await {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!(error = %e, "nostr listen connect");
                    return;
                }
            };
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let tags = vec![
                vec!["qel".into(), "candidate-relay".into()],
                vec!["expires".into(), (now + 300).to_string()],
                vec!["qel-backchannel".into(), relay_url_bg.clone()],
                vec!["qel-transports".into(), "nostr-ws".into()],
                vec![
                    "d".into(),
                    format!("listen:{}", ghost.nostr_pubkey_hex()),
                ],
            ];
            let content = json!({
                "type": "candidate-relay",
                "version": 1,
                "ecdh_public": ghost.nostr_pubkey_hex(),
            })
            .to_string();
            if let Ok(ann) = seal_event(&ghost, now, KIND_QEL_CANDIDATE, tags, content) {
                let _ = relay.publish(&ann);
            }

            let filter = json!({
                "kinds": [KIND_QEL_CANDIDATE],
                "#p": [ghost.nostr_pubkey_hex()],
                "limit": 50
            });
            let (_sid, mut rx_ev) = match relay.subscribe(filter).await {
                Ok(x) => x,
                Err(e) => {
                    tracing::warn!(error = %e, "nostr listen subscribe");
                    return;
                }
            };

            while let Some(ev) = rx_ev.recv().await {
                if ev.kind != KIND_QEL_CANDIDATE || ev.pubkey == ghost.nostr_pubkey_hex() {
                    continue;
                }
                let g = match GhostId::from_secret_bytes(sk, 3600) {
                    Ok(g) => g,
                    Err(_) => continue,
                };
                match accept_connection(relay.clone(), g, &ev).await {
                    Ok(conn) => {
                        let send_back =
                            encode_nostr_multiaddr(&relay_url_bg, &ev.pubkey);
                        let _ = tx.unbounded_send((conn, addr_bg.clone(), send_back));
                    }
                    Err(e) => tracing::warn!(error = %e, "accept_connection"),
                }
            }
        });

        self.listeners.insert(
            id,
            ListenerState {
                addr,
                tell_new_addr: true,
                incoming_rx: rx,
            },
        );
        Ok(())
    }

    fn remove_listener(&mut self, id: ListenerId) -> bool {
        self.listeners.remove(&id).is_some()
    }

    fn dial(
        &mut self,
        addr: Multiaddr,
        _opts: DialOpts,
    ) -> Result<Self::Dial, TransportError<Self::Error>> {
        let (relay_url, peer_ghost) = match parse_nostr_multiaddr(&addr) {
            Ok(x) => x,
            Err(_) => return Err(TransportError::MultiaddrNotSupported(addr)),
        };
        let local = self.load_ghost().map_err(TransportError::Other)?;
        Ok(Box::pin(async move {
            dial_connection(&relay_url, local, &peer_ghost)
                .await
                .map_err(NostrTransportError::from)
        }))
    }

    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<TransportEvent<Self::ListenerUpgrade, Self::Error>> {
        let ids: Vec<ListenerId> = self.listeners.keys().copied().collect();
        for id in ids {
            let Some(listener) = self.listeners.get_mut(&id) else {
                continue;
            };
            if listener.tell_new_addr {
                listener.tell_new_addr = false;
                let listen_addr = listener.addr.clone();
                return Poll::Ready(TransportEvent::NewAddress {
                    listener_id: id,
                    listen_addr,
                });
            }
            match listener.incoming_rx.poll_next_unpin(cx) {
                Poll::Ready(Some((conn, local_addr, send_back_addr))) => {
                    return Poll::Ready(TransportEvent::Incoming {
                        listener_id: id,
                        upgrade: ready(Ok(conn)),
                        local_addr,
                        send_back_addr,
                    });
                }
                Poll::Ready(None) => {
                    self.listeners.remove(&id);
                    return Poll::Ready(TransportEvent::ListenerClosed {
                        listener_id: id,
                        reason: Ok(()),
                    });
                }
                Poll::Pending => {}
            }
        }
        Poll::Pending
    }
}

/// Transporte pronto para SwarmBuilder::with_other_transport.
pub fn build(
    keypair: &Keypair,
    home: PathBuf,
) -> Result<
    libp2p::core::transport::Boxed<(PeerId, StreamMuxerBox)>,
    String,
> {
    let noise = noise::Config::new(keypair).map_err(|e| e.to_string())?;
    let transport = NostrTransport::new(home)
        .upgrade(Version::V1Lazy)
        .authenticate(noise)
        .multiplex(yamux::Config::default())
        .map(|(peer, muxer), _| (peer, StreamMuxerBox::new(muxer)))
        .boxed();
    Ok(transport)
}

/// Multiaddr de listen para a sessão CandidateRelay em `home`.
pub fn listen_multiaddr(home: &std::path::Path, relay_url: &str) -> Result<Multiaddr, String> {
    let (_, ghost) =
        CandidateSession::load_or_create(home).map_err(|e| e.to_string())?;
    Ok(encode_nostr_multiaddr(relay_url, &ghost.nostr_pubkey_hex()))
}
