//! # Isotope — Dados sharded por natureza
//!
//! Um **Nucleus** é um shard: guarda um subconjunto do keyspace,
//! determinado pelo hash da chave. Uma consulta é um **Decay**: propaga-se
//! por hifas aos núcleos vizinhos, que respondem; a **fusão** eventual usa
//! um CRDT last-writer-wins por timestamp lógico.
//!
//! Stub coeso: sharding e fusão são in-memory; propagação real por hifas
//! fica para a próxima fase.

use mycelium_core::ContentId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, thiserror::Error)]
pub enum IsotopeError {
    #[error("a chave {0:?} não pertence a este nucleus")]
    WrongNucleus(String),
}

/// Registro versionado (CRDT last-writer-wins).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Atom {
    pub value: Vec<u8>,
    /// Relógio lógico: maior vence na fusão.
    pub clock: u64,
}

/// Um shard do keyspace.
#[derive(Debug)]
pub struct Nucleus {
    /// Índice deste nucleus no anel de shards.
    pub index: u32,
    /// Total de shards no anel.
    pub ring_size: u32,
    atoms: HashMap<String, Atom>,
}

impl Nucleus {
    pub fn new(index: u32, ring_size: u32) -> Self {
        Self {
            index,
            ring_size: ring_size.max(1),
            atoms: HashMap::new(),
        }
    }

    /// Shard "natural" de uma chave no anel.
    pub fn shard_of(key: &str, ring_size: u32) -> u32 {
        let hash = ContentId::of(key.as_bytes());
        u32::from_le_bytes([hash.0[0], hash.0[1], hash.0[2], hash.0[3]]) % ring_size.max(1)
    }

    fn owns(&self, key: &str) -> bool {
        Self::shard_of(key, self.ring_size) == self.index
    }

    /// Escreve um átomo. Rejeita chaves de outros núcleos.
    pub fn write(&mut self, key: &str, value: Vec<u8>, clock: u64) -> Result<(), IsotopeError> {
        if !self.owns(key) {
            return Err(IsotopeError::WrongNucleus(key.to_string()));
        }
        let incoming = Atom { value, clock };
        match self.atoms.get(key) {
            Some(existing) if existing.clock >= incoming.clock => {}
            _ => {
                self.atoms.insert(key.to_string(), incoming);
            }
        }
        Ok(())
    }

    /// Consulta local (a resposta a um Decay que chegou por hifa).
    pub fn decay(&self, key: &str) -> Option<&Atom> {
        self.atoms.get(key)
    }

    /// Fusão eventual: absorve os átomos de uma réplica do mesmo shard.
    /// Last-writer-wins pelo relógio lógico.
    pub fn fuse(&mut self, replica: &Nucleus) {
        for (key, atom) in &replica.atoms {
            match self.atoms.get(key) {
                Some(existing) if existing.clock >= atom.clock => {}
                _ => {
                    self.atoms.insert(key.clone(), atom.clone());
                }
            }
        }
    }

    pub fn len(&self) -> usize {
        self.atoms.len()
    }

    pub fn is_empty(&self) -> bool {
        self.atoms.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Encontra uma chave que caia no shard desejado.
    fn key_for_shard(index: u32, ring: u32) -> String {
        (0..)
            .map(|i| format!("key-{i}"))
            .find(|k| Nucleus::shard_of(k, ring) == index)
            .unwrap()
    }

    #[test]
    fn keys_route_to_their_natural_shard() {
        let ring = 4;
        let key = key_for_shard(2, ring);
        let mut right = Nucleus::new(2, ring);
        let mut wrong = Nucleus::new(0, ring);
        assert!(right.write(&key, b"v".to_vec(), 1).is_ok());
        assert!(matches!(
            wrong.write(&key, b"v".to_vec(), 1),
            Err(IsotopeError::WrongNucleus(_))
        ));
    }

    #[test]
    fn last_writer_wins_on_fuse() {
        let ring = 1;
        let key = key_for_shard(0, ring);

        let mut a = Nucleus::new(0, ring);
        let mut b = Nucleus::new(0, ring);
        a.write(&key, b"old".to_vec(), 1).unwrap();
        b.write(&key, b"new".to_vec(), 2).unwrap();

        a.fuse(&b);
        assert_eq!(a.decay(&key).unwrap().value, b"new");

        // Fusão no sentido contrário não regride o valor.
        b.fuse(&a);
        assert_eq!(b.decay(&key).unwrap().clock, 2);
    }

    #[test]
    fn stale_write_is_ignored() {
        let ring = 1;
        let key = key_for_shard(0, ring);
        let mut n = Nucleus::new(0, ring);
        n.write(&key, b"v2".to_vec(), 2).unwrap();
        n.write(&key, b"v1".to_vec(), 1).unwrap();
        assert_eq!(n.decay(&key).unwrap().value, b"v2");
    }
}
