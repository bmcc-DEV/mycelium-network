//! # TheField — Colaboração sem plataforma
//!
//! Um **Signal** é uma proposta (review, merge, release) que se propaga
//! pelo micélio. Nós ressoam (votam) e, quando a ressonância atinge o
//! quórum, o Signal dispara — por exemplo, injetando um Vector do Inertia.
//!
//! IDs são content-addressed (`ContentId`) para viajar pelas hifas entre nós.

use mycelium_core::{ContentId, NodeId};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, thiserror::Error)]
pub enum FieldError {
    #[error("signal {0} não encontrado no campo")]
    SignalNotFound(ContentId),
    #[error("signal já disparou; ressonância tardia ignorada")]
    AlreadyFired,
    #[error("falha de serialização: {0}")]
    Codec(#[from] serde_json::Error),
}

/// O que o Signal propõe.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Proposal {
    /// Revisão/merge de um Plot do Giggs.
    MergePlot { plot: ContentId },
    /// Publicação de release.
    Release { version: String },
    /// Disparo de pipeline (vira Vector no Inertia → Chamber/Ion).
    Pipeline {
        name: String,
        plot: ContentId,
        target_ion: String,
    },
}

/// Estado de um Signal no campo.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignalState {
    /// Propagando e coletando ressonância.
    Resonating,
    /// Quórum atingido: o efeito foi disparado.
    Fired,
    /// Evaporou sem atingir quórum.
    Faded,
}

/// Corpo do Signal usado para derivar o `ContentId` (sem resonators).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct SignalSeed {
    origin: NodeId,
    proposal: Proposal,
    quorum: usize,
    nonce: u64,
}

/// Uma proposta viva que se propaga pelo micélio.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Signal {
    pub id: ContentId,
    pub origin: NodeId,
    pub proposal: Proposal,
    /// Quantas ressonâncias são necessárias para disparar.
    pub quorum: usize,
    pub nonce: u64,
    pub state: SignalState,
    resonators: HashSet<NodeId>,
}

impl Signal {
    pub fn resonance(&self) -> usize {
        self.resonators.len()
    }

    pub fn resonators(&self) -> &HashSet<NodeId> {
        &self.resonators
    }
}

/// O campo local: Signals que este nó conhece.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Field {
    signals: HashMap<ContentId, Signal>,
    next_nonce: u64,
}

impl Field {
    pub fn new() -> Self {
        Self::default()
    }

    /// Emite um Signal no campo. O ID é content-addressed do seed.
    pub fn emit(
        &mut self,
        origin: NodeId,
        proposal: Proposal,
        quorum: usize,
    ) -> Result<ContentId, FieldError> {
        let nonce = self.next_nonce;
        self.next_nonce += 1;
        let seed = SignalSeed {
            origin,
            proposal: proposal.clone(),
            quorum: quorum.max(1),
            nonce,
        };
        let id = ContentId::of(&serde_json::to_vec(&seed)?);
        self.signals.insert(
            id,
            Signal {
                id,
                origin,
                proposal,
                quorum: quorum.max(1),
                nonce,
                state: SignalState::Resonating,
                resonators: HashSet::new(),
            },
        );
        Ok(id)
    }

    /// Absorve um Signal propagado por um vizinho (idempotente).
    pub fn absorb(&mut self, signal: Signal) -> ContentId {
        let id = signal.id;
        match self.signals.get_mut(&id) {
            Some(existing) => {
                for r in &signal.resonators {
                    existing.resonators.insert(*r);
                }
                if existing.resonators.len() >= existing.quorum {
                    existing.state = SignalState::Fired;
                } else if signal.state == SignalState::Fired {
                    existing.state = SignalState::Fired;
                }
                // Mantém next_nonce acima de qualquer nonce visto.
                self.next_nonce = self.next_nonce.max(signal.nonce + 1);
            }
            None => {
                self.next_nonce = self.next_nonce.max(signal.nonce + 1);
                self.signals.insert(id, signal);
            }
        }
        id
    }

