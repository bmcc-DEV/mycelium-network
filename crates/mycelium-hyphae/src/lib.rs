//! # mycelium-hyphae
//!
//! Hifas são os links vivos do micélio. Não são "conexões TCP": são
//! relacionamentos que fortalecem com o uso e atrofiam sem ele.
//!
//! - **Transporte**: QUIC + TCP/Noise/Yamux
//! - **Descoberta**: Kademlia DHT + mDNS + seed book público (dnsaddr/HTTP)
//! - **Gossip**: tópicos de feromônios e do Lattice
//! - **Spore Bank**: put/get de records no DHT

mod seeds;

pub use seeds::{SeedBook, DEFAULT_BOOTSTRAP_URL};

use futures::StreamExt;
use libp2p::{
    gossipsub, identify, kad,
    kad::{store::RecordStore, Quorum, Record},
    mdns, noise,
    swarm::{
        behaviour::toggle::Toggle,
        NetworkBehaviour, SwarmEvent,
    },
    tcp, yamux, Multiaddr, PeerId, Swarm, SwarmBuilder,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Tópico de gossip por onde os feromônios se espalham.
pub const PHEROMONE_TOPIC: &str = "mycelium/pheromones/v1";
/// Tópico do protocolo Lattice (Plots, Signals, Vectors).
pub const LATTICE_TOPIC: &str = "mycelium/lattice/v1";

/// Erros da camada de hifas.
#[derive(Debug, thiserror::Error)]
pub enum HyphaeError {
    #[error("falha ao germinar o nó: {0}")]
    Germination(String),
    #[error("falha ao publicar no gossip: {0}")]
    Gossip(String),
    #[error("falha no DHT: {0}")]
    Dht(String),
    #[error("multiaddr inválido: {0}")]
    Addr(String),
}

/// Configuração de germinação.
#[derive(Debug, Clone)]
pub struct HyphaeConfig {
    /// Semente ed25519 (32 bytes). Mesma semente ⇒ mesmo PeerId.
    pub seed: Option<[u8; 32]>,
    /// Endereços de escuta. Vazio ⇒ TCP e QUIC em porta efêmera.
    pub listen: Vec<Multiaddr>,
    /// Peers de bootstrap remoto (dial explícito na germinação).
    pub bootstrap: Vec<Multiaddr>,
    /// Dispara `kademlia.bootstrap()` após dialar seeds.
    pub kad_bootstrap: bool,
    /// Descoberta local via mDNS. Desligue para forçar seed book / bootstrap.
    pub enable_mdns: bool,
}

impl Default for HyphaeConfig {
    fn default() -> Self {
        Self {
            seed: None,
            listen: Vec::new(),
            bootstrap: Vec::new(),
            kad_bootstrap: false,
            enable_mdns: true,
        }
    }
}

/// Eventos que a hifa reporta ao organismo (CLI/daemon).
#[derive(Debug)]
pub enum HyphaEvent {
    Rooted { address: Multiaddr },
    NeighborSniffed { peer: PeerId },
    NeighborEvaporated { peer: PeerId },
    Anastomosis { peer: PeerId },
    Atrophy { peer: PeerId },
    /// Mensagem no tópico de feromônios.
    PheromoneReceived { from: Option<PeerId>, data: Vec<u8> },
    /// Mensagem no tópico Lattice.
    LatticeReceived { from: Option<PeerId>, data: Vec<u8> },
    /// Record DHT recuperado (Spore Bank).
    RecordFound { key: Vec<u8>, value: Vec<u8> },
    /// Query DHT terminou sem resultado.
    RecordNotFound { key: Vec<u8> },
}

/// Métricas de um relacionamento vivo com um vizinho.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyphaLink {
    /// Cresce com uso (mensagens, conexões); decai com atrofia.
    pub strength: u32,
    pub connected: bool,
    /// Quantas vezes a hifa atrofiou (conexão fechou).
    pub atrophy_count: u32,
    /// Mensagens gossip trocadas.
    pub messages: u64,
    /// Epoch secs da última atividade.
    pub last_seen_secs: u64,
}

impl Default for HyphaLink {
    fn default() -> Self {
        Self {
            strength: 0,
            connected: false,
            atrophy_count: 0,
            messages: 0,
            last_seen_secs: now_secs(),
        }
    }
}

