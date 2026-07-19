//! # Plasma — Orquestração auto-organizada
//!
//! Um **Ion** é uma carga de trabalho viva: uma Chamber do Vacuum ligada
//! a um nome lógico, com carga elétrica (demanda) que atrai ou repele
//! réplicas. O Plasma se auto-organiza — Ions nascem onde há nutrientes
//! e morrem (recombinam) quando a demanda cai.
//!
//! Stub coeso: o "cluster" é in-memory; migração real via hifas fica para
//! a próxima fase.

use mycelium_core::{FruitingBody, NodeId, Nutrient, Vitality};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use vacuum::Chamber;

#[derive(Debug, thiserror::Error)]
pub enum PlasmaError {
    #[error("ion {0} não encontrado no plasma")]
    IonNotFound(String),
    #[error("ion {0} já orbitando")]
    AlreadyOrbiting(String),
    #[error("ion decomposto: não pode reagir")]
    Decomposed,
}

/// Estado de carga de um Ion — quanto "atrai" réplicas.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Charge {
    /// Demanda alta: o Plasma deve brotar réplicas.
    Positive,
    /// Em equilíbrio: uma instância basta.
    Neutral,
    /// Demanda baixa: candidatos a recombinação (morte).
    Negative,
}

/// Uma carga de trabalho auto-organizada.
#[derive(Debug)]
pub struct Ion {
    pub name: String,
    pub host: NodeId,
    pub chamber: Chamber,
    pub charge: Charge,
    /// Réplicas desejadas sob carga positiva.
    pub desired_replicas: u32,
}

impl Ion {
    pub fn birth(name: impl Into<String>, host: NodeId, chamber: Chamber) -> Self {
        Self {
            name: name.into(),
            host,
            chamber,
            charge: Charge::Neutral,
            desired_replicas: 1,
        }
    }

    /// Ajusta a carga conforme a demanda observada (req/s, heurística).
    pub fn sense(&mut self, requests_per_sec: u64) {
        match requests_per_sec {
            0 => {
                self.charge = Charge::Negative;
                self.desired_replicas = 1;
            }
            1..=50 => {
                self.charge = Charge::Neutral;
                self.desired_replicas = 1;
            }
            n => {
                self.charge = Charge::Positive;
                self.desired_replicas = 1 + (n / 50) as u32;
            }
        }
    }
}

impl FruitingBody for Ion {
    fn kind(&self) -> &'static str {
        "ion"
    }

    fn vitality(&self) -> Vitality {
        self.chamber.vitality()
    }

    fn diet(&self) -> Vec<Nutrient> {
        self.chamber.diet()
    }

    fn decompose(&mut self) {
        self.chamber.decompose();
        self.charge = Charge::Negative;
    }
}

/// O Plasma local: o conjunto de Ions que orbitam este nó.
#[derive(Debug, Default)]
pub struct Cloud {
    ions: HashMap<String, Ion>,
}

impl Cloud {
    pub fn new() -> Self {
        Self::default()
    }

    /// Injeta um Ion no Plasma.
    pub fn inject(&mut self, ion: Ion) -> Result<(), PlasmaError> {
        if self.ions.contains_key(&ion.name) {
            return Err(PlasmaError::AlreadyOrbiting(ion.name));
        }
        self.ions.insert(ion.name.clone(), ion);
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<&Ion> {
        self.ions.get(name)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut Ion> {
        self.ions.get_mut(name)
    }

    /// Reage: Ions com carga negativa e sem tráfego são recombinados
    /// (removidos do Plasma, Chamber decomposta).
    pub fn react(&mut self) -> Vec<String> {
        let doomed: Vec<String> = self
            .ions
            .iter()
            .filter(|(_, ion)| {
                ion.charge == Charge::Negative && ion.vitality() != Vitality::Decomposed
            })
            .map(|(name, _)| name.clone())
            .collect();

        for name in &doomed {
            if let Some(mut ion) = self.ions.remove(name) {
                ion.decompose();
            }
        }
        doomed
    }

    /// Ions que pedem réplicas (carga positiva).
    pub fn hungry(&self) -> impl Iterator<Item = &Ion> {
        self.ions
            .values()
            .filter(|i| i.charge == Charge::Positive && i.desired_replicas > 1)
    }

    pub fn len(&self) -> usize {
        self.ions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ions.is_empty()
    }

    pub fn names(&self) -> impl Iterator<Item = &String> {
        self.ions.keys()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mycelium_core::Resources;
    use vacuum::{Chamber, LayerPool, Void};

    fn chamber(name: &str) -> Chamber {
        let mut pool = LayerPool::new();
        let layer = pool.deposit(format!("layer-{name}").into_bytes());
        let void = Void {
            name: name.into(),
            layers: vec![layer],
            entrypoint: "/bin/serve".into(),
        };
        Chamber::suck(void, &pool, Resources::default()).unwrap()
    }

    #[test]
    fn ion_senses_demand_and_asks_for_replicas() {
        let host = NodeId::derive(b"node");
        let mut ion = Ion::birth("webapp", host, chamber("webapp"));
        ion.sense(0);
        assert_eq!(ion.charge, Charge::Negative);

        ion.sense(10);
        assert_eq!(ion.charge, Charge::Neutral);

        ion.sense(200);
        assert_eq!(ion.charge, Charge::Positive);
        assert_eq!(ion.desired_replicas, 5);
    }

    #[test]
    fn cloud_recombines_negative_ions() {
        let host = NodeId::derive(b"node");
        let mut cloud = Cloud::new();
        let mut ion = Ion::birth("idle", host, chamber("idle"));
        ion.sense(0);
        cloud.inject(ion).unwrap();

        let doomed = cloud.react();
        assert_eq!(doomed, vec!["idle".to_string()]);
        assert!(cloud.is_empty());
    }

    #[test]
    fn cannot_inject_duplicate_ion() {
        let host = NodeId::derive(b"node");
        let mut cloud = Cloud::new();
        cloud
            .inject(Ion::birth("webapp", host, chamber("a")))
            .unwrap();
        assert!(matches!(
            cloud.inject(Ion::birth("webapp", host, chamber("b"))),
            Err(PlasmaError::AlreadyOrbiting(_))
        ));
    }

    #[test]
    fn fruiting_body_contract() {
        let ion = Ion::birth("api", NodeId::derive(b"n"), chamber("api"));
        assert_eq!(ion.kind(), "ion");
        assert_eq!(ion.vitality(), Vitality::Fruiting);
        assert!(ion.diet().contains(&Nutrient::Enzymes));
    }
}