    /// Um nó ressoa com o Signal. Retorna o estado após a ressonância.
    pub fn resonate(
        &mut self,
        signal_id: &ContentId,
        node: NodeId,
    ) -> Result<SignalState, FieldError> {
        let signal = self
            .signals
            .get_mut(signal_id)
            .ok_or(FieldError::SignalNotFound(*signal_id))?;
        if signal.state == SignalState::Fired {
            return Err(FieldError::AlreadyFired);
        }
        signal.resonators.insert(node);
        if signal.resonators.len() >= signal.quorum {
            signal.state = SignalState::Fired;
        }
        Ok(signal.state)
    }

    /// Aplica uma ressonância remota (não falha se já disparou).
    pub fn absorb_resonance(
        &mut self,
        signal_id: &ContentId,
        node: NodeId,
    ) -> Result<SignalState, FieldError> {
        let signal = self
            .signals
            .get_mut(signal_id)
            .ok_or(FieldError::SignalNotFound(*signal_id))?;
        signal.resonators.insert(node);
        if signal.resonators.len() >= signal.quorum {
            signal.state = SignalState::Fired;
        }
        Ok(signal.state)
    }

    pub fn get(&self, signal_id: &ContentId) -> Option<&Signal> {
        self.signals.get(signal_id)
    }

    /// Signals que já dispararam e aguardam execução (ex.: pelo Inertia).
    pub fn fired(&self) -> impl Iterator<Item = &Signal> {
        self.signals
            .values()
            .filter(|s| s.state == SignalState::Fired)
    }

    pub fn resonating(&self) -> impl Iterator<Item = &Signal> {
        self.signals
            .values()
            .filter(|s| s.state == SignalState::Resonating)
    }

    pub fn len(&self) -> usize {
        self.signals.len()
    }

    pub fn is_empty(&self) -> bool {
        self.signals.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(n: u8) -> NodeId {
        NodeId::derive(&[n])
    }

    fn plot() -> ContentId {
        ContentId::of(b"code")
    }

    #[test]
    fn signal_fires_on_quorum() {
        let mut field = Field::new();
        let id = field
            .emit(
                node(0),
                Proposal::Pipeline {
                    name: "ci".into(),
                    plot: plot(),
                    target_ion: "webapp".into(),
                },
                3,
            )
            .unwrap();

        assert_eq!(
            field.resonate(&id, node(1)).unwrap(),
            SignalState::Resonating
        );
        assert_eq!(
            field.resonate(&id, node(2)).unwrap(),
            SignalState::Resonating
        );
        assert_eq!(field.resonate(&id, node(3)).unwrap(), SignalState::Fired);
        assert_eq!(field.fired().count(), 1);
    }

    #[test]
    fn absorb_merges_resonators_across_nodes() {
        let mut alice = Field::new();
        let id = alice
            .emit(node(0), Proposal::Release { version: "1.0".into() }, 2)
            .unwrap();
        alice.resonate(&id, node(1)).unwrap();

        let mut bob = Field::new();
        bob.absorb(alice.get(&id).unwrap().clone());
        assert_eq!(bob.resonate(&id, node(2)).unwrap(), SignalState::Fired);
    }

    #[test]
    fn duplicate_resonance_does_not_count_twice() {
        let mut field = Field::new();
        let id = field
            .emit(node(0), Proposal::Release { version: "1.0".into() }, 2)
            .unwrap();
        field.resonate(&id, node(1)).unwrap();
        field.resonate(&id, node(1)).unwrap();
        assert_eq!(field.get(&id).unwrap().state, SignalState::Resonating);
    }

    #[test]
    fn late_resonance_after_fire_is_rejected() {
        let mut field = Field::new();
        let id = field
            .emit(
                node(0),
                Proposal::Pipeline {
                    name: "ci".into(),
                    plot: plot(),
                    target_ion: "x".into(),
                },
                1,
            )
            .unwrap();
        field.resonate(&id, node(1)).unwrap();
        assert!(matches!(
            field.resonate(&id, node(2)),
            Err(FieldError::AlreadyFired)
        ));
    }
}
