//! Envelope do protocolo Lattice — viaja pelo tópico `mycelium/lattice/v1`.

use giggs::Plot;
use inertia::{Momentum, Vector};
use isotope::Atom;
use mycelium_core::{ContentId, NodeId};
use serde::{Deserialize, Serialize};
use thefield::Signal;

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
}

impl Envelope {
    pub fn encode(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }
}
