//! Conexão futures::AsyncRead/AsyncWrite sobre kind 39406 (framing fiável).

use crate::framing::{Frame, ReliableState};
use crate::ws::PersistentRelay;
use bytes::BytesMut;
use futures::{AsyncRead as FAsyncRead, AsyncWrite as FAsyncWrite};
use mycelium_ghostid::GhostId;
use mycelium_nostr::{
    decrypt_nip44_to_string, encrypt_nip44, seal_event, CandidateRelay, CandidateState,
    KIND_QEL_BACKCHANNEL, KIND_QEL_CANDIDATE,
};
use pin_project::pin_project;
use serde_json::json;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt, DuplexStream};
use tokio_util::compat::{Compat, TokioAsyncReadCompatExt};

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Metade local da conexão (futures IO para libp2p).
#[pin_project]
pub struct NostrConnection {
    #[pin]
    inner: Compat<DuplexStream>,
}

impl FAsyncRead for NostrConnection {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        self.project().inner.poll_read(cx, buf)
    }
}

impl FAsyncWrite for NostrConnection {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.project().inner.poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.project().inner.poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.project().inner.poll_close(cx)
    }
}

/// Cria par (local futures-compat para libp2p, remoto tokio para o pump).
pub fn duplex_pair(buffer: usize) -> (NostrConnection, DuplexStream) {
    let (a, b) = tokio::io::duplex(buffer);
    (
        NostrConnection {
            inner: a.compat(),
        },
        b,
    )
}

/// Publica um frame 39406 cifrado NIP-44.
pub fn publish_data_frame(
    relay: &PersistentRelay,
    ghost: &GhostId,
    peer_pk: &str,
    stream_id_hex: &str,
    frame: &Frame,
) -> Result<(), io::Error> {
    let plain = hex::encode(frame.encode());
    let content = encrypt_nip44(ghost, peer_pk, &plain)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
    let tags = vec![
        vec!["qel".into(), "data".into()],
        vec!["p".into(), peer_pk.to_string()],
        vec!["qel-stream".into(), stream_id_hex.to_string()],
        vec!["qel-seq".into(), frame.seq.to_string()],
        vec!["qel-ack".into(), frame.ack.to_string()],
        vec![
            "d".into(),
            format!("data:{}:{}:{}", stream_id_hex, frame.seq, now_secs()),
        ],
    ];
    let ev = seal_event(ghost, now_secs(), KIND_QEL_BACKCHANNEL, tags, content)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
    relay
        .publish(&ev)
        .map_err(|e| io::Error::new(io::ErrorKind::BrokenPipe, e.to_string()))
}

fn parse_incoming_frame(ghost: &GhostId, ev: &mycelium_nostr::NostrEvent) -> Option<Frame> {
    let sk = ghost.secret_key_bytes();
    let plain = decrypt_nip44_to_string(&sk, &ev.pubkey, &ev.content).ok()?;
    let bytes = hex::decode(plain.trim()).ok()?;
    Frame::decode(&bytes)
}

fn shared_stream_id(a: &str, b: &str) -> [u8; 8] {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    if a < b {
        h.update(a.as_bytes());
        h.update(b.as_bytes());
    } else {
        h.update(b.as_bytes());
        h.update(a.as_bytes());
    }
    let dig = h.finalize();
    let mut out = [0u8; 8];
    out.copy_from_slice(&dig[..8]);
    out
}

/// Arranca o pump bidirecional: `pipe` ↔ Nostr 39406.
pub fn spawn_pump(
    relay: PersistentRelay,
    ghost: GhostId,
    peer_pk: String,
    stream_id: [u8; 8],
    mut pipe: DuplexStream,
) {
    let stream_hex = hex::encode(stream_id);
    tokio::spawn(async move {
        let mut reliable = ReliableState::new(stream_id);
        let mut pending_out = BytesMut::new();
        let filter = json!({
            "kinds": [KIND_QEL_BACKCHANNEL],
            "#p": [ghost.nostr_pubkey_hex()],
            "limit": 100
        });
        let (_sub, mut rx) = match relay.subscribe(filter).await {
            Ok(x) => x,
            Err(e) => {
                tracing::warn!(error = %e, "pump subscribe falhou");
                return;
            }
        };

        let mut tick = tokio::time::interval(Duration::from_millis(200));
        let mut read_buf = vec![0u8; 8192];

        loop {
            tokio::select! {
                _ = tick.tick() => {
                    for f in reliable.retransmit_due() {
                        let _ = publish_data_frame(&relay, &ghost, &peer_pk, &stream_hex, &f);
                    }
                    while !pending_out.is_empty() && !reliable.window_full() {
                        let n = pending_out.len().min(crate::framing::MAX_PAYLOAD);
                        let chunk = pending_out.split_to(n).to_vec();
                        for f in reliable.enqueue_send(chunk) {
                            let _ = publish_data_frame(&relay, &ghost, &peer_pk, &stream_hex, &f);
                        }
                    }
                }
                n = pipe.read(&mut read_buf) => {
                    match n {
                        Ok(0) => break,
                        Ok(n) => {
                            pending_out.extend_from_slice(&read_buf[..n]);
                            while !pending_out.is_empty() && !reliable.window_full() {
                                let take = pending_out.len().min(crate::framing::MAX_PAYLOAD);
                                let chunk = pending_out.split_to(take).to_vec();
                                for f in reliable.enqueue_send(chunk) {
                                    let _ = publish_data_frame(&relay, &ghost, &peer_pk, &stream_hex, &f);
                                }
                            }
                        }
                        Err(_) => break,
                    }
                }
                ev = rx.recv() => {
                    match ev {
                        Some(ev) => {
                            // Filtrar stream_id no cliente — relays públicos não indexam #qel-stream
                            let sid_ok = ev.tags.iter().any(|t| {
                                t.len() >= 2 && t[0] == "qel-stream" && t[1] == stream_hex
                            });
                            if !sid_ok {
                                continue;
                            }
                            if let Some(frame) = parse_incoming_frame(&ghost, &ev) {
                                let (delivered, ack) = reliable.on_recv(frame);
                                if !delivered.is_empty() {
                                    if pipe.write_all(&delivered).await.is_err() {
                                        break;
                                    }
                                    let _ = pipe.flush().await;
                                }
                                if let Some(ack) = ack {
                                    let _ = publish_data_frame(&relay, &ghost, &peer_pk, &stream_hex, &ack);
                                }
                            }
                        }
                        None => break,
                    }
                }
            }
        }
    });
}

