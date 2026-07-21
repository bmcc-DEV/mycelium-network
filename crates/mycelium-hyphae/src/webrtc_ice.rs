//! WebRTC experimental — feature `webrtc` + `libp2p-webrtc` 0.9 (libp2p 0.56).
//!
//! O transporte oficial é **webrtc-direct** (sem API de STUN configurável no crate).
//! A lista [`PUBLIC_STUN_SERVERS`] documenta infra pública partilhável (como DNS) e
//! alimenta o script de diagnóstico; não torna o CPE um esporocarp.

/// STUN públicos partilhados (substituíveis). Não são esporocarp Mycelium.
pub const PUBLIC_STUN_SERVERS: &[&str] = &[
    "stun.l.google.com:19302",
    "stun.cloudflare.com:3478",
    "stun.nextcloud.com:443",
];

#[derive(Clone, Debug)]
pub struct WebrtcIceConfig {
    pub stun_servers: Vec<String>,
    pub listen_port: u16,
}

impl Default for WebrtcIceConfig {
    fn default() -> Self {
        Self {
            stun_servers: PUBLIC_STUN_SERVERS.iter().map(|s| (*s).to_string()).collect(),
            listen_port: 4002,
        }
    }
}

/// `true` se este build inclui transporte WebRTC.
pub fn webrtc_available() -> bool {
    cfg!(feature = "webrtc")
}

/// Multiaddr de escuta webrtc-direct (fingerprint acrescentado pelo listener).
pub fn webrtc_listen_addr(port: u16) -> String {
    format!("/ip4/0.0.0.0/udp/{port}/webrtc-direct")
}

#[cfg(feature = "webrtc")]
pub mod transport {
    use libp2p::identity::Keypair;
    use libp2p_webrtc::tokio::{Certificate, Transport};
    use rand::thread_rng;

    /// Constrói o transporte webrtc-direct (compatível com SwarmBuilder `with_other_transport`).
    pub fn build(keypair: Keypair) -> Result<Transport, String> {
        let cert = Certificate::generate(&mut thread_rng()).map_err(|e| e.to_string())?;
        Ok(Transport::new(keypair, cert))
    }

    pub fn note() -> &'static str {
        "libp2p-webrtc 0.9 webrtc-direct (sem STUN API); STUN list é documentação/diagnose"
    }
}
