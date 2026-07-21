//! Catálogo de relays mesh (zero VPS): só peers com inbound verificado.
//!
//! Gossip: [`RELAY_MESH_TOPIC`]. DHT key: `/mycelium/relays/<PeerId>`.

use libp2p::{Multiaddr, PeerId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Tópico gossipsub para anúncios de relay.
pub const RELAY_MESH_TOPIC: &str = "mycelium/relay-mesh/v1";

/// Prefixo DHT para registos de relay.
pub const RELAY_DHT_PREFIX: &[u8] = b"/mycelium/relays/";

const RELAY_TTL: Duration = Duration::from_secs(180);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RelayAdvertisement {
    pub peer_id: String,
    pub listen_addrs: Vec<String>,
    pub capacity_remaining: usize,
    pub timestamp: u64,
    /// Só true se o operador marcou inbound alcançável.
    pub wan_reachable: bool,
}

#[derive(Clone, Debug, Default)]
pub struct RelayMeshConfig {
    #[allow(dead_code)]
    pub max_relayed_peers: usize,
}

impl RelayMeshConfig {
    pub fn default_capacity() -> usize {
        64
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelayHealth {
    SelfRelay,
    Healthy { count: usize },
    Degraded { count: usize },
    None,
}

impl RelayHealth {
    pub fn label(&self) -> String {
        match self {
            RelayHealth::SelfRelay => "self".into(),
            RelayHealth::Healthy { count } => format!("healthy:{count}"),
            RelayHealth::Degraded { count } => format!("degraded:{count}"),
            RelayHealth::None => "none".into(),
        }
    }
}

#[derive(Debug, Default)]
pub struct RelayMesh {
    is_relay: bool,
    wan_reachable: bool,
    public_addrs: Vec<Multiaddr>,
    known: HashMap<PeerId, (RelayAdvertisement, Instant)>,
    /// Relay escolhido para circuitos (folha).
    active_relay: Option<PeerId>,
    capacity: usize,
}

impl RelayMesh {
    pub fn new(is_relay: bool, wan_reachable: bool) -> Self {
        Self {
            is_relay,
            wan_reachable,
            public_addrs: Vec::new(),
            known: HashMap::new(),
            active_relay: None,
            capacity: RelayMeshConfig::default_capacity(),
        }
    }

    pub fn set_public_addrs(&mut self, addrs: Vec<Multiaddr>) {
        self.public_addrs = addrs;
    }

    pub fn is_relay(&self) -> bool {
        self.is_relay && self.wan_reachable
    }

    pub fn active_relay(&self) -> Option<PeerId> {
        self.active_relay
    }

    pub fn set_active_relay(&mut self, peer: Option<PeerId>) {
        self.active_relay = peer;
    }

    pub fn dht_key_for(peer: &PeerId) -> Vec<u8> {
        let mut k = RELAY_DHT_PREFIX.to_vec();
        k.extend_from_slice(peer.to_string().as_bytes());
        k
    }

    pub fn advertisement(&self, local: &PeerId) -> Option<RelayAdvertisement> {
        if !self.is_relay() {
            return None;
        }
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Some(RelayAdvertisement {
            peer_id: local.to_string(),
            listen_addrs: self.public_addrs.iter().map(|a| a.to_string()).collect(),
            capacity_remaining: self.capacity,
            timestamp: ts,
            wan_reachable: true,
        })
    }

    pub fn ingest(&mut self, adv: RelayAdvertisement) {
        if !adv.wan_reachable {
            return;
        }
        let Ok(peer) = adv.peer_id.parse::<PeerId>() else {
            return;
        };
        if adv.capacity_remaining == 0 {
            self.known.remove(&peer);
            return;
        }
        self.known.insert(peer, (adv, Instant::now()));
    }

    pub fn prune(&mut self) {
        self.known
            .retain(|_, (_, seen)| seen.elapsed() < RELAY_TTL);
    }

    pub fn health(&self) -> RelayHealth {
        let n = self.prune_count();
        if self.is_relay() {
            return RelayHealth::SelfRelay;
        }
        if n >= 2 {
            RelayHealth::Healthy { count: n }
        } else if n == 1 {
            RelayHealth::Degraded { count: n }
        } else {
            RelayHealth::None
        }
    }

    fn prune_count(&self) -> usize {
        self.known
            .values()
            .filter(|(_, seen)| seen.elapsed() < RELAY_TTL)
            .count()
    }

    /// Melhor relay: mais capacidade, visto recentemente.
    pub fn select_best(&self) -> Option<(PeerId, Multiaddr)> {
        let mut best: Option<(PeerId, Multiaddr, usize)> = None;
        for (peer, (adv, seen)) in &self.known {
            if seen.elapsed() >= RELAY_TTL {
                continue;
            }
            let Some(addr_s) = adv.listen_addrs.first() else {
                continue;
            };
            let Ok(addr) = addr_s.parse::<Multiaddr>() else {
                continue;
            };
            let cap = adv.capacity_remaining;
            match &best {
                None => best = Some((*peer, addr, cap)),
                Some((_, _, c)) if cap > *c => best = Some((*peer, addr, cap)),
                _ => {}
            }
        }
        best.map(|(p, a, _)| (p, a))
    }

    /// `/relay_addr/p2p-circuit` (listen) ou com `/p2p/<target>` para dial.
    pub fn circuit_listen(relay_addr: &Multiaddr) -> Multiaddr {
        let mut a = relay_addr.clone();
        a.push(libp2p::multiaddr::Protocol::P2pCircuit);
        a
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unreachable_ads() {
        let mut mesh = RelayMesh::new(false, false);
        mesh.ingest(RelayAdvertisement {
            peer_id: "12D3KooWDummy".into(),
            listen_addrs: vec!["/ip4/1.2.3.4/tcp/4001".into()],
            capacity_remaining: 10,
            timestamp: 0,
            wan_reachable: false,
        });
        assert!(matches!(mesh.health(), RelayHealth::None));
    }
}
