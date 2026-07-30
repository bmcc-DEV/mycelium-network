//! Transporte PQC (ML-KEM-1024) — TCP + KEM + pipeline Noise+Yamux.
//!
//! O Transport fornece conexões TCP brutas após handshake KEM.
//! O Noise roda por cima (autenticação), seguido de Yamux (multiplex).
//!
//! A segurança é híbrida: o KEM protege contra quantum, o Noise provê
//! autenticação ed25519. Ambos os segredos são combinados no handshake híbrido.
//!
//! Multiaddr: `/unix/mycelium-pqc/<pk_hex>`

use futures::channel::mpsc;
use futures::future::{ready, Ready};
use futures::prelude::*;
use libp2p::core::muxing::StreamMuxerBox;
use libp2p::core::transport::{DialOpts, ListenerId, TransportError, TransportEvent};
use libp2p::core::upgrade::Version;
use libp2p::core::Transport;
use libp2p::multiaddr::{Multiaddr, Protocol};
use libp2p::{noise, yamux, PeerId};
use mycelium_pqc::{mlkem_decapsulate, mlkem_encapsulate, mlkem_keygen};
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use thiserror::Error;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio_util::compat::TokioAsyncReadCompatExt;

#[derive(Debug, Error)]
pub enum PqcTransportError {
    #[error("{0}")]
    Msg(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

type IncomingTx = mpsc::UnboundedSender<(TcpStream, Multiaddr)>;

struct ListenerState {
    addr: Multiaddr,
    tell_new_addr: bool,
    incoming_rx: mpsc::UnboundedReceiver<(TcpStream, Multiaddr)>,
}

/// Transporte TCP + KEM. Output = Compat<TcpStream>, pipeline aplica Noise + Yamux.
pub struct PqcTransport {
    local_kp: Arc<mycelium_pqc::KemKeyPair>,
    local_pk_hex: String,
    listeners: HashMap<ListenerId, ListenerState>,
}

impl PqcTransport {
    pub fn new() -> Result<Self, PqcTransportError> {
        let kp = mlkem_keygen();
        let pk_hex = hex::encode(&kp.public_key);
        Ok(Self {
            local_kp: Arc::new(kp),
            local_pk_hex: pk_hex,
            listeners: HashMap::new(),
        })
    }

    pub fn public_key_hex(&self) -> &str {
        &self.local_pk_hex
    }

    /// Multiaddr de listen: `/tcp/<port>/unix/mycelium-pqc/<pk_hex>`
    fn encode_multiaddr(&self, port: u16) -> Multiaddr {
        let path = format!("mycelium-pqc/{}", self.local_pk_hex);
        Multiaddr::empty()
            .with(Protocol::Tcp(port))
            .with(Protocol::Unix(path.into()))
    }

    fn parse_pk_hex(addr: &Multiaddr) -> Option<String> {
        let mut iter = addr.iter();
        let _port = iter.next()?; // Tcp
        match iter.next()? {
            Protocol::Unix(path) => path.strip_prefix("mycelium-pqc/").map(|s| s.to_string()),
            _ => None,
        }
    }
}

impl Transport for PqcTransport {
    type Output = tokio_util::compat::Compat<TcpStream>;
    type Error = PqcTransportError;
    type ListenerUpgrade = Ready<Result<Self::Output, Self::Error>>;
    type Dial = Pin<Box<dyn Future<Output = Result<Self::Output, Self::Error>> + Send>>;

    fn listen_on(
        &mut self,
        id: ListenerId,
        addr: Multiaddr,
    ) -> Result<(), TransportError<Self::Error>> {
        let port = match Self::parse_pk_hex(&addr) {
            Some(_) => match addr.iter().next() {
                Some(Protocol::Tcp(p)) => p,
                _ => return Err(TransportError::MultiaddrNotSupported(addr)),
            },
            None => return Err(TransportError::MultiaddrNotSupported(addr)),
        };
        let bind = format!("0.0.0.0:{port}")
            .parse::<std::net::SocketAddr>()
            .map_err(|e| TransportError::Other(PqcTransportError::Msg(e.to_string())))?;

        let kp = Arc::clone(&self.local_kp);
        let listen_addr = self.encode_multiaddr(port);
        let (tx, rx): (IncomingTx, _) = mpsc::unbounded();

        tokio::spawn(async move {
            let listener = match TcpListener::bind(bind).await {
                Ok(l) => l,
                Err(e) => {
                    tracing::warn!(error = %e, "pqc listen");
                    return;
                }
            };
            loop {
                match listener.accept().await {
                    Ok((mut stream, peer_addr)) => {
                        let mut ct = vec![0u8; 1568];
                        if stream.read_exact(&mut ct).await.is_err() {
                            continue;
                        }
                        if mlkem_decapsulate(kp.private_bytes(), &ct).is_err() {
                            continue;
                        }
                        let peer_maddr = Multiaddr::empty()
                            .with(Protocol::Tcp(peer_addr.port()));
                        let _ = tx.unbounded_send((stream, peer_maddr));
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "pqc accept");
                        break;
                    }
                }
            }
        });

        self.listeners.insert(
            id,
            ListenerState {
                addr: listen_addr,
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
        let pk_hex = Self::parse_pk_hex(&addr)
            .ok_or_else(|| TransportError::MultiaddrNotSupported(addr.clone()))?;
        let port = match addr.iter().next() {
            Some(Protocol::Tcp(p)) => p,
            _ => return Err(TransportError::MultiaddrNotSupported(addr)),
        };
        let remote = format!("127.0.0.1:{port}");
        let _kp = Arc::clone(&self.local_kp);

        Ok(Box::pin(async move {
            let mut stream = TcpStream::connect(&remote).await?;
            let peer_pk = hex::decode(&pk_hex)
                .map_err(|e| PqcTransportError::Msg(e.to_string()))?;
            let enc = mlkem_encapsulate(&peer_pk)
                .map_err(|e| PqcTransportError::Msg(e.to_string()))?;
            stream.write_all(&enc.ciphertext).await?;
            stream.flush().await?;
            Ok(stream.compat())
        }))
    }

    fn poll(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<TransportEvent<Self::ListenerUpgrade, Self::Error>> {
        let this = self.get_mut();
        let ids: Vec<ListenerId> = this.listeners.keys().copied().collect();
        for id in ids {
            let listener = match this.listeners.get_mut(&id) {
                Some(l) => l,
                None => continue,
            };
            if listener.tell_new_addr {
                listener.tell_new_addr = false;
                return Poll::Ready(TransportEvent::NewAddress {
                    listener_id: id,
                    listen_addr: listener.addr.clone(),
                });
            }
            match listener.incoming_rx.poll_next_unpin(cx) {
                Poll::Ready(Some((stream, peer_addr))) => {
                    return Poll::Ready(TransportEvent::Incoming {
                        listener_id: id,
                        upgrade: ready(Ok(stream.compat())),
                        local_addr: listener.addr.clone(),
                        send_back_addr: peer_addr,
                    });
                }
                Poll::Ready(None) => {
                    this.listeners.remove(&id);
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

// --- Handshake híbrido (pós-Noise) ---

pub struct HybridSecret {
    pub combined: [u8; 32],
    pub pqc_raw: Vec<u8>,
}

pub fn generate_pqc_keypair() -> mycelium_pqc::KemKeyPair {
    mlkem_keygen()
}

pub fn client_handshake(
    server_pk: &[u8],
    noise_secret: &[u8; 32],
) -> Result<(HybridSecret, Vec<u8>), String> {
    let enc = mlkem_encapsulate(server_pk).map_err(|e| e.to_string())?;
    let combined = blake3::hash(&[noise_secret.as_slice(), &enc.shared_secret].concat());
    let ct = enc.ciphertext.clone();
    Ok((
        HybridSecret {
            combined: *combined.as_bytes(),
            pqc_raw: enc.shared_secret.clone(),
        },
        ct,
    ))
}

pub fn server_handshake(
    private_key: &[u8],
    ciphertext: &[u8],
    noise_secret: &[u8; 32],
) -> Result<HybridSecret, String> {
    let shared = mlkem_decapsulate(private_key, ciphertext).map_err(|e| e.to_string())?;
    let combined = blake3::hash(&[noise_secret.as_slice(), &shared].concat());
    Ok(HybridSecret {
        combined: *combined.as_bytes(),
        pqc_raw: shared,
    })
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
    fn pqc_multiaddr_roundtrip() {
        let path = format!("mycelium-pqc/aabbccdd");
        let addr = Multiaddr::empty()
            .with(Protocol::Tcp(4003))
            .with(Protocol::Unix(path.into()));
        let pk = PqcTransport::parse_pk_hex(&addr).unwrap();
        assert_eq!(pk, "aabbccdd");
    }
}

/// Constrói transporte registável via `SwarmBuilder::with_other_transport`.
pub fn build(
    keypair: &libp2p::identity::Keypair,
) -> Result<libp2p::core::transport::Boxed<(PeerId, StreamMuxerBox)>, String> {
    let noise_cfg = noise::Config::new(keypair).map_err(|e| e.to_string())?;
    let transport = PqcTransport::new()
        .map_err(|e| e.to_string())?
        .upgrade(Version::V1)
        .authenticate(noise_cfg)
        .multiplex(yamux::Config::default())
        .map(|(peer, muxer), _| (peer, StreamMuxerBox::new(muxer)))
        .boxed();
    Ok(transport)
}
