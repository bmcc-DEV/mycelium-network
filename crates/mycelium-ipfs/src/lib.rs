//! # mycelium-ipfs
//!
//! Blockstore local (Fase 4 mínima). Chaves = Blake3 / [`ContentId`] Mycelium
//! com prefixo cosmético `Qm` — **não** é CID IPFS/multihash.
//!
//! Sem bitswap de rede neste crate.

use mycelium_core::ContentId;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr;

/// Erros do blockstore.
#[derive(Debug, thiserror::Error)]
pub enum IpfsError {
    #[error("io: {0}")]
    Io(#[from] io::Error),
    #[error("bloco ausente: {0}")]
    NotFound(String),
    #[error("content id: {0}")]
    ContentId(String),
}

/// Blockstore em `{root}/` — um ficheiro por hash Blake3 (hex, sem `Qm`).
pub struct BlockStore {
    root: PathBuf,
}

impl BlockStore {
    /// Abre (ou cria) o store em `home/ipfs-blocks`.
    pub fn open(home: impl AsRef<Path>) -> Result<Self, IpfsError> {
        let root = home.as_ref().join("ipfs-blocks");
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn path_for(&self, id: &ContentId) -> PathBuf {
        self.root.join(hex::encode(id.0))
    }

    /// Grava bytes; devolve o [`ContentId`] (Blake3 do conteúdo).
    pub fn put(&self, bytes: &[u8]) -> Result<ContentId, IpfsError> {
        let id = ContentId::of(bytes);
        let path = self.path_for(&id);
        if !path.exists() {
            fs::write(&path, bytes)?;
        }
        Ok(id)
    }

    /// Grava sob um id explícito (ex. spore print já identificado).
    pub fn put_named(&self, id: &ContentId, bytes: &[u8]) -> Result<(), IpfsError> {
        let path = self.path_for(id);
        fs::write(&path, bytes)?;
        Ok(())
    }

    pub fn get(&self, id: &ContentId) -> Result<Vec<u8>, IpfsError> {
        let path = self.path_for(id);
        fs::read(&path).map_err(|e| {
            if e.kind() == io::ErrorKind::NotFound {
                IpfsError::NotFound(id.to_string())
            } else {
                IpfsError::Io(e)
            }
        })
    }

    pub fn has(&self, id: &ContentId) -> bool {
        self.path_for(id).exists()
    }

    /// Parse helper a partir de string `Qm…` / hex.
    pub fn get_str(&self, id: &str) -> Result<Vec<u8>, IpfsError> {
        let id = ContentId::from_str(id).map_err(IpfsError::ContentId)?;
        self.get(&id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn put_get_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = BlockStore::open(dir.path()).unwrap();
        let id = store.put(b"floresta-hybrid").unwrap();
        assert!(store.has(&id));
        assert_eq!(store.get(&id).unwrap(), b"floresta-hybrid");
        assert_eq!(id, ContentId::of(b"floresta-hybrid"));
    }

    #[test]
    fn hybrid_offline_put_then_get() {
        // Orquestração offline: plot → blockstore → get (sem wss).
        let dir = tempfile::tempdir().unwrap();
        let store = BlockStore::open(dir.path()).unwrap();
        let plot = b"offline-hybrid-spore-print";
        let id = ContentId::of(plot);
        store.put_named(&id, plot).unwrap();
        assert_eq!(store.get(&id).unwrap(), plot);
        assert!(store.has(&id));
    }
}
