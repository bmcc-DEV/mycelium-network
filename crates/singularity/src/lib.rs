//! # Singularity — Roteamento por gravidade
//!
//! O **Event Horizon** é a fronteira do micélio: requisições HTTP do mundo
//! externo entram aqui e são proxyadas por **rizomorfos** até o upstream
//! da Chamber com maior gravidade.

mod proxy;

pub use proxy::{serve_horizon, HorizonHandle};

use mycelium_core::NodeId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Debug, thiserror::Error)]
pub enum SingularityError {
    #[error("nenhum ion orbita o host {0}")]
    NoOrbit(String),
    #[error("ion {0} sem upstream")]
    NoUpstream(String),
}

/// Um backend registrado no horizonte: um Ion do Plasma acessível.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Orbit {
    pub ion: String,
    pub node: NodeId,
    /// Capacidade disponível (quanto maior, mais gravidade).
    pub mass: u64,
    /// Latência fisiológica em "biossegundos" (quanto menor, melhor).
    pub resistance: u64,
    /// URL da Chamber (ex.: `http://127.0.0.1:41234`).
    #[serde(default)]
    pub upstream: String,
}

impl Orbit {
    pub fn gravity(&self) -> f64 {
        self.mass as f64 / (1.0 + self.resistance as f64)
    }
}

/// Tabela de roteamento compartilhada com o proxy HTTP.
pub type HorizonTable = Arc<RwLock<EventHorizon>>;

/// A fronteira do micélio: mapeia hosts/ions para órbitas internas.
#[derive(Debug, Default, Clone)]
pub struct EventHorizon {
    /// host lógico → órbitas (ex.: `sporocarp.mycelium/abc123`)
    orbits: HashMap<String, Vec<Orbit>>,
    /// ion name → melhor upstream (atalho para path-based routing)
    by_ion: HashMap<String, Orbit>,
}

impl EventHorizon {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn shared() -> HorizonTable {
        Arc::new(RwLock::new(Self::new()))
    }

    /// Expõe um Ion sob um host externo e indexa por nome do ion.
    pub fn expose(&mut self, host: impl Into<String>, orbit: Orbit) {
        let host = host.into();
        self.by_ion.insert(orbit.ion.clone(), orbit.clone());
        self.orbits.entry(host).or_default().push(orbit);
    }

    pub fn route(&self, host: &str) -> Result<&Orbit, SingularityError> {
        self.orbits
            .get(host)
            .and_then(|orbits| {
                orbits.iter().max_by(|a, b| {
                    a.gravity()
                        .partial_cmp(&b.gravity())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
            })
            .ok_or_else(|| SingularityError::NoOrbit(host.to_string()))
    }

    /// Roteia pelo nome do Ion (path `/webapp/...`).
    pub fn route_ion(&self, ion: &str) -> Result<&Orbit, SingularityError> {
        self.by_ion
            .get(ion)
            .ok_or_else(|| SingularityError::NoOrbit(ion.to_string()))
    }

    pub fn collapse(&mut self, node: &NodeId) {
        for orbits in self.orbits.values_mut() {
            orbits.retain(|o| &o.node != node);
        }
        self.orbits.retain(|_, orbits| !orbits.is_empty());
        self.by_ion.retain(|_, o| &o.node != node);
    }

    pub fn remove_ion(&mut self, ion: &str) {
        self.by_ion.remove(ion);
        for orbits in self.orbits.values_mut() {
            orbits.retain(|o| o.ion != ion);
        }
        self.orbits.retain(|_, orbits| !orbits.is_empty());
    }

    pub fn hosts(&self) -> impl Iterator<Item = &String> {
        self.orbits.keys()
    }

    pub fn ions(&self) -> impl Iterator<Item = &String> {
        self.by_ion.keys()
    }

    pub fn ion_upstreams(&self) -> Vec<(String, String)> {
        self.by_ion
            .iter()
            .map(|(k, v)| (k.clone(), v.upstream.clone()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn orbit(ion: &str, node: u8, mass: u64, resistance: u64) -> Orbit {
        Orbit {
            ion: ion.into(),
            node: NodeId::derive(&[node]),
            mass,
            resistance,
            upstream: format!("http://127.0.0.1:{}", 8000 + node as u16),
        }
    }

    #[test]
    fn heaviest_orbit_wins() {
        let mut horizon = EventHorizon::new();
        horizon.expose("app.mycelium", orbit("webapp", 1, 10, 0));
        horizon.expose("app.mycelium", orbit("webapp", 2, 100, 0));
        assert_eq!(
            horizon.route("app.mycelium").unwrap().node,
            NodeId::derive(&[2])
        );
    }

    #[test]
    fn route_by_ion_name() {
        let mut horizon = EventHorizon::new();
        horizon.expose("h", orbit("api", 1, 10, 0));
        assert_eq!(horizon.route_ion("api").unwrap().upstream, "http://127.0.0.1:8001");
    }

    #[test]
    fn resistance_drags_gravity_down() {
        let mut horizon = EventHorizon::new();
        horizon.expose("app.mycelium", orbit("webapp", 1, 100, 99));
        horizon.expose("app.mycelium", orbit("webapp", 2, 60, 0));
        assert_eq!(
            horizon.route("app.mycelium").unwrap().node,
            NodeId::derive(&[2])
        );
    }

    #[test]
    fn collapsed_node_leaves_the_horizon() {
        let mut horizon = EventHorizon::new();
        horizon.expose("app.mycelium", orbit("webapp", 1, 10, 0));
        horizon.collapse(&NodeId::derive(&[1]));
        assert!(matches!(
            horizon.route("app.mycelium"),
            Err(SingularityError::NoOrbit(_))
        ));
    }
}