/// Estabelece conexão Nostr (dialer): handshake 39401 + pump 39406.
pub async fn dial_connection(
    relay_url: &str,
    local: GhostId,
    peer_ghost: &str,
) -> Result<NostrConnection, io::Error> {
    let relay = PersistentRelay::connect(relay_url)
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::ConnectionRefused, e.to_string()))?;

    let filter = json!({
        "kinds": [KIND_QEL_CANDIDATE],
        "#p": [local.nostr_pubkey_hex()],
        "limit": 20
    });
    let _ = relay.subscribe(filter).await;

    let mut engine = CandidateRelay::new(relay_url)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
    engine.ghost = local;

    let expires = now_secs() + 300;
    let tags = vec![
        vec!["qel".into(), "candidate-relay".into()],
        vec!["p".into(), peer_ghost.to_string()],
        vec!["expires".into(), expires.to_string()],
        vec!["qel-backchannel".into(), relay_url.to_string()],
        vec!["qel-transports".into(), "nostr-ws".into()],
        vec![
            "d".into(),
            format!("dial:{}:{}", engine.ghost.nostr_pubkey_hex(), peer_ghost),
        ],
    ];
    let content = json!({
        "type": "candidate-relay",
        "version": 1,
        "ecdh_public": engine.ghost.nostr_pubkey_hex(),
    })
    .to_string();
    let ann = seal_event(
        &engine.ghost,
        now_secs(),
        KIND_QEL_CANDIDATE,
        tags,
        content,
    )
    .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
    relay
        .publish(&ann)
        .map_err(|e| io::Error::new(io::ErrorKind::BrokenPipe, e.to_string()))?;

    let peer = peer_ghost.to_string();
    let local_pk = engine.ghost.nostr_pubkey_hex();
    let hs = relay
        .wait_event(
            |ev| {
                ev.kind == KIND_QEL_CANDIDATE
                    && ev.pubkey == peer
                    && ev
                        .tags
                        .iter()
                        .any(|t| t.len() >= 2 && t[0] == "p" && t[1] == local_pk)
            },
            Duration::from_secs(25),
        )
        .await
        .map_err(|e| {
            tracing::warn!(error = %e, %peer, "nostr dial handshake timeout");
            io::Error::new(io::ErrorKind::TimedOut, e.to_string())
        })?;

    tracing::info!(%peer, "nostr handshake OK — a abrir stream");
    let _ = engine.process_announcement(&hs);
    let stream_id = shared_stream_id(&local_pk, &peer);

    let (conn, pipe) = duplex_pair(64 * 1024);
    spawn_pump(relay, engine.ghost, peer, stream_id, pipe);
    tokio::time::sleep(Duration::from_millis(300)).await;
    Ok(conn)
}

/// Aceita um dial entrante (já temos evento 39401 do peer).
pub async fn accept_connection(
    relay: PersistentRelay,
    local: GhostId,
    peer_ev: &mycelium_nostr::NostrEvent,
) -> Result<NostrConnection, io::Error> {
    let mut engine = CandidateRelay {
        ghost: local,
        peers: Vec::new(),
        backchannel_relay: relay.url().to_string(),
        state: CandidateState::Searching,
    };
    let peer = engine
        .process_announcement(peer_ev)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "announcement inválido"))?;

    let expires = now_secs() + 60;
    let tags = vec![
        vec!["qel".into(), "handshake".into()],
        vec!["p".into(), peer.ghost_id.clone()],
        vec!["expires".into(), expires.to_string()],
        vec!["qel-backchannel".into(), relay.url().to_string()],
        vec![
            "d".into(),
            format!("hs:{}:{}", engine.ghost.nostr_pubkey_hex(), peer.ghost_id),
        ],
    ];
    let content = json!({
        "type": "handshake-ack",
        "ecdh_public": engine.ghost.nostr_pubkey_hex(),
    })
    .to_string();
    let hs = seal_event(
        &engine.ghost,
        now_secs(),
        KIND_QEL_CANDIDATE,
        tags,
        content,
    )
    .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
    relay
        .publish(&hs)
        .map_err(|e| io::Error::new(io::ErrorKind::BrokenPipe, e.to_string()))?;

    let stream_id = shared_stream_id(&engine.ghost.nostr_pubkey_hex(), &peer.ghost_id);
    let (conn, pipe) = duplex_pair(64 * 1024);
    spawn_pump(relay, engine.ghost, peer.ghost_id, stream_id, pipe);
    tokio::time::sleep(Duration::from_millis(300)).await;
    Ok(conn)
}