impl HyphaLink {
    fn touch(&mut self) {
        self.last_seen_secs = now_secs();
    }

    fn strengthen(&mut self, by: u32) {
        self.strength = self.strength.saturating_add(by);
        self.touch();
    }
}

/// Snapshot persistível das métricas de hifas.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HyphaMetrics {
    pub links: HashMap<String, HyphaLink>,
    pub total_anastomoses: u64,
    pub total_atrophies: u64,
    pub messages_in: u64,
    pub messages_out: u64,
}

#[derive(NetworkBehaviour)]
struct SubstrateBehaviour {
    kademlia: kad::Behaviour<kad::store::MemoryStore>,
    gossipsub: gossipsub::Behaviour,
    mdns: Toggle<mdns::tokio::Behaviour>,
    identify: identify::Behaviour,
}

/// Um nó do micélio: swarm libp2p + estado dos links vivos.
pub struct HyphaeNode {
    swarm: Swarm<SubstrateBehaviour>,
    pheromone_topic: gossipsub::IdentTopic,
    lattice_topic: gossipsub::IdentTopic,
    links: HashMap<PeerId, HyphaLink>,
    listen_addrs: Vec<Multiaddr>,
    metrics: HyphaMetrics,
    last_decay: Instant,
}

impl HyphaeNode {
    /// Germina com configuração padrão (seed opcional).
    pub fn germinate(seed: Option<[u8; 32]>) -> Result<Self, HyphaeError> {
        Self::germinate_with(HyphaeConfig {
            seed,
            ..Default::default()
        })
    }

