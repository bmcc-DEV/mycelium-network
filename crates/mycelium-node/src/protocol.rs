//! Envelope do protocolo Lattice — viaja pelo tópico `mycelium/lattice/v1`.
//!
//! Wire format atual: `{"v":1,"msg":{...}}`. Decodifica também Envelope nu (legado).

use giggs::Plot;
use inertia::{Momentum, Vector};
use isotope::Atom;
use mycelium_core::{ContentId, NodeId};
use serde::{Deserialize, Serialize};
use thefield::Signal;

/// Versão do wire format suportada por este binário.
pub const ENVELOPE_VERSION: u32 = 1;

/// Mensagens que os nós trocam pelas hifas.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Envelope {
    /// Spore print de um Plot do Giggs.
    SporePrint { plot: Plot },
    /// Signal emitido no TheField.
    SignalBroadcast { signal: Signal },
    /// Ressonância de um nó com um Signal.
    Resonance {
        signal_id: ContentId,
        resonator: NodeId,
    },
    /// Vector do Inertia oferecido à rede (CPU ociosa pode executar).
    VectorOffer { vector: Vector },
    /// Resultado de um Vector executado (local ou remoto).
    MomentumReport {
        vector: Vector,
        momentum: Momentum,
        executor: NodeId,
    },
    /// Átomo do Isotope (estado LWW propagado por hifas).
    AtomSync { key: String, atom: Atom },
    /// Anúncio: este nó tem a layer content-addressed.
    LayerOffer { id: ContentId },
    /// Pedido: preciso desta layer (vizinhos com blob respondem via DHT/offer).
    LayerNeed { id: ContentId },
    /// Consulta Isotope: preciso deste átomo (Decay pelas hifas).
    DecayQuery { key: String, asker: NodeId },
    /// Resposta Isotope a um Decay.
    DecayReply { key: String, atom: Atom },
}

/// Frame versionado no fio.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct EnvelopeFrame {
    v: u32,
    msg: Envelope,
}

impl Envelope {
    pub fn encode(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(&EnvelopeFrame {
            v: ENVELOPE_VERSION,
            msg: self.clone(),
        })
    }

    /// Decodifica frame `v:1` ou Envelope nu (legado). Versões futuras → erro.
    pub fn decode(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        if let Ok(frame) = serde_json::from_slice::<EnvelopeFrame>(bytes) {
            if frame.v == 0 || frame.v > ENVELOPE_VERSION {
                return Err(serde::de::Error::custom(format!(
                    "envelope versão {} não suportada (max {ENVELOPE_VERSION})",
                    frame.v
                )));
            }
            return Ok(frame.msg);
        }
        // Legado: Envelope sem wrapper `v`.
        serde_json::from_slice(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_versioned_frame() {
        let env = Envelope::LayerNeed {
            id: ContentId::of(b"layer"),
        };
        let bytes = env.encode().unwrap();
        let s = std::str::from_utf8(&bytes).unwrap();
        assert!(s.contains("\"v\":1"));
        assert!(s.contains("\"msg\""));
        let back = Envelope::decode(&bytes).unwrap();
        match back {
            Envelope::LayerNeed { id } => assert_eq!(id, ContentId::of(b"layer")),
            _ => panic!("tipo errado"),
        }
    }

    #[test]
    fn legacy_bare_envelope_still_decodes() {
        let env = Envelope::LayerOffer {
            id: ContentId::of(b"x"),
        };
        let bare = serde_json::to_vec(&env).unwrap();
        let back = Envelope::decode(&bare).unwrap();
        assert!(matches!(back, Envelope::LayerOffer { .. }));
    }

    #[test]
    fn unknown_version_is_rejected() {
        let env = Envelope::LayerNeed {
            id: ContentId::of(b"layer"),
        };
        let mut frame = EnvelopeFrame {
            v: 99,
            msg: env,
        };
        let raw = serde_json::to_vec(&frame).unwrap();
        assert!(Envelope::decode(&raw).is_err());
        frame.v = 1;
        assert!(Envelope::decode(&serde_json::to_vec(&frame).unwrap()).is_ok());
    }
}
