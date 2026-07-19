//! # Vacuum — Runtime sem daemon, sem registry
//!
//! Um **Void** é a "imagem": manifesto content-addressed de camadas.
//! Uma **Chamber** é a instância; [`ChamberProcess`] a materializa como
//! processo filho com isolamento (bubblewrap por padrão quando disponível)
//! e limites soft de memória/CPU.

mod layers;
mod process;

pub use layers::{LayerArchive, LayerStore};
pub use process::{ChamberProcess, FruitOptions, Isolation};

use mycelium_core::{ContentId, FruitingBody, Nutrient, Resources, Vitality};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, thiserror::Error)]
pub enum VacuumError {
    #[error("camada {0} ausente: nenhum vizinho a serviu")]
    LayerMissing(ContentId),
    #[error("chamber já decomposta")]
    Decomposed,
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("codec: {0}")]
    Codec(#[from] serde_json::Error),
    #[error("spawn: {0}")]
    Spawn(String),
}

/// Manifesto de uma "imagem": lista de camadas content-addressed.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Void {
    pub name: String,
    pub layers: Vec<ContentId>,
    pub entrypoint: String,
}

/// Depósito local de camadas conhecidas (RAM — cache / testes).
#[derive(Debug, Default)]
pub struct LayerPool {
    blobs: HashMap<ContentId, Vec<u8>>,
}

impl LayerPool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn deposit(&mut self, bytes: Vec<u8>) -> ContentId {
        let id = ContentId::of(&bytes);
        self.blobs.insert(id, bytes);
        id
    }

    pub fn get(&self, id: &ContentId) -> Option<&[u8]> {
        self.blobs.get(id).map(|b| b.as_slice())
    }

    pub fn has(&self, id: &ContentId) -> bool {
        self.blobs.contains_key(id)
    }

    pub fn missing<'a>(&self, void: &'a Void) -> Vec<&'a ContentId> {
        void.layers.iter().filter(|l| !self.has(l)).collect()
    }
}

/// Metadados da instância (ainda sem processo).
#[derive(Debug)]
pub struct Chamber {
    pub void: Void,
    pub resources: Resources,
    vitality: Vitality,
}

impl Chamber {
    pub fn suck(void: Void, pool: &LayerPool, resources: Resources) -> Result<Self, VacuumError> {
        if let Some(missing) = pool.missing(&void).first() {
            return Err(VacuumError::LayerMissing(**missing));
        }
        Ok(Self {
            void,
            resources,
            vitality: Vitality::Fruiting,
        })
    }

    pub fn suck_store(
        void: Void,
        store: &LayerStore,
        resources: Resources,
    ) -> Result<Self, VacuumError> {
        if let Some(missing) = store.missing(&void.layers).first() {
            return Err(VacuumError::LayerMissing(**missing));
        }
        Ok(Self {
            void,
            resources,
            vitality: Vitality::Fruiting,
        })
    }

    pub fn hibernate(&mut self) {
        if self.vitality == Vitality::Fruiting {
            self.vitality = Vitality::Dormant;
        }
    }

    pub fn awaken(&mut self) -> Result<(), VacuumError> {
        match self.vitality {
            Vitality::Decomposed => Err(VacuumError::Decomposed),
            _ => {
                self.vitality = Vitality::Fruiting;
                Ok(())
            }
        }
    }
}

impl FruitingBody for Chamber {
    fn kind(&self) -> &'static str {
        "chamber"
    }

    fn vitality(&self) -> Vitality {
        self.vitality
    }

    fn diet(&self) -> Vec<Nutrient> {
        vec![Nutrient::Enzymes, Nutrient::Atp]
    }

    fn decompose(&mut self) {
        self.vitality = Vitality::Decomposed;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn void_with(pool: &mut LayerPool) -> Void {
        let l1 = pool.deposit(b"base layer".to_vec());
        let l2 = pool.deposit(b"app layer".to_vec());
        Void {
            name: "webapp".into(),
            layers: vec![l1, l2],
            entrypoint: "/bin/serve".into(),
        }
    }

    #[test]
    fn chamber_is_born_when_layers_present() {
        let mut pool = LayerPool::new();
        let void = void_with(&mut pool);
        let chamber = Chamber::suck(void, &pool, Resources::default()).unwrap();
        assert_eq!(chamber.vitality(), Vitality::Fruiting);
        assert_eq!(chamber.kind(), "chamber");
    }

    #[test]
    fn missing_layer_blocks_birth() {
        let mut pool = LayerPool::new();
        let mut void = void_with(&mut pool);
        void.layers.push(ContentId::of(b"never sucked"));
        assert!(matches!(
            Chamber::suck(void, &pool, Resources::default()),
            Err(VacuumError::LayerMissing(_))
        ));
    }

    #[test]
    fn sclerotium_cycle_hibernate_awaken_decompose() {
        let mut pool = LayerPool::new();
        let void = void_with(&mut pool);
        let mut chamber = Chamber::suck(void, &pool, Resources::default()).unwrap();

        chamber.hibernate();
        assert_eq!(chamber.vitality(), Vitality::Dormant);

        chamber.awaken().unwrap();
        assert_eq!(chamber.vitality(), Vitality::Fruiting);

        chamber.decompose();
        assert!(chamber.awaken().is_err());
    }
}