    /// Germina com bootstrap remoto, listen customizado, etc.
    pub fn germinate_with(config: HyphaeConfig) -> Result<Self, HyphaeError> {
        let keypair = match config.seed {
            Some(mut s) => libp2p::identity::Keypair::ed25519_from_bytes(&mut s)
                .map_err(|e| HyphaeError::Germination(e.to_string()))?,
            None => libp2p::identity::Keypair::generate_ed25519(),
        };

        let enable_mdns = config.enable_mdns;
        let mut swarm = SwarmBuilder::with_existing_identity(keypair)
            .with_tokio()
            .with_tcp(
                tcp::Config::default(),
                noise::Config::new,
                yamux::Config::default,
            )
            .map_err(|e| HyphaeError::Germination(e.to_string()))?
            .with_quic()
            .with_dns()
            .map_err(|e| HyphaeError::Germination(e.to_string()))?
            .with_behaviour(move |key| {
                let peer_id = PeerId::from(key.public());

                let mut kademlia = kad::Behaviour::new(
                    peer_id,
                    kad::store::MemoryStore::new(peer_id),
                );
                kademlia.set_mode(Some(kad::Mode::Client));

                let gossip_config = gossipsub::ConfigBuilder::default()
                    .heartbeat_interval(Duration::from_secs(3))
                    .validation_mode(gossipsub::ValidationMode::Strict)
                    .build()
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
                let gossipsub = gossipsub::Behaviour::new(
                    gossipsub::MessageAuthenticity::Signed(key.clone()),
                    gossip_config,
                )
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

                let mdns = if enable_mdns {
                    Toggle::from(Some(mdns::tokio::Behaviour::new(
                        mdns::Config::default(),
                        peer_id,
                    )?))
                } else {
                    tracing::info!("mDNS desligado — discovery só via seeds/bootstrap");
                    Toggle::from(None)
                };

                let identify = identify::Behaviour::new(identify::Config::new(
                    "/mycelium/substrate/0.1.0".into(),
                    key.public(),
                ));

                Ok(SubstrateBehaviour {
                    kademlia,
                    gossipsub,
                    mdns,
                    identify,
                })
            })
            .map_err(|e| HyphaeError::Germination(e.to_string()))?
            .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(120)))
            .build();

        let pheromone_topic = gossipsub::IdentTopic::new(PHEROMONE_TOPIC);
        let lattice_topic = gossipsub::IdentTopic::new(LATTICE_TOPIC);
        swarm
            .behaviour_mut()
            .gossipsub
            .subscribe(&pheromone_topic)
            .map_err(|e| HyphaeError::Germination(e.to_string()))?;
        swarm
            .behaviour_mut()
            .gossipsub
            .subscribe(&lattice_topic)
            .map_err(|e| HyphaeError::Germination(e.to_string()))?;

        let listen = if config.listen.is_empty() {
            vec![
                "/ip4/0.0.0.0/udp/0/quic-v1"
                    .parse()
                    .expect("multiaddr estático"),
                "/ip4/0.0.0.0/tcp/0".parse().expect("multiaddr estático"),
            ]
        } else {
            config.listen
        };

        for addr in listen {
            swarm
                .listen_on(addr)
                .map_err(|e| HyphaeError::Germination(e.to_string()))?;
        }

        let mut node = Self {
            swarm,
            pheromone_topic,
            lattice_topic,
            links: HashMap::new(),
            listen_addrs: Vec::new(),
            metrics: HyphaMetrics::default(),
            last_decay: Instant::now(),
        };

        let do_kad = config.kad_bootstrap && !config.bootstrap.is_empty();
        for addr in config.bootstrap {
            if let Err(e) = node.reach(addr.clone()) {
                tracing::warn!(%addr, "seed dial falhou na germinação: {e}");
            }
        }
        if do_kad {
            let _ = node.kad_bootstrap();
        }

        Ok(node)
    }

    pub fn peer_id(&self) -> PeerId {
        *self.swarm.local_peer_id()
    }

    pub fn links(&self) -> &HashMap<PeerId, HyphaLink> {
        &self.links
    }

    pub fn metrics(&self) -> &HyphaMetrics {
        &self.metrics
    }

    /// Endereços de escuta observados (úteis para bootstrap de outros nós).
    pub fn listen_addrs(&self) -> &[Multiaddr] {
        &self.listen_addrs
    }

    /// Endereços com `/p2p/<PeerId>` embutido — prontos para dial remoto
    /// (loopback, LAN e IPs públicos observados; `0.0.0.0` → `127.0.0.1`).
    pub fn dialable_addrs(&self) -> Vec<Multiaddr> {
        let peer = self.peer_id();
        let mut out = Vec::new();
        for a in &self.listen_addrs {
            let s = a.to_string();
            if s.contains("/ip4/0.0.0.0/") {
                let local = s.replace("/ip4/0.0.0.0/", "/ip4/127.0.0.1/");
                if let Ok(mut addr) = local.parse::<Multiaddr>() {
                    if !addr.to_string().contains("/p2p/") {
                        addr.push(libp2p::multiaddr::Protocol::P2p(peer));
                    }
                    out.push(addr);
                }
                continue;
            }
            let mut addr = a.clone();
            if !addr.to_string().contains("/p2p/") {
                addr.push(libp2p::multiaddr::Protocol::P2p(peer));
            }
            out.push(addr);
        }
        out.sort_by_key(|a| a.to_string());
        out.dedup();
        out
    }

    /// Dispara bootstrap Kademlia (requer pelo menos um peer conhecido).
    pub fn kad_bootstrap(&mut self) -> Result<(), HyphaeError> {
        self.swarm
            .behaviour_mut()
            .kademlia
            .bootstrap()
            .map(|_| ())
            .map_err(|e| HyphaeError::Dht(e.to_string()))
    }

    /// Re-diala um conjunto de seeds (catálogo público / arquivo).
    pub fn reach_seeds(&mut self, seeds: &[Multiaddr]) -> usize {
        let mut ok = 0;
        for addr in seeds {
            match self.reach(addr.clone()) {
                Ok(()) => ok += 1,
                Err(e) => tracing::debug!(%addr, "seed reach: {e}"),
            }
        }
        if ok > 0 {
            let _ = self.kad_bootstrap();
        }
        ok
    }

    pub fn connected_neighbors(&self) -> usize {
        self.links.values().filter(|l| l.connected).count()
    }

    /// Restaura métricas persistidas (força/atrofia) após reboot.
    pub fn restore_metrics(&mut self, metrics: HyphaMetrics) {
        self.metrics = metrics;
        for (peer_str, link) in &self.metrics.links {
            if let Ok(peer) = peer_str.parse::<PeerId>() {
                let mut restored = link.clone();
                restored.connected = false;
                self.links.insert(peer, restored);
            }
        }
    }

    /// Snapshot das métricas com links atuais.
    pub fn snapshot_metrics(&self) -> HyphaMetrics {
        let mut m = self.metrics.clone();
        m.links = self
            .links
            .iter()
            .map(|(p, l)| (p.to_string(), l.clone()))
            .collect();
        m
    }

    /// Publica no tópico de feromônios.
    pub fn secrete(&mut self, data: Vec<u8>) -> Result<bool, HyphaeError> {
        self.publish(self.pheromone_topic.clone(), data)
    }

    /// Publica no tópico Lattice (Plots, Signals, Vectors).
    pub fn broadcast_lattice(&mut self, data: Vec<u8>) -> Result<bool, HyphaeError> {
        self.publish(self.lattice_topic.clone(), data)
    }

    fn publish(
        &mut self,
        topic: gossipsub::IdentTopic,
        data: Vec<u8>,
    ) -> Result<bool, HyphaeError> {
        match self.swarm.behaviour_mut().gossipsub.publish(topic, data) {
            Ok(_) => {
                self.metrics.messages_out += 1;
                Ok(true)
            }
            Err(gossipsub::PublishError::NoPeersSubscribedToTopic) => Ok(false),
            Err(e) => Err(HyphaeError::Gossip(e.to_string())),
        }
    }

    /// Dial explícito e estável (bootstrap remoto).
    pub fn reach(&mut self, addr: Multiaddr) -> Result<(), HyphaeError> {
        // Extrai PeerId se presente e registra no Kademlia.
        if let Some(peer) = peer_from_multiaddr(&addr) {
            self.swarm
                .behaviour_mut()
                .kademlia
                .add_address(&peer, strip_p2p(addr.clone()));
            self.swarm
                .behaviour_mut()
                .gossipsub
                .add_explicit_peer(&peer);
            self.swarm
                .behaviour_mut()
                .kademlia
                .set_mode(Some(kad::Mode::Server));
        }
        self.swarm
            .dial(addr)
            .map_err(|e| HyphaeError::Gossip(e.to_string()))
    }

    /// Deposita um record no DHT (Spore Bank).
    pub fn dht_put(&mut self, key: Vec<u8>, value: Vec<u8>) -> Result<(), HyphaeError> {
        let record = Record {
            key: kad::RecordKey::new(&key),
            value,
            publisher: Some(self.peer_id()),
            expires: None,
        };
        self.swarm
            .behaviour_mut()
            .kademlia
            .put_record(record, Quorum::One)
            .map_err(|e| HyphaeError::Dht(e.to_string()))?;
        Ok(())
    }

    /// Solicita um record do DHT. O resultado chega via [`HyphaEvent::RecordFound`].
    pub fn dht_get(&mut self, key: Vec<u8>) {
        self.swarm
            .behaviour_mut()
            .kademlia
            .get_record(kad::RecordKey::new(&key));
    }

    /// Guarda localmente no store DHT (também usado como cache do Spore Bank).
    pub fn dht_store_local(&mut self, key: Vec<u8>, value: Vec<u8>) -> Result<(), HyphaeError> {
        let record = Record {
            key: kad::RecordKey::new(&key),
            value,
            publisher: Some(self.peer_id()),
            expires: None,
        };
        self.swarm
            .behaviour_mut()
            .kademlia
            .store_mut()
            .put(record)
            .map_err(|e| HyphaeError::Dht(format!("{e:?}")))?;
        Ok(())
    }

    /// Decai força de hifas inativas (chamado periodicamente pelo pulse).
    fn decay_idle_links(&mut self) {
        if self.last_decay.elapsed() < Duration::from_secs(30) {
            return;
        }
        self.last_decay = Instant::now();
        let now = now_secs();
        for link in self.links.values_mut() {
            if !link.connected && now.saturating_sub(link.last_seen_secs) > 60 {
                link.strength = link.strength.saturating_sub(1);
            }
        }
    }

    /// Avança o organismo: processa o próximo evento do swarm.
    pub async fn pulse(&mut self) -> Option<HyphaEvent> {
        self.decay_idle_links();
        loop {
            let event = self.swarm.select_next_some().await;
            match event {
                SwarmEvent::NewListenAddr { address, .. } => {
                    if !self.listen_addrs.contains(&address) {
                        self.listen_addrs.push(address.clone());
                    }
                    return Some(HyphaEvent::Rooted { address });
                }
                SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                    let link = self.links.entry(peer_id).or_default();
                    link.connected = true;
                    link.strengthen(1);
                    self.metrics.total_anastomoses += 1;
                    self.swarm
                        .behaviour_mut()
                        .gossipsub
                        .add_explicit_peer(&peer_id);
                    self.swarm
                        .behaviour_mut()
                        .kademlia
                        .set_mode(Some(kad::Mode::Server));
                    return Some(HyphaEvent::Anastomosis { peer: peer_id });
                }
                SwarmEvent::ConnectionClosed { peer_id, .. } => {
                    if let Some(link) = self.links.get_mut(&peer_id) {
                        link.connected = false;
                        link.atrophy_count += 1;
                        link.touch();
                    }
                    self.metrics.total_atrophies += 1;
                    return Some(HyphaEvent::Atrophy { peer: peer_id });
                }
                SwarmEvent::Behaviour(SubstrateBehaviourEvent::Mdns(
                    mdns::Event::Discovered(list),
                )) => {
                    let mut first = None;
                    for (peer, addr) in list {
                        self.adopt_peer(peer, addr);
                        first.get_or_insert(peer);
                    }
                    if let Some(peer) = first {
                        return Some(HyphaEvent::NeighborSniffed { peer });
                    }
                }
                SwarmEvent::Behaviour(SubstrateBehaviourEvent::Mdns(
                    mdns::Event::Expired(list),
                )) => {
                    let mut first = None;
                    for (peer, _addr) in list {
                        self.swarm
                            .behaviour_mut()
                            .gossipsub
                            .remove_explicit_peer(&peer);
                        first.get_or_insert(peer);
                    }
                    if let Some(peer) = first {
                        return Some(HyphaEvent::NeighborEvaporated { peer });
                    }
                }
                SwarmEvent::Behaviour(SubstrateBehaviourEvent::Identify(
                    identify::Event::Received { peer_id, info, .. },
                )) => {
                    for addr in info.listen_addrs {
                        self.swarm
                            .behaviour_mut()
                            .kademlia
                            .add_address(&peer_id, addr);
                    }
                }
                SwarmEvent::Behaviour(SubstrateBehaviourEvent::Gossipsub(
                    gossipsub::Event::Message { message, .. },
                )) => {
                    self.metrics.messages_in += 1;
                    if let Some(peer) = message.source {
                        let link = self.links.entry(peer).or_default();
                        link.messages += 1;
                        link.strengthen(1);
                    }
                    let topic = message.topic.as_str();
                    if topic == LATTICE_TOPIC {
                        return Some(HyphaEvent::LatticeReceived {
                            from: message.source,
                            data: message.data,
                        });
                    }
                    return Some(HyphaEvent::PheromoneReceived {
                        from: message.source,
                        data: message.data,
                    });
                }
                SwarmEvent::Behaviour(SubstrateBehaviourEvent::Kademlia(
                    kad::Event::OutboundQueryProgressed { result, .. },
                )) => match result {
                    kad::QueryResult::GetRecord(Ok(kad::GetRecordOk::FoundRecord(
                        peer_record,
                    ))) => {
                        return Some(HyphaEvent::RecordFound {
                            key: peer_record.record.key.to_vec(),
                            value: peer_record.record.value,
                        });
                    }
                    kad::QueryResult::GetRecord(Err(e)) => {
                        let key = match &e {
                            kad::GetRecordError::NotFound { key, .. } => key.to_vec(),
                            kad::GetRecordError::QuorumFailed { key, .. } => key.to_vec(),
                            kad::GetRecordError::Timeout { key, .. } => key.to_vec(),
                        };
                        return Some(HyphaEvent::RecordNotFound { key });
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }

    fn adopt_peer(&mut self, peer: PeerId, addr: Multiaddr) {
        self.swarm
            .behaviour_mut()
            .gossipsub
            .add_explicit_peer(&peer);
        self.swarm
            .behaviour_mut()
            .kademlia
            .add_address(&peer, addr.clone());
        self.swarm
            .behaviour_mut()
            .kademlia
            .set_mode(Some(kad::Mode::Server));
        let _ = self.swarm.dial(addr);
        self.links.entry(peer).or_default().strengthen(1);
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn peer_from_multiaddr(addr: &Multiaddr) -> Option<PeerId> {
    addr.iter().find_map(|p| match p {
        libp2p::multiaddr::Protocol::P2p(peer) => Some(peer),
        _ => None,
    })
}

fn strip_p2p(mut addr: Multiaddr) -> Multiaddr {
    // Remove o protocolo P2p final se existir, para add_address.
    if matches!(
        addr.iter().last(),
        Some(libp2p::multiaddr::Protocol::P2p(_))
    ) {
        let _ = addr.pop();
    }
    addr
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn node_germinates_and_roots() {
        let mut node = HyphaeNode::germinate(None).expect("germina");
        let peer = node.peer_id();
        assert!(!peer.to_string().is_empty());

        let deadline = tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if let Some(HyphaEvent::Rooted { address }) = node.pulse().await {
                    break address;
                }
            }
        })
        .await
        .expect("enraíza a tempo");
        assert!(!deadline.to_string().is_empty());
    }

    #[tokio::test]
    async fn deterministic_seed_yields_deterministic_peer_id() {
        let a = HyphaeNode::germinate(Some([7u8; 32])).unwrap();
        let b = HyphaeNode::germinate(Some([7u8; 32])).unwrap();
        assert_eq!(a.peer_id(), b.peer_id());
    }

    #[tokio::test]
    async fn secrete_without_neighbors_is_not_an_error() {
        let mut node = HyphaeNode::germinate(None).unwrap();
        assert_eq!(node.secrete(b"scent".to_vec()).unwrap(), false);
    }

    /// Dois nós: A enraíza, B faz dial explícito no endereço de A → anastomose.
    #[tokio::test]
    async fn two_nodes_anastomose_via_bootstrap_dial() {
        let mut a = HyphaeNode::germinate_with(HyphaeConfig {
            seed: Some([1u8; 32]),
            listen: vec!["/ip4/127.0.0.1/tcp/0".parse().unwrap()],
            bootstrap: vec![],
            ..Default::default()
        })
        .unwrap();

        let a_addr = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if let Some(HyphaEvent::Rooted { address }) = a.pulse().await {
                    if address.to_string().contains("/tcp/") {
                        let mut dialable = address;
                        dialable.push(libp2p::multiaddr::Protocol::P2p(a.peer_id()));
                        break dialable;
                    }
                }
            }
        })
        .await
        .expect("A enraíza");

        let mut b = HyphaeNode::germinate_with(HyphaeConfig {
            seed: Some([2u8; 32]),
            listen: vec!["/ip4/127.0.0.1/tcp/0".parse().unwrap()],
            bootstrap: vec![a_addr],
            ..Default::default()
        })
        .unwrap();

        let anastomosed = tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                tokio::select! {
                    ev = a.pulse() => {
                        if let Some(HyphaEvent::Anastomosis { peer }) = ev {
                            if peer == b.peer_id() {
                                return true;
                            }
                        }
                    }
                    ev = b.pulse() => {
                        if let Some(HyphaEvent::Anastomosis { peer }) = ev {
                            if peer == a.peer_id() {
                                return true;
                            }
                        }
                    }
                }
            }
        })
        .await
        .expect("anastomose a tempo");

        assert!(anastomosed);
        assert!(a.connected_neighbors() >= 1 || b.connected_neighbors() >= 1);
        assert!(a.metrics().total_anastomoses >= 1 || b.metrics().total_anastomoses >= 1);
    }

    #[tokio::test]
    async fn dht_local_store_roundtrip_via_put() {
        let mut node = HyphaeNode::germinate(None).unwrap();
        // Espera enraizar.
        let _ = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if matches!(node.pulse().await, Some(HyphaEvent::Rooted { .. })) {
                    break;
                }
            }
        })
        .await;

        node.dht_store_local(b"spore:test".to_vec(), b"mycelium".to_vec())
            .unwrap();
        // Leitura local do store.
        let key = kad::RecordKey::new(&b"spore:test");
        let got = node
            .swarm
            .behaviour_mut()
            .kademlia
            .store_mut()
            .get(&key)
            .expect("record local");
        assert_eq!(got.value, b"mycelium");
    }
}
