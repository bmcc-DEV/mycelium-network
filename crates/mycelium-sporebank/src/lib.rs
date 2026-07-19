//! # Spore Bank — Seed/DHT de estado
//!
//! O Spore Bank é o "banco de esporos" do micélio: Plots do Giggs e outros
//! spore prints vivem em disco e podem ser anunciados/recuperados via DHT
//! (Kademlia) através das hifas.
//!
//! Prefixo de chave DHT: `spore/<ContentId hex>`.

use giggs::{GiggsError, Mesh, Plot};
use mycelium_core::ContentId;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum SporeBankError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("codec: {0}")]
    Codec(#[from] serde_json::Error),
    #[error(transparent)]
    Giggs(#[from] GiggsError),
    #[error("esporo {0} não encontrado no banco local")]
    Missing(ContentId),
}

/// Prefixo das chaves DHT do Spore Bank.
pub const DHT_KEY_PREFIX: &[u8] = b"spore/";

/// Chave DHT para um ContentId.
pub fn dht_key(id: &ContentId) -> Vec<u8> {
    let mut key = DHT_KEY_PREFIX.to_vec();
    key.extend_from_slice(id.0.as_slice());
    key
}

/// Extrai ContentId de uma chave DHT `spore/...`.
pub fn content_id_from_dht_key(key: &[u8]) -> Option<ContentId> {
    let rest = key.strip_prefix(DHT_KEY_PREFIX)?;
    if rest.len() != 32 {
        return None;
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(rest);
    Some(ContentId(arr))
}

/// Índice persistido do banco.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Index {
    ids: Vec<ContentId>,
}

/// Spore Bank em disco + mirror in-memory ([`Mesh`]).
#[derive(Debug)]
pub struct SporeBank {
    root: PathBuf,
    mesh: Mesh,
    index: Index,
}

impl SporeBank {
    /// Abre (ou cria) o banco em `root/sporebank/`.
    pub fn open(home: impl AsRef<Path>) -> Result<Self, SporeBankError> {
        let root = home.as_ref().join("sporebank");
        std::fs::create_dir_all(root.join("plots"))?;

        let index_path = root.join("index.json");
        let index: Index = if index_path.exists() {
            serde_json::from_slice(&std::fs::read(&index_path)?)?
        } else {
            Index::default()
        };

        let mut mesh = Mesh::new();
        for id in &index.ids {
            let path = plot_path(&root, id);
            if path.exists() {
                let bytes = std::fs::read(&path)?;
                let _ = mesh.absorb(&bytes);
            }
        }

        Ok(Self { root, mesh, index })
    }

    fn save_index(&self) -> Result<(), SporeBankError> {
        let bytes = serde_json::to_vec_pretty(&self.index)?;
        std::fs::write(self.root.join("index.json"), bytes)?;
        Ok(())
    }

    /// Deposita um Plot: grava em disco, atualiza o mesh e devolve o id.
    pub fn deposit(&mut self, plot: Plot) -> Result<ContentId, SporeBankError> {
        let id = self.mesh.sow(plot)?;
        let bytes = self.mesh.spore_print(&id)?;
        std::fs::write(plot_path(&self.root, &id), &bytes)?;
        if !self.index.ids.contains(&id) {
            self.index.ids.push(id);
            self.save_index()?;
        }
        Ok(id)
    }

    /// Absorve bytes de um spore print (vindo de gossip/DHT).
    pub fn absorb(&mut self, bytes: &[u8]) -> Result<ContentId, SporeBankError> {
        let id = self.mesh.absorb(bytes)?;
        std::fs::write(plot_path(&self.root, &id), bytes)?;
        if !self.index.ids.contains(&id) {
            self.index.ids.push(id);
            self.save_index()?;
        }
        Ok(id)
    }

    pub fn recall(&self, id: &ContentId) -> Option<&Plot> {
        self.mesh.get(id)
    }

    pub fn spore_print(&self, id: &ContentId) -> Result<Vec<u8>, SporeBankError> {
        Ok(self.mesh.spore_print(id)?)
    }

    pub fn mesh(&self) -> &Mesh {
        &self.mesh
    }

    pub fn mesh_mut(&mut self) -> &mut Mesh {
        &mut self.mesh
    }

    pub fn ids(&self) -> &[ContentId] {
        &self.index.ids
    }

    pub fn len(&self) -> usize {
        self.index.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.index.ids.is_empty()
    }
}

fn plot_path(root: &Path, id: &ContentId) -> PathBuf {
    root.join("plots").join(format!("{}.json", hex::encode(id.0)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use giggs::Leaf;
    use mycelium_core::NodeId;

    fn tmp() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "sporebank-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn sample_plot(msg: &str) -> Plot {
        Plot {
            author: NodeId::derive(b"author"),
            message: msg.into(),
            parents: vec![],
            leaves: vec![Leaf {
                path: "main.rs".into(),
                content: msg.as_bytes().to_vec(),
            }],
        }
    }

    #[test]
    fn deposit_survives_reopen() {
        let home = tmp();
        let id = {
            let mut bank = SporeBank::open(&home).unwrap();
            bank.deposit(sample_plot("persist")).unwrap()
        };
        let bank = SporeBank::open(&home).unwrap();
        assert_eq!(bank.recall(&id).unwrap().message, "persist");
        assert_eq!(bank.len(), 1);
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn dht_key_roundtrip() {
        let id = ContentId::of(b"spore");
        let key = dht_key(&id);
        assert_eq!(content_id_from_dht_key(&key), Some(id));
    }

    #[test]
    fn absorb_from_spore_print() {
        let home = tmp();
        let mut a = SporeBank::open(home.join("a")).unwrap();
        let mut b = SporeBank::open(home.join("b")).unwrap();
        let id = a.deposit(sample_plot("replicated")).unwrap();
        let bytes = a.spore_print(&id).unwrap();
        assert_eq!(b.absorb(&bytes).unwrap(), id);
        assert_eq!(b.recall(&id).unwrap().message, "replicated");
        std::fs::remove_dir_all(&home).ok();
    }
}
