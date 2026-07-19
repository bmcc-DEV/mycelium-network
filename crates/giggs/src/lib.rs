//! # Giggs — Versionamento Mesh
//!
//! Commits são **Plots**: snapshots content-addressed que se replicam por
//! gossip entre nós vizinhos. Não há servidor central; o "repositório" é o
//! conjunto de Plots que o micélio conhece.
//!
//! Este é um stub coeso: armazenamento in-memory, replicação real via
//! hifas fica para a próxima fase.

use mycelium_core::{ContentId, NodeId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, thiserror::Error)]
pub enum GiggsError {
    #[error("plot {0} não encontrado no mesh local")]
    PlotNotFound(ContentId),
    #[error("falha de serialização: {0}")]
    Codec(#[from] serde_json::Error),
}

/// Um arquivo dentro de um Plot.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Leaf {
    pub path: String,
    pub content: Vec<u8>,
}

/// Um commit no mesh: snapshot imutável e content-addressed.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Plot {
    pub author: NodeId,
    pub message: String,
    pub parents: Vec<ContentId>,
    pub leaves: Vec<Leaf>,
}

impl Plot {
    /// Endereço do Plot: hash do conteúdo serializado.
    pub fn id(&self) -> Result<ContentId, GiggsError> {
        Ok(ContentId::of(&serde_json::to_vec(self)?))
    }
}

/// O mesh local: os Plots que este nó conhece.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Mesh {
    plots: HashMap<ContentId, Plot>,
}

impl Mesh {
    pub fn new() -> Self {
        Self::default()
    }

    /// Semeia um Plot no mesh; retorna seu endereço.
    pub fn sow(&mut self, plot: Plot) -> Result<ContentId, GiggsError> {
        let id = plot.id()?;
        self.plots.insert(id, plot);
        Ok(id)
    }

    pub fn get(&self, id: &ContentId) -> Option<&Plot> {
        self.plots.get(id)
    }

    /// Caminha a linhagem de um Plot até as raízes (histórico).
    pub fn lineage(&self, id: &ContentId) -> Result<Vec<ContentId>, GiggsError> {
        let mut out = Vec::new();
        let mut stack = vec![*id];
        while let Some(current) = stack.pop() {
            let plot = self
                .plots
                .get(&current)
                .ok_or(GiggsError::PlotNotFound(current))?;
            out.push(current);
            stack.extend(&plot.parents);
        }
        Ok(out)
    }

    /// Bytes de um Plot prontos para replicação via gossip pelas hifas.
    pub fn spore_print(&self, id: &ContentId) -> Result<Vec<u8>, GiggsError> {
        let plot = self.plots.get(id).ok_or(GiggsError::PlotNotFound(*id))?;
        Ok(serde_json::to_vec(plot)?)
    }

    /// Absorve um Plot replicado por um vizinho.
    pub fn absorb(&mut self, bytes: &[u8]) -> Result<ContentId, GiggsError> {
        let plot: Plot = serde_json::from_slice(bytes)?;
        self.sow(plot)
    }

    pub fn len(&self) -> usize {
        self.plots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.plots.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plot(msg: &str, parents: Vec<ContentId>) -> Plot {
        Plot {
            author: NodeId::derive(b"dev"),
            message: msg.into(),
            parents,
            leaves: vec![Leaf {
                path: "main.rs".into(),
                content: msg.as_bytes().to_vec(),
            }],
        }
    }

    #[test]
    fn plots_are_content_addressed() {
        let a = plot("init", vec![]);
        let b = plot("init", vec![]);
        assert_eq!(a.id().unwrap(), b.id().unwrap());
        assert_ne!(a.id().unwrap(), plot("feat", vec![]).id().unwrap());
    }

    #[test]
    fn lineage_walks_history() {
        let mut mesh = Mesh::new();
        let root = mesh.sow(plot("init", vec![])).unwrap();
        let child = mesh.sow(plot("feat", vec![root])).unwrap();
        let lineage = mesh.lineage(&child).unwrap();
        assert_eq!(lineage, vec![child, root]);
    }

    #[test]
    fn replication_roundtrip() {
        let mut alice = Mesh::new();
        let mut bob = Mesh::new();
        let id = alice.sow(plot("init", vec![])).unwrap();
        let bytes = alice.spore_print(&id).unwrap();
        let absorbed = bob.absorb(&bytes).unwrap();
        assert_eq!(id, absorbed);
        assert_eq!(alice.get(&id), bob.get(&id));
    }
}
