//! Organismo: o nó vivo — hifas + Spore Bank + Lattice + Chambers + Event Horizon.

use crate::control::{ControlMsg, Request, Response, StatusReport};
use crate::protocol::Envelope;
use crate::store::{IonRecord, NodeStore, OrganismState, StoreError};
use giggs::{Leaf, Plot};
use inertia::{Flywheel, Momentum, Thrust, Vector};
use isotope::{Atom, Nucleus, DEFAULT_RING_SIZE};
use mycelium_core::{ContentId, Membrane, NodeId, Nutrient, Resources};
use mycelium_hyphae::{
    detect_global_ipv6, diagnose_membrane, env_assume_reachable, with_membrane_flag, HyphaEvent,
    HyphaeConfig, HyphaeNode, MailboxMessage, RelayAdvertisement, SeedBook, DEFAULT_DNS_SEED_NAME,
    MAILBOX_DHT_PREFIX, RELAY_DHT_PREFIX,
};
use mycelium_nutrients::Ledger;
use mycelium_pheromones::{Gland, Trail};
use mycelium_sporebank::{
    content_id_from_layer_dht_key, dht_key, layer_dht_key, SporeBank,
};
use mycelium_tropical::{MyceliumPhase, PhysarumNetwork};
use plasma::{Cloud, Ion};
use singularity::{serve_horizon, EventHorizon, HorizonHandle, HorizonTable, Orbit};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use thefield::{Proposal, SignalState};
use tokio::sync::mpsc;
use vacuum::{
    Chamber, ChamberProcess, FruitOptions, Isolation, LayerArchive, LayerStore, Void,
};

#[derive(Debug, thiserror::Error)]
pub enum OrganismError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Spore(#[from] mycelium_sporebank::SporeBankError),
    #[error(transparent)]
    Hyphae(#[from] mycelium_hyphae::HyphaeError),
    #[error(transparent)]
    Field(#[from] thefield::FieldError),
    #[error(transparent)]
    Vacuum(#[from] vacuum::VacuumError),
    #[error(transparent)]
    Nutrient(#[from] mycelium_nutrients::NutrientError),
    #[error("{0}")]
    Msg(String),
}

impl From<String> for OrganismError {
    fn from(s: String) -> Self {
        OrganismError::Msg(s)
    }
}

pub struct OrganismConfig {
    pub home: PathBuf,
    pub contribute: Option<Resources>,
    pub bootstrap: Vec<String>,
    pub horizon_port: u16,
    /// Multiaddrs de escuta (ex.: `/ip4/0.0.0.0/tcp/4001` para ser seed público).
    pub listen: Vec<String>,
    pub seed_file: Option<PathBuf>,
    pub public_bootstrap: bool,
    pub bootstrap_url: Option<String>,
    pub enable_mdns: bool,
    /// IP público anunciado (NAT / seed).
    pub announce_ip: Option<String>,
    /// IPv6 público anunciado (`MYCELIUM_ANNOUNCE_IP6`).
    pub announce_ip6: Option<String>,
    /// Seed opera como circuit relay v2.
    pub enable_relay: bool,
    /// Volunteer Sporocarp: relay + DNS + crédito ATP.
    pub sporocarp: bool,
    /// Override explícito da membrana (`--membrane`).
    pub membrane: Option<Membrane>,
    /// Inbound TCP/QUIC verificado (`--assume-reachable` / `MYCELIUM_REACHABLE`).
    pub assume_reachable: bool,
    /// Escuta webrtc-direct (build com `--features webrtc`).
    pub enable_webrtc: bool,
    pub webrtc_port: u16,
    /// Transporte libp2p sobre Nostr.
    /// `None` = auto (folha/floresta); `Some(true/false)` = forçar.
    pub nostr_transport: Option<bool>,
    pub nostr_relay: Option<String>,
}

pub struct Organism {
    store: NodeStore,
    gland: Gland,
    ledger: Ledger,
    resources: Resources,
    hyphae: HyphaeNode,
    bank: SporeBank,
    state: OrganismState,
    flywheel: Flywheel,
    cloud: Cloud,
    horizon: HorizonTable,
    chambers: HashMap<String, ChamberProcess>,
    mycelium_bin: PathBuf,
    processed: HashSet<ContentId>,
    horizon_handle: Option<HorizonHandle>,
    seed_book: SeedBook,
    nucleus: Nucleus,
    /// Artefato do último Build bem-sucedido (por plot).
    build_artifacts: HashMap<ContentId, LayerArchive>,
    /// Vectors remotos já aceitos (evita re-execução).
    remote_done: HashSet<String>,
    /// Decays em curso (miss local → DecayQuery broadcast).
    pending_decays: HashSet<String>,
    sporocarp: bool,
    membrane: Membrane,
    dns_seed: Option<String>,
    /// Operador afirmou inbound alcançável (WAN relayável).
    assume_reachable: bool,
    /// Rede Physarum (tick periódico no loop RSA leve).
    physarum: PhysarumNetwork,
    physarum_phase: MyceliumPhase,
    enable_nostr_transport: bool,
    nostr_relay: String,
    #[cfg(feature = "nostr-transport")]
    nostr_dialed: HashMap<String, std::time::Instant>,
    vault: entropy::Vault,
    remote_ledger: HashMap<NodeId, (HashMap<Nutrient, u64>, u64)>,
    known_zones: HashMap<String, Vec<NodeId>>,
    ion_hosts: HashMap<String, String>,
    catalog: std::sync::Arc<std::sync::Mutex<mycelium_store::StoreCatalog>>,
    home: PathBuf,
    /// Identidade de assinatura (GhostID/NIP-01) para a Micelial Value Layer.
    ghost: mycelium_ghostid::GhostId,
    /// Registro de ativos RWA / empresas (Fase 3/4).
    assets: crate::assets::AssetRegistry,
    /// Nonce de transferência emitida (anti-replay).
    transfer_nonce: u64,
}

impl Organism {
    pub fn awaken(config: OrganismConfig) -> Result<Self, OrganismError> {
        let store = NodeStore::open(&config.home)?;
        let gland = store.load_or_create_gland()?;
        let mut ledger = store.load_ledger();
        let resources = if let Some(r) = config.contribute {
            store.save_resources(&r)?;
            r
        } else {
            store
                .load_resources()
                .unwrap_or_else(|| Resources::from_str("1cpu,1gb,10gb").unwrap())
        };
        if ledger.history().is_empty() {
            ledger.pledge(&resources);
            store.save_ledger(&ledger)?;
        }

        let mut state = store.load_state();
        for addr in &config.bootstrap {
            if !state.bootstrap.contains(addr) {
                state.bootstrap.push(addr.clone());
            }
        }
        if config.horizon_port != 0 {
            state.horizon_port = config.horizon_port;
        }

        let seed_book = SeedBook::assemble(
            &config.home,
            &config.bootstrap,
            config.seed_file.as_deref(),
            config.public_bootstrap,
            config.bootstrap_url.as_deref(),
        )
        .map_err(|e| OrganismError::Msg(e.to_string()))?;
        // Persiste seeds descobertos/passados.
        for s in seed_book.as_strings() {
            if !state.bootstrap.contains(&s) {
                state.bootstrap.push(s);
            }
        }
        let _ = seed_book.save_file(config.home.join("seeds.txt"));

        let listen: Vec<_> = config
            .listen
            .iter()
            .filter_map(|s| s.parse().ok())
            .collect();

        let announce_ip = config
            .announce_ip
            .or_else(|| std::env::var("MYCELIUM_ANNOUNCE_IP").ok());
        let announce_ip6 = config
            .announce_ip6
            .or_else(|| std::env::var("MYCELIUM_ANNOUNCE_IP6").ok());
        let has_global_ip6 = announce_ip6.is_some() || detect_global_ipv6();
        let assume_reachable = config.assume_reachable || env_assume_reachable();
        let membrane = diagnose_membrane(
            has_global_ip6,
            announce_ip.as_deref(),
            config.sporocarp,
            config.membrane,
            assume_reachable,
        );
        if matches!(membrane, Membrane::Esporocarp) && !assume_reachable && config.sporocarp {
            tracing::warn!(
                "esporocarp sem MYCELIUM_REACHABLE/--assume-reachable — IPv6 global \
                 NÃO prova inbound (ex.: firewall Vivo). TXT /esporocarp pode anunciar \
                 um nó inacessível. Confirme com: nc -vz <ip6> 4001 de fora da LAN."
            );
        }
        // Relay server só em esporocarp (ou --relay explícito legado).
        let enable_relay_server =
            config.enable_relay || matches!(membrane, Membrane::Esporocarp);
        let enable_relay_client = !enable_relay_server;

        let bootstrap_addrs = seed_book.multiaddrs_for(membrane);
        let dns_seed = std::env::var("MYCELIUM_DNS_SEEDS")
            .ok()
            .or_else(|| {
                if config.public_bootstrap || config.sporocarp {
                    Some(DEFAULT_DNS_SEED_NAME.to_string())
                } else {
                    None
                }
            });
        tracing::info!(
            %membrane,
            has_global_ip6,
            assume_reachable,
            announce_ip = announce_ip.as_deref().unwrap_or("-"),
            "membrana diagnosticada"
        );
        // Folha/Floresta: Nostr transport por default (CGNAT / IPv6 sem inbound).
        // Raiz/Esporocarp: opt-in. `--no-nostr-transport` / `Some(false)` desliga.
        #[cfg(feature = "nostr-transport")]
        let enable_nostr_transport = match config.nostr_transport {
            Some(v) => v,
            None => matches!(membrane, Membrane::Folha | Membrane::Floresta),
        };
        #[cfg(not(feature = "nostr-transport"))]
        let enable_nostr_transport = false;
        if enable_nostr_transport {
            tracing::info!(%membrane, "nostr-transport activo (folha/floresta auto ou --nostr-transport)");
        }
        let mut hyphae = HyphaeNode::germinate_with(HyphaeConfig {
            seed: Some(gland.seed()),
            listen,
            bootstrap: bootstrap_addrs,
            kad_bootstrap: !seed_book.is_empty(),
            enable_mdns: config.enable_mdns,
            announce_ip,
            announce_ip6,
            enable_relay_server,
            enable_relay_client,
            membrane,
            assume_reachable,
            enable_webrtc: config.enable_webrtc,
            webrtc_port: config.webrtc_port,
            enable_nostr_transport,
            nostr_home: Some(config.home.clone()),
            nostr_relay: config.nostr_relay.clone(),
        })?;
        hyphae.restore_metrics(state.hypha_metrics.clone());

        let bank = SporeBank::open(&config.home)?;
        let mut processed = HashSet::new();
        for s in &state.processed_signals {
            if let Ok(id) = s.parse::<ContentId>() {
                processed.insert(id);
            }
        }

        let mycelium_bin = std::env::current_exe().map_err(|e| OrganismError::Msg(e.to_string()))?;
        let horizon = EventHorizon::shared();
        {
            let mut h = horizon.write().unwrap();
            h.set_home(config.home.clone());
        }
        let records = state.ions.clone();
        let mut nucleus = store
            .load_nucleus()
            .unwrap_or_else(|| Nucleus::for_node(&gland.node_id(), DEFAULT_RING_SIZE));
        let before = (nucleus.index, nucleus.ring_size);
        nucleus = nucleus.migrate_to_ring(&gland.node_id(), DEFAULT_RING_SIZE);
        if (nucleus.index, nucleus.ring_size) != before {
            tracing::info!(
                shard = nucleus.index,
                ring = nucleus.ring_size,
                "isotope nucleus migrado para anel padrão"
            );
            store.save_nucleus(&nucleus)?;
        }
        let vault = store.load_vault();
        let catalog = mycelium_store::StoreCatalog::open(&config.home)
            .map_err(|e| OrganismError::Msg(e.to_string()))?;
        let assets = crate::assets::AssetRegistry::open(&config.home)
            .map_err(|e| OrganismError::Msg(e.to_string()))?;
        let ghost = ghost_for_node(gland.seed());
        let mut org = Self {
            store,
            gland,
            ledger,
            resources,
            hyphae,
            bank,
            state,
            flywheel: Flywheel::new(),
            cloud: Cloud::new(),
            horizon,
            chambers: HashMap::new(),
            mycelium_bin,
            processed,
            horizon_handle: None,
            seed_book,
            nucleus,
            build_artifacts: HashMap::new(),
            remote_done: HashSet::new(),
            pending_decays: HashSet::new(),
            sporocarp: config.sporocarp || matches!(membrane, Membrane::Esporocarp),
            membrane,
            dns_seed,
            assume_reachable,
            physarum: PhysarumNetwork::new(4, 0.1, 0.01),
            physarum_phase: MyceliumPhase::Exploratory,
            enable_nostr_transport,
            nostr_relay: config
                .nostr_relay
                .unwrap_or_else(|| "wss://nos.lol".into()),
            #[cfg(feature = "nostr-transport")]
            nostr_dialed: HashMap::new(),
            vault,
            remote_ledger: HashMap::new(),
            known_zones: HashMap::new(),
            ion_hosts: HashMap::new(),
            catalog: std::sync::Arc::new(std::sync::Mutex::new(catalog)),
            home: config.home.clone(),
            ghost,
            assets,
            transfer_nonce: 0,
        };

        for rec in records {
            if let Err(e) = org.fruit_ion(&rec.name, &rec.plot, &rec.pipeline, false) {
                tracing::warn!(ion = %rec.name, "falha ao re-frutificar: {e}");
            }
        }
        Ok(org)
    }

    pub fn node_id(&self) -> mycelium_core::NodeId {
        self.gland.node_id()
    }

    pub fn home(&self) -> &Path {
        &self.store.root
    }

    pub fn persist(&mut self) -> Result<(), OrganismError> {
        self.state.hypha_metrics = self.hyphae.snapshot_metrics();
        self.state.processed_signals = self.processed.iter().map(|id| id.to_string()).collect();
        self.store.save_state(&self.state)?;
        self.store.save_ledger(&self.ledger)?;
        self.store.save_nucleus(&self.nucleus)?;
        self.store.save_vault(&self.vault)?;
        let addrs: Vec<String> = self
            .hyphae
            .dialable_addrs()
            .iter()
            .map(|a| a.to_string())
            .collect();
        if !addrs.is_empty() {
            self.store.save_listen_addrs(&addrs)?;
        }
        Ok(())
    }

    fn status_report(&self) -> StatusReport {
        let m = self.hyphae.metrics();
        let ion_names: Vec<String> = self.state.ions.iter().map(|i| i.name.clone()).collect();
        let endpoints: Vec<String> = self
            .chambers
            .iter()
            .map(|(name, c)| {
                format!(
                    "{name} → {} (pid {:?}, {:?})",
                    c.upstream,
                    c.pid(),
                    c.isolation
                )
            })
            .collect();
        let horizon_url = format!("http://127.0.0.1:{}", self.state.horizon_port);
        StatusReport {
            node_id: self.gland.node_id().to_string(),
            peer_id: self.hyphae.peer_id().to_string(),
            listen_addrs: self
                .hyphae
                .dialable_addrs()
                .iter()
                .map(|a| a.to_string())
                .collect(),
            neighbors: self.hyphae.connected_neighbors(),
            plots: self.bank.len(),
            signals: self.state.field.len(),
            ions: ion_names,
            atp: self.ledger.balance(Nutrient::Atp),
            enzymes: self.ledger.balance(Nutrient::Enzymes),
            mycelia: self.ledger.balance(Nutrient::Mycelia),
            spores: self.ledger.balance(Nutrient::Spores),
            resilience: self.ledger.balance(Nutrient::Resilience),
            anastomoses: m.total_anastomoses,
            atrophies: m.total_atrophies,
            messages_in: m.messages_in,
            messages_out: m.messages_out,
            home: self.store.root.display().to_string(),
            event_horizon: horizon_url,
            ion_endpoints: endpoints,
            isotope_atoms: self.nucleus.len(),
            isotope_shard: self.nucleus.index,
            isotope_ring: self.nucleus.ring_size,
            membrane: self.membrane.as_str().to_string(),
            sporocarp: self.sporocarp,
            dns_seed: self.dns_seed.clone(),
            wan_reachable: self.assume_reachable,
            is_relay: self.assume_reachable
                && (self.sporocarp || matches!(self.membrane, Membrane::Esporocarp)),
            active_relay: self.hyphae.active_relay_peer().map(|p| p.to_string()),
            relay_health: self.hyphae.relay_mesh_health_label(),
            physarum_phase: match self.physarum_phase {
                MyceliumPhase::Exploratory => "exploratory".into(),
                MyceliumPhase::Transport => "transport".into(),
                MyceliumPhase::Dormant => "dormant".into(),
            },
        }
    }

    /// Um passo Physarum: potenciais ← ATP + vizinhos; adapta condutâncias.
    fn physarum_tick(&mut self, dt: f64) {
        let neighbors = self.hyphae.connected_neighbors();
        let n = (neighbors + 1).clamp(2, 16);
        if self.physarum.n != n {
            self.physarum = PhysarumNetwork::new(n, 0.1, 0.01);
        }
        self.physarum.potentials[0] = self.ledger.balance(Nutrient::Atp) as f64;
        for i in 1..self.physarum.n {
            self.physarum.potentials[i] = if i <= neighbors { 1.0 + (i as f64) * 0.01 } else { 0.0 };
        }
        self.physarum.step(dt);
        let prev = self.physarum_phase;
        self.physarum_phase = self.physarum.phase();
        if self.physarum_phase != prev {
            tracing::info!(
                phase = ?self.physarum_phase,
                neighbors,
                "physarum fase"
            );
        }
    }

    pub fn sow(
        &mut self,
        message: String,
        path: String,
        content: String,
    ) -> Result<ContentId, OrganismError> {
        let plot = Plot {
            author: self.gland.node_id(),
            message,
            parents: vec![],
            leaves: vec![Leaf {
                path,
                content: content.into_bytes(),
            }],
        };
        let id = self.bank.deposit(plot.clone())?;
        let bytes = self.bank.spore_print(&id)?;
        let _ = self.hyphae.dht_store_local(dht_key(&id), bytes.clone());
        let _ = self.hyphae.dht_put(dht_key(&id), bytes);
        let env = Envelope::SporePrint { plot };
        let _ = self
            .hyphae
            .broadcast_lattice(env.encode().map_err(|e| OrganismError::Msg(e.to_string()))?);
        self.persist()?;
        Ok(id)
    }

    /// Publica uma árvore de código (repo) como Plot multi-leaf content-addressed,
    /// anunciada na DHT e difundida por gossip — distribuição soberana, sem GitHub.
    pub fn publish_repo(
        &mut self,
        message: String,
        leaves: Vec<giggs::Leaf>,
    ) -> Result<ContentId, OrganismError> {
        let plot = Plot {
            author: self.gland.node_id(),
            message,
            parents: vec![],
            leaves,
        };
        let id = self.bank.deposit(plot.clone())?;
        let bytes = self.bank.spore_print(&id)?;
        let _ = self.hyphae.dht_store_local(dht_key(&id), bytes.clone());
        let _ = self.hyphae.dht_put(dht_key(&id), bytes);
        let env = Envelope::SporePrint { plot };
        let _ = self
            .hyphae
            .broadcast_lattice(env.encode().map_err(|e| OrganismError::Msg(e.to_string()))?);
        self.persist()?;
        Ok(id)
    }

    // ── Micelial Value Layer ────────────────────────────────
    /// GhostID pubkey x-only do nó (carteira).
    pub fn wallet_pubkey(&self) -> [u8; 32] {
        self.ghost.nostr_pubkey()
    }

    pub fn wallet_pubkey_hex(&self) -> String {
        self.ghost.nostr_pubkey_hex()
}

/// Cria, assina, aplica e propaga uma transferência de nutrientes.

    pub fn transfer(
        &mut self,
        to_hex: &str,
        amount: u64,
        nutrient: mycelium_core::Nutrient,
        kind: mycelium_nutrients::TxKind,
        memo: String,
        asset: Option<String>,
    ) -> Result<mycelium_nutrients::SignedTransfer, String> {
        let to = hex::decode(to_hex)
            .map_err(|e| format!("pubkey destino inválida: {e}"))?
            .try_into()
            .map_err(|_| "pubkey destino precisa de 32 bytes".to_string())?;

        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let payload = mycelium_nutrients::TransferPayload {
            kind,
            from: self.wallet_pubkey(),
            to,
            nutrient,
            amount,
            memo,
            asset,
            nonce: self.transfer_nonce,
            ts,
        };
        let bytes = serde_json::to_vec(&payload).map_err(|e| e.to_string())?;
        let sig = self.ghost.sign(&bytes);
        self.transfer_nonce += 1;

        let tx = mycelium_nutrients::SignedTransfer {
            payload,
            sig: hex::encode(sig),
        };

        // Verifica a própria assinatura e aplica no ledger local.
        self.apply_incoming_transfer(tx.clone(), true).map_err(|e| e.to_string())?;

        // Propaga pela malha.
        let env = Envelope::ValueTransfer { tx: tx.clone() };
        let _ = self
            .hyphae
            .broadcast_lattice(env.encode().map_err(|e| e.to_string())?);
        self.persist().map_err(|e| e.to_string())?;
        Ok(tx)
    }

    /// Verifica assinatura Schnorr (GhostID) e aplica no ledger local.
    /// `propagated=true` não re-propaga (para a origem).
    pub fn apply_incoming_transfer(
        &mut self,
        tx: mycelium_nutrients::SignedTransfer,
        propagated: bool,
    ) -> Result<(), String> {
        let p = &tx.payload;
        let sig = hex::decode(&tx.sig)
            .map_err(|e| format!("assinatura inválida: {e}"))?;
        let sig: [u8; 64] = sig
            .try_into()
            .map_err(|_| "assinatura precisa de 64 bytes".to_string())?;
        let hash = mycelium_core::ContentId::of(&tx.canonical_bytes()).0;
        mycelium_ghostid::GhostId::verify_nostr_event(&p.from, &hash, &sig)
            .map_err(|_| "assinatura Schnorr inválida".to_string())?;

        let my = self.wallet_pubkey();
        self.ledger.apply_transfer(&tx, &my).map_err(|e| e.to_string())?;

        if !propagated {
            let env = Envelope::ValueTransfer { tx };
            let _ = self.hyphae.broadcast_lattice(
                env.encode().map_err(|e| e.to_string())?,
            );
        }
        Ok(())
    }

    /// Recompensa por proof-of-relay / compute (Fase 2), chamada periodicamente.
    pub fn reward_hardware(&mut self, neighbors: usize) {
        if neighbors > 0 {
            self.ledger.relay_reward((neighbors as u64) * 5);
        }
        self.ledger.heartbeat(1);
    }

    pub fn emit_signal(
        &mut self,
        plot: ContentId,
        quorum: usize,
        ion: String,
        name: String,
    ) -> Result<ContentId, OrganismError> {
        if self.bank.recall(&plot).is_none() {
            return Err(OrganismError::Msg(format!(
                "plot {plot} ausente do Spore Bank local"
            )));
        }
        let id = self.state.field.emit(
            self.gland.node_id(),
            Proposal::Pipeline {
                name,
                plot,
                target_ion: ion,
            },
            quorum,
        )?;
        let _ = self.state.field.resonate(&id, self.gland.node_id());
        let signal = self
            .state
            .field
            .get(&id)
            .cloned()
            .ok_or_else(|| OrganismError::Msg("signal sumiu".into()))?;
        let env = Envelope::SignalBroadcast { signal };
        let _ = self
            .hyphae
            .broadcast_lattice(env.encode().map_err(|e| OrganismError::Msg(e.to_string()))?);
        self.try_fire_pipelines()?;
        self.persist()?;
        Ok(id)
    }

    pub fn resonate(&mut self, signal_id: ContentId) -> Result<SignalState, OrganismError> {
        let state = self
            .state
            .field
            .resonate(&signal_id, self.gland.node_id())?;
        let env = Envelope::Resonance {
            signal_id,
            resonator: self.gland.node_id(),
        };
        let _ = self
            .hyphae
            .broadcast_lattice(env.encode().map_err(|e| OrganismError::Msg(e.to_string()))?);
        self.try_fire_pipelines()?;
        self.persist()?;
        Ok(state)
    }

    fn try_fire_pipelines(&mut self) -> Result<(), OrganismError> {
        let fired: Vec<_> = self
            .state
            .field
            .fired()
            .filter(|s| !self.processed.contains(&s.id))
            .cloned()
            .collect();

        for signal in fired {
            if let Proposal::Pipeline {
                plot,
                target_ion,
                name,
            } = &signal.proposal
            {
                let i_am_origin = signal.origin == self.gland.node_id();
                // Só o emissor do Signal faz Build→Test→Deploy local.
                // Peers remotes ganham ATP via VectorOffer (Build/Test), sem frutar Chamber.
                if !i_am_origin {
                    tracing::info!(
                        signal = %signal.id.short(),
                        origin = %signal.origin.short(),
                        "pipeline fired — peer remoto ignora Deploy (origin_only)"
                    );
                    self.processed.insert(signal.id);
                    continue;
                }

                tracing::info!(
                    signal = %signal.id.short(),
                    ion = %target_ion,
                    "pipeline fired — spinning inertia (origin)"
                );
                let work = self.prepare_workbench(plot)?;
                self.flywheel.inject(Vector {
                    plot: *plot,
                    thrust: Thrust::Build,
                    emitter: signal.origin,
                });
                self.flywheel.inject(Vector {
                    plot: *plot,
                    thrust: Thrust::Test,
                    emitter: signal.origin,
                });
                self.flywheel.inject(Vector {
                    plot: *plot,
                    thrust: Thrust::Deploy {
                        target_ion: target_ion.clone(),
                    },
                    emitter: signal.origin,
                });

                while let Ok((vector, momentum)) =
                    self.flywheel.spin(self.gland.node_id(), &work)
                {
                    self.ledger
                        .feed(Nutrient::Atp, momentum.atp_earned, &momentum.log);
                    tracing::info!("{}", momentum.log);
                    if !momentum.success {
                        tracing::warn!(thrust = ?vector.thrust, "inertia falhou — abortando pipeline");
                        break;
                    }
                    if matches!(vector.thrust, Thrust::Build) {
                        let archive = match inertia::collect_artifact(&work) {
                            Some(files) => {
                                let mut a = LayerArchive::new();
                                for (path, bytes) in files {
                                    a.insert(path, bytes);
                                }
                                a
                            }
                            None => {
                                let fallback = self
                                    .bank
                                    .spore_print(plot)
                                    .unwrap_or_else(|_| b"{}".to_vec());
                                LayerArchive::single("app.payload", fallback)
                            }
                        };
                        self.build_artifacts.insert(*plot, archive);
                    }
                    if let Thrust::Deploy { ref target_ion } = vector.thrust {
                        self.birth_ion(target_ion, &vector.plot.to_string(), name)?;
                    }
                    self.broadcast_momentum(&vector, &momentum, self.gland.node_id())?;
                    // Oferece Build/Test à rede (Deploy fica no emissor).
                    if !matches!(vector.thrust, Thrust::Deploy { .. }) {
                        let env = Envelope::VectorOffer {
                            vector: vector.clone(),
                        };
                        let _ = self.hyphae.broadcast_lattice(
                            env.encode().map_err(|e| OrganismError::Msg(e.to_string()))?,
                        );
                    }
                }
                self.processed.insert(signal.id);
            } else {
                self.processed.insert(signal.id);
            }
        }
        Ok(())
    }

    fn prepare_workbench(&self, plot: &ContentId) -> Result<PathBuf, OrganismError> {
        let plot_data = self
            .bank
            .recall(plot)
            .ok_or_else(|| OrganismError::Msg(format!("plot {plot} ausente para build")))?;
        let work = self.store.builds_dir().join(plot.short());
        let leaves: Vec<(String, Vec<u8>)> = plot_data
            .leaves
            .iter()
            .map(|l| (l.path.clone(), l.content.clone()))
            .collect();
        inertia::materialize_leaves(&work, &leaves)
            .map_err(|e| OrganismError::Msg(e.to_string()))?;
        std::fs::write(work.join("MESSAGE"), plot_data.message.as_bytes())
            .map_err(|e| OrganismError::Msg(e.to_string()))?;
        Ok(work)
    }

    fn vector_fingerprint(vector: &Vector) -> String {
        format!(
            "{}:{:?}:{}",
            vector.plot,
            vector.thrust,
            vector.emitter.short()
        )
    }

    fn broadcast_momentum(
        &mut self,
        vector: &Vector,
        momentum: &Momentum,
        executor: NodeId,
    ) -> Result<(), OrganismError> {
        let env = Envelope::MomentumReport {
            vector: vector.clone(),
            momentum: momentum.clone(),
            executor,
        };
        let _ = self
            .hyphae
            .broadcast_lattice(env.encode().map_err(|e| OrganismError::Msg(e.to_string()))?);
        Ok(())
    }

    /// Anuncia layer no DHT + gossip.
    fn announce_layer(&mut self, id: ContentId, bytes: &[u8]) -> Result<(), OrganismError> {
        let key = layer_dht_key(&id);
        let _ = self.hyphae.dht_store_local(key.clone(), bytes.to_vec());
        let _ = self.hyphae.dht_put(key, bytes.to_vec());
        let env = Envelope::LayerOffer { id };
        let _ = self
            .hyphae
            .broadcast_lattice(env.encode().map_err(|e| OrganismError::Msg(e.to_string()))?);
        Ok(())
    }

    /// Se a layer falta, pede à rede (gossip + DHT).
    fn request_layer(&mut self, id: &ContentId) {
        tracing::info!(layer = %id.short(), "pedindo layer aos vizinhos");
        let env = Envelope::LayerNeed { id: *id };
        if let Ok(bytes) = env.encode() {
            let _ = self.hyphae.broadcast_lattice(bytes);
        }
        self.hyphae.dht_get(layer_dht_key(id));
    }

    fn serve_layer_if_present(&mut self, id: &ContentId) -> Result<(), OrganismError> {
        let store = LayerStore::open(self.store.layers_dir())
            .map_err(|e| OrganismError::Msg(e.to_string()))?;
        if let Some(bytes) = store.get(id) {
            self.announce_layer(*id, &bytes)?;
            tracing::info!(layer = %id.short(), "layer servida ao pedido");
        }
        Ok(())
    }

    /// Executa Vector remoto (Build/Test) se houver CPU ociosa e Plot local.
    fn accept_remote_vector(&mut self, vector: Vector) -> Result<(), OrganismError> {
        if self.resources.cpu_cores == 0 || self.flywheel.pending() > 2 {
            return Ok(());
        }
        if matches!(vector.thrust, Thrust::Deploy { .. }) {
            return Ok(());
        }
        if vector.emitter == self.gland.node_id() {
            return Ok(());
        }
        let fp = Self::vector_fingerprint(&vector);
        if self.remote_done.contains(&fp) {
            return Ok(());
        }
        if self.bank.recall(&vector.plot).is_none() {
            self.hyphae.dht_get(dht_key(&vector.plot));
            tracing::debug!(plot = %vector.plot.short(), "vector remoto: plot ausente, DHT get");
            return Ok(());
        }
        let work = self.prepare_workbench(&vector.plot)?;
        self.flywheel.inject(vector);
        if let Ok((v, momentum)) = self.flywheel.spin(self.gland.node_id(), &work) {
            self.remote_done.insert(Self::vector_fingerprint(&v));
            self.ledger
                .feed(Nutrient::Atp, momentum.atp_earned, &momentum.log);
            tracing::info!(
                plot = %v.plot.short(),
                "vector remoto executado: {}",
                momentum.log
            );
            self.broadcast_momentum(&v, &momentum, self.gland.node_id())?;
        }
        Ok(())
    }

    fn birth_ion(
        &mut self,
        name: &str,
        plot: &str,
        pipeline: &str,
    ) -> Result<(), OrganismError> {
        if self.state.ions.iter().any(|i| i.name == name) {
            // Já registrado — garante que a chamber está viva.
            if !self.chambers.contains_key(name) {
                self.fruit_ion(name, plot, pipeline, false)?;
            }
            return Ok(());
        }
        self.fruit_ion(name, plot, pipeline, true)?;
        Ok(())
    }

    /// Materializa Chamber (processo) + Orbit no Event Horizon.
    fn fruit_ion(
        &mut self,
        name: &str,
        plot: &str,
        pipeline: &str,
        persist_record: bool,
    ) -> Result<(), OrganismError> {
        let plot_id: ContentId = plot.parse().map_err(OrganismError::Msg)?;
        let message = self
            .bank
            .recall(&plot_id)
            .map(|p| p.message.clone())
            .unwrap_or_else(|| format!("ion:{name}"));

        let layer_store = LayerStore::open(self.store.layers_dir())
            .map_err(|e| OrganismError::Msg(e.to_string()))?;
        let mut base = LayerArchive::single("MESSAGE", message.as_bytes());
        base.insert("pipeline.txt", pipeline.as_bytes().to_vec());
        let base_bytes = base
            .encode()
            .map_err(|e| OrganismError::Msg(e.to_string()))?;
        let base_id = layer_store
            .put(&base_bytes)
            .map_err(|e| OrganismError::Msg(e.to_string()))?;
        self.announce_layer(base_id, &base_bytes)?;

        let app = self.build_artifacts.remove(&plot_id).unwrap_or_else(|| {
            let payload = self
                .bank
                .spore_print(&plot_id)
                .unwrap_or_else(|_| message.as_bytes().to_vec());
            LayerArchive::single("app.payload", payload)
        });
        let app_bytes = app
            .encode()
            .map_err(|e| OrganismError::Msg(e.to_string()))?;
        let app_id = layer_store
            .put(&app_bytes)
            .map_err(|e| OrganismError::Msg(e.to_string()))?;
        self.announce_layer(app_id, &app_bytes)?;

        let void = Void {
            name: name.to_string(),
            layers: vec![base_id, app_id],
            entrypoint: "chamber-serve".into(),
        };
        // Se alguma layer sumir do disco, pede à rede antes de falhar.
        for lid in &void.layers {
            if !layer_store.has(lid) {
                self.request_layer(lid);
            }
        }
        let chamber = Chamber::suck_store(void.clone(), &layer_store, self.resources)?;
        let ion = Ion::birth(name, self.gland.node_id(), chamber);
        match self.cloud.inject(ion) {
            Ok(()) | Err(plasma::PlasmaError::AlreadyOrbiting(_)) => {}
            Err(e) => return Err(OrganismError::Msg(e.to_string())),
        }

        let mem = if self.resources.ram_mib > 0 {
            Some(self.resources.ram_mib)
        } else {
            None
        };
        let cpu = if self.resources.cpu_cores > 0 {
            Some(self.resources.cpu_cores)
        } else {
            None
        };
        let proc = ChamberProcess::fruit_void(
            &self.mycelium_bin,
            &self.store.chambers_dir(),
            &void,
            &layer_store,
            &message,
            FruitOptions {
                isolation: Isolation::Auto,
                memory_mib: mem,
                cpu_cores: cpu,
            },
        )?;

        let host = format!("sporocarp.mycelium/{}", self.gland.node_id().short());
        {
            let mut table = self.horizon.write().unwrap();
            table.expose(
                &host,
                Orbit {
                    ion: name.to_string(),
                    node: self.gland.node_id(),
                    mass: self.resources.cpu_cores as u64 * 10 + 1,
                    resistance: 0,
                    upstream: proc.upstream.clone(),
                },
            );
        }

        tracing::info!(
            ion = name,
            upstream = %proc.upstream,
            layers = ?void.layers.iter().map(|l| l.short()).collect::<Vec<_>>(),
            horizon = %format!("http://127.0.0.1:{}/{name}/", self.state.horizon_port),
            "chamber viva — ion no event horizon"
        );

        self.chambers.insert(name.to_string(), proc);

        if persist_record {
            self.state.ions.push(IonRecord {
                name: name.to_string(),
                plot: plot_id.to_string(),
                pipeline: pipeline.to_string(),
            });
            if self.ledger.balance(Nutrient::Atp) > 0 {
                let _ = self
                    .ledger
                    .metabolize(Nutrient::Atp, 1, None, format!("deploy:{name}"));
            }
            self.persist()?;
        }
        Ok(())
    }

    pub fn isotope_put(
        &mut self,
        key: String,
        value: String,
        clock: Option<u64>,
    ) -> Result<(u64, bool), OrganismError> {
        let clock = clock.unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(1)
        });
        let atom = Atom {
            value: value.into_bytes(),
            clock,
        };
        let owned = self.nucleus.owns(&key);
        if owned {
            self.nucleus
                .write(&key, atom.value.clone(), clock)
                .map_err(|e| OrganismError::Msg(e.to_string()))?;
        } else {
            // Cache local; o dono do shard persiste via AtomSync.
            self.nucleus.absorb(&key, atom.clone());
            tracing::debug!(
                %key,
                shard = Nucleus::shard_of(&key, self.nucleus.ring_size),
                local = self.nucleus.index,
                "isotope put em shard remoto — AtomSync"
            );
        }
        let env = Envelope::AtomSync {
            key: key.clone(),
            atom,
        };
        let _ = self
            .hyphae
            .broadcast_lattice(env.encode().map_err(|e| OrganismError::Msg(e.to_string()))?);
        self.persist()?;
        Ok((clock, owned))
    }

    /// Hit local, ou dispara Decay e devolve None enquanto aguarda reply.
    pub fn isotope_get(&mut self, key: &str) -> Result<Option<Atom>, OrganismError> {
        if let Some(atom) = self.nucleus.decay(key) {
            self.pending_decays.remove(key);
            return Ok(Some(atom.clone()));
        }
        self.begin_decay(key)?;
        Ok(None)
    }

    fn begin_decay(&mut self, key: &str) -> Result<(), OrganismError> {
        if self.pending_decays.contains(key) {
            return Ok(());
        }
        self.pending_decays.insert(key.to_string());
        let env = Envelope::DecayQuery {
            key: key.to_string(),
            asker: self.gland.node_id(),
        };
        let _ = self
            .hyphae
            .broadcast_lattice(env.encode().map_err(|e| OrganismError::Msg(e.to_string()))?);
        tracing::info!(%key, "decay query enviado às hifas");
        Ok(())
    }

    fn reply_decay(&mut self, key: &str, asker: NodeId) -> Result<(), OrganismError> {
        if asker == self.gland.node_id() {
            return Ok(());
        }
        let Some(atom) = self.nucleus.decay(key).cloned() else {
            return Ok(());
        };
        let env = Envelope::DecayReply {
            key: key.to_string(),
            atom,
        };
        let _ = self
            .hyphae
            .broadcast_lattice(env.encode().map_err(|e| OrganismError::Msg(e.to_string()))?);
        tracing::debug!(%key, asker = %asker.short(), "decay reply enviado");
        Ok(())
    }

    fn handle_envelope(&mut self, env: Envelope) -> Result<(), OrganismError> {
        match env {
            Envelope::SporePrint { plot } => {
                let id = self.bank.deposit(plot)?;
                let bytes = self.bank.spore_print(&id)?;
                let _ = self.hyphae.dht_store_local(dht_key(&id), bytes.clone());
                let _ = self.hyphae.dht_put(dht_key(&id), bytes);
                tracing::info!(plot = %id.short(), "spore print absorvido");
            }
            Envelope::SignalBroadcast { signal } => {
                let id = self.state.field.absorb(signal);
                tracing::info!(signal = %id.short(), "signal absorvido");
                self.try_fire_pipelines()?;
            }
            Envelope::Resonance {
                signal_id,
                resonator,
            } => match self.state.field.absorb_resonance(&signal_id, resonator) {
                Ok(state) => {
                    tracing::info!(signal = %signal_id.short(), ?state, "ressonância absorvida");
                    self.try_fire_pipelines()?;
                }
                Err(thefield::FieldError::SignalNotFound(_)) => {}
                Err(e) => return Err(e.into()),
            },
            Envelope::VectorOffer { vector } => {
                tracing::debug!(plot = %vector.plot.short(), "vector oferecido na rede");
                self.accept_remote_vector(vector)?;
            }
            Envelope::MomentumReport {
                vector,
                momentum,
                executor,
            } => {
                tracing::info!(
                    plot = %vector.plot.short(),
                    executor = %executor.short(),
                    success = momentum.success,
                    "momentum report: {}",
                    momentum.log
                );
                // Crédito simbólico no emissor quando o trabalho veio de outro nó.
                if vector.emitter == self.gland.node_id() && executor != self.gland.node_id() {
                    self.ledger.feed(
                        Nutrient::Spores,
                        1,
                        format!("remote-inertia:{}", executor.short()),
                    );
                }
            }
            Envelope::AtomSync { key, atom } => {
                if self.nucleus.owns(&key) {
                    self.nucleus.absorb(&key, atom);
                    tracing::info!(%key, "atom sync absorvido (dono do shard)");
                } else if self.pending_decays.contains(&key) {
                    self.nucleus.absorb(&key, atom);
                    self.pending_decays.remove(&key);
                    tracing::info!(%key, "atom sync absorvido (decay pendente)");
                } else {
                    tracing::debug!(%key, "atom sync ignorado — use Decay para ler shard remoto");
                }
            }
            Envelope::LayerOffer { id } => {
                let store = LayerStore::open(self.store.layers_dir())
                    .map_err(|e| OrganismError::Msg(e.to_string()))?;
                if !store.has(&id) {
                    self.hyphae.dht_get(layer_dht_key(&id));
                    tracing::debug!(layer = %id.short(), "layer offer → DHT get");
                }
            }
            Envelope::LayerNeed { id } => {
                self.serve_layer_if_present(&id)?;
            }
            Envelope::DecayQuery { key, asker } => {
                self.reply_decay(&key, asker)?;
            }
            Envelope::DecayReply { key, atom } => {
                self.nucleus.absorb(&key, atom);
                self.pending_decays.remove(&key);
                tracing::info!(%key, "decay reply absorvido");
            }
            Envelope::ShadeOffer {
                shade,
                custodian,
                from,
            } => {
                if custodian == self.gland.node_id() {
                    self.vault.hold(from, shade);
                    self.persist()?;
                    tracing::info!(%from, "shade custodiada");
                }
            }
            Envelope::ShadeRequest {
                requester,
                threshold: _,
            } => {
                if requester != self.gland.node_id() && !self.vault.is_empty() {
                    for shade in self.vault.gather() {
                        let env = Envelope::ShadeOffer {
                            shade,
                            custodian: requester,
                            from: self.gland.node_id(),
                        };
                        if let Ok(bytes) = env.encode() {
                            let _ = self.hyphae.broadcast_lattice(bytes);
                        }
                    }
                    tracing::info!(%requester, "shades enviadas ao requisitante");
                }
            }
            Envelope::BalanceSync { node_id, balances, clock } => {
                if node_id == self.gland.node_id() {
                    return Ok(());
                }
                let entry = self.remote_ledger.entry(node_id).or_default();
                if clock > entry.1 {
                    entry.0 = balances;
                    entry.1 = clock;
                }
            }
            Envelope::IonOffer { ion, host: _, charge: _, desired_replicas: _, layers } => {
                if self.resources.cpu_cores == 0 || self.chambers.contains_key(&ion) {
                    return Ok(());
                }
                // Aceita se tem recursos ociosos
                let env = Envelope::IonAccept {
                    ion: ion.clone(),
                    acceptor: self.gland.node_id(),
                };
                if let Ok(bytes) = env.encode() {
                    let _ = self.hyphae.broadcast_lattice(bytes);
                }
                // Pede as layers
                for lid in &layers {
                    let env = Envelope::LayerNeed { id: *lid };
                    if let Ok(bytes) = env.encode() {
                        let _ = self.hyphae.broadcast_lattice(bytes);
                    }
                }
            }
            Envelope::IonAccept { ion, acceptor } => {
                if acceptor == self.gland.node_id() {
                    return Ok(());
                }
                tracing::info!(%ion, %acceptor, "IonOffer aceito");
            }
            Envelope::IonMigrate { ion, void, layers } => {
                let layer_store = match vacuum::LayerStore::open(self.store.layers_dir()) {
                    Ok(s) => s,
                    Err(_) => return Ok(()),
                };
                for (lid, data) in &layers {
                    let p = layer_store.path_of(lid);
                    if !p.exists() {
                        let _ = std::fs::write(&p, data);
                    }
                }
                let missing: Vec<ContentId> = void.layers.iter().filter(|lid| !layer_store.has(lid)).copied().collect();
                if !missing.is_empty() {
                    for lid in &missing {
                        self.request_layer(lid);
                    }
                    return Ok(());
                }
                let name = void.name.clone();
                match vacuum::ChamberProcess::fruit_void(
                    &self.mycelium_bin,
                    &self.store.chambers_dir(),
                    &void,
                    &layer_store,
                    &name,
                    vacuum::FruitOptions::default(),
                ) {
                    Ok(proc) => {
                        let host = format!("sporocarp.mycelium/{}", self.gland.node_id().short());
                        let upstream = proc.upstream.clone();
                        {
                            let mut table = self.horizon.write().unwrap();
                            table.expose(&host, singularity::Orbit {
                                ion: ion.clone(),
                                node: self.gland.node_id(),
                                mass: self.resources.cpu_cores as u64 * 10 + 1,
                                resistance: 0,
                                upstream: upstream.clone(),
                            });
                        }
                        self.chambers.insert(ion.clone(), proc);
                        let env = Envelope::IonReady {
                            ion: ion.clone(),
                            node: self.gland.node_id(),
                            upstream,
                        };
                        if let Ok(bytes) = env.encode() {
                            let _ = self.hyphae.broadcast_lattice(bytes);
                        }
                        tracing::info!(%ion, "Ion migrado aceito — ChamberProcess frutificada");
                    }
                    Err(e) => tracing::warn!(error = %e, "IonMigrate fruit_void falhou"),
                }
            }
            Envelope::IonReady { ion, node, upstream } => {
                if node == self.gland.node_id() {
                    return Ok(());
                }
                let host = format!("sporocarp.mycelium/{}", self.gland.node_id().short());
                {
                    let mut table = self.horizon.write().unwrap();
                    table.expose(&host, singularity::Orbit {
                        ion: ion.clone(),
                        node,
                        mass: 10,
                        resistance: 1,
                        upstream,
                    });
                }
                tracing::info!(%ion, %node, "IonReady — rota adicionada no Horizon");
            }
            Envelope::ZoneAnnounce { prefix, custodian } => {
                self.known_zones.entry(prefix).or_default().push(custodian);
            }
            Envelope::ValueTransfer { tx } => {
                if let Err(e) = self.apply_incoming_transfer(tx, false) {
                    tracing::debug!("value-transfer rejeitada: {e}");
                }
            }
        }
        self.persist()?;
        Ok(())
    }

    fn handle_control(&mut self, req: Request) -> Response {
        match req {
            Request::Status => Response::Status(Box::new(self.status_report())),
            Request::Sow {
                message,
                path,
                content,
                qel,
                nostr,
                ghost,
                recipient,
            } => match self.sow(message, path, content) {
                Ok(id) => {
                    let _ = (qel, nostr, ghost, recipient);
                    Response::Ok {
                        message: format!("plot semeado: {id}"),
                    }
                }
                Err(e) => Response::Err {
                    message: e.to_string(),
                },
            },
            Request::Signal {
                plot,
                quorum,
                ion,
                name,
            } => match plot.parse::<ContentId>() {
                Ok(plot_id) => match self.emit_signal(plot_id, quorum, ion, name) {
                    Ok(id) => Response::Ok {
                        message: format!("signal emitido: {id}"),
                    },
                    Err(e) => Response::Err {
                        message: e.to_string(),
                    },
                },
                Err(e) => Response::Err { message: e },
            },
            Request::Resonate { signal } => match signal.parse::<ContentId>() {
                Ok(id) => match self.resonate(id) {
                    Ok(state) => Response::Ok {
                        message: format!("ressonância ok: {state:?}"),
                    },
                    Err(e) => Response::Err {
                        message: e.to_string(),
                    },
                },
                Err(e) => Response::Err { message: e },
            },
            Request::Recall {
                plot,
                qel,
                nostr,
                qel_threshold,
            } => match plot.parse::<ContentId>() {
                Ok(id) => match self.bank.recall(&id) {
                    Some(p) => Response::Ok {
                        message: format!(
                            "plot {} — \"{}\" ({} leaves)",
                            id.short(),
                            p.message,
                            p.leaves.len()
                        ),
                    },
                    None => {
                        let _ = (qel, nostr, qel_threshold);
                        self.hyphae.dht_get(dht_key(&id));
                        Response::Ok {
                            message: format!(
                                "plot {} ausente localmente; consulta DHT disparada (usa CLI --qel --nostr para mailbox)",
                                id.short()
                            ),
                        }
                    }
                },
                Err(e) => Response::Err { message: e },
            },
            Request::Bootstrap { addr } => match addr.parse() {
                Ok(multiaddr) => match self.hyphae.reach(multiaddr) {
                    Ok(()) => {
                        if !self.state.bootstrap.contains(&addr) {
                            self.state.bootstrap.push(addr.clone());
                            let _ = self.persist();
                        }
                        Response::Ok {
                            message: format!("dialando {addr}"),
                        }
                    }
                    Err(e) => Response::Err {
                        message: e.to_string(),
                    },
                },
                Err(e) => Response::Err {
                    message: format!("multiaddr inválido: {e}"),
                },
            },
            Request::IsotopePut { key, value, clock } => match self.isotope_put(key, value, clock)
            {
                Ok((c, owned)) => Response::Ok {
                    message: format!("atom escrito (clock={c}, owned={owned})"),
                },
                Err(e) => Response::Err {
                    message: e.to_string(),
                },
            },
            Request::IsotopeGet { key } => match self.isotope_get(&key) {
                Ok(Some(atom)) => {
                    let val = String::from_utf8_lossy(&atom.value);
                    Response::Ok {
                        message: format!("atom {key}={val} (clock={})", atom.clock),
                    }
                }
                Ok(None) => Response::Err {
                    message: format!("decay em curso para `{key}` — tente de novo"),
                },
                Err(e) => Response::Err {
                    message: e.to_string(),
                },
            },
            Request::EntropyShatter {
                secret,
                threshold,
                total,
            } => {
                match entropy::Vault::shatter(secret.as_bytes(), threshold, total) {
                    Ok(shades) => {
                        let n = shades.len();
                        self.vault = entropy::Vault::new();
                        let node_id = self.gland.node_id();
                        for (i, s) in shades.into_iter().enumerate() {
                            let custodian = if i == 0 {
                                node_id
                            } else {
                                // Distribui para peers via gossip
                                let env = Envelope::ShadeOffer {
                                    shade: s.clone(),
                                    custodian: node_id,
                                    from: node_id,
                                };
                                if let Ok(bytes) = env.encode() {
                                    let _ = self.hyphae.broadcast_lattice(bytes);
                                }
                                // Hold local da primeira shade
                                node_id
                            };
                            self.vault.hold(custodian, s);
                        }
                        self.persist().ok();
                        Response::Ok {
                            message: format!("entropy: {n} shades geradas e distribuídas ({threshold}+{total})"),
                        }
                    }
                    Err(e) => Response::Err {
                        message: format!("entropy shatter: {e}"),
                    },
                }
            }
            Request::EntropyReconstruct { threshold } => {
                // Tenta coleta remota primeiro
                let env = Envelope::ShadeRequest {
                    requester: self.gland.node_id(),
                    threshold,
                };
                if let Ok(bytes) = env.encode() {
                    let _ = self.hyphae.broadcast_lattice(bytes);
                }
                // Espera por respostas ShadeOffer via gossip
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
                while std::time::Instant::now() < deadline {
                    let gathered = self.vault.gather();
                    if gathered.len() >= threshold as usize {
                        break;
                    }
                    std::thread::yield_now();
                }
                let gathered = self.vault.gather();
                if gathered.len() < threshold as usize {
                    return Response::Err {
                        message: format!(
                            "entropy: só {} shades em custódia (precisa de {})",
                            gathered.len(),
                            threshold
                        ),
                    };
                }
                match entropy::ChaosKey::materialize(&gathered, threshold) {
                    Ok(key) => {
                        let hex = hex::encode(key.reveal().unwrap_or(&[]));
                        Response::Ok {
                            message: format!("entropy: segredo reconstruído: {hex}"),
                        }
                    }
                    Err(e) => Response::Err {
                        message: format!("entropy reconstruct: {e}"),
                    },
                }
            }
            Request::EntropyStatus => {
                let count = self.vault.len();
                let shades_hex: Vec<String> = self
                    .vault
                    .gather()
                    .iter()
                    .map(|s| format!("shade[{}]: {} bytes", s.index, s.shares.len()))
                    .collect();
                Response::Ok {
                    message: format!(
                        "entropy: {} shades em custódia\n{}",
                        count,
                        shades_hex.join("\n")
                    ),
                }
            }
            Request::Balance => {
                let mut msg = format!(
                    "ATP={} Enzymes={} Mycelia={} Spores={} Resilience={} (local)\n",
                    self.ledger.balance(Nutrient::Atp),
                    self.ledger.balance(Nutrient::Enzymes),
                    self.ledger.balance(Nutrient::Mycelia),
                    self.ledger.balance(Nutrient::Spores),
                    self.ledger.balance(Nutrient::Resilience),
                );
                for (peer, (bals, clock)) in &self.remote_ledger {
                    msg.push_str(&format!(
                        "peer {}... (clock {clock}): ATP={} Enzymes={} Mycelia={} Spores={} Resilience={}\n",
                        peer.short(),
                        bals.get(&Nutrient::Atp).copied().unwrap_or(0),
                        bals.get(&Nutrient::Enzymes).copied().unwrap_or(0),
                        bals.get(&Nutrient::Mycelia).copied().unwrap_or(0),
                        bals.get(&Nutrient::Spores).copied().unwrap_or(0),
                        bals.get(&Nutrient::Resilience).copied().unwrap_or(0),
                    ));
                }
                Response::Ok { message: msg.trim().to_string() }
            }
            Request::IonMigrate { ion, target } => {
                let ion_name = ion.clone();
                let chamber = match self.chambers.get(&ion_name) {
                    Some(c) => c,
                    None => return Response::Err { message: format!("ion `{ion_name}` não está neste nó") },
                };
                // Extrai Void do chamber spec
                let void = vacuum::Void {
                    name: ion_name.clone(),
                    layers: chamber.void_layers().iter().filter_map(|s| s.parse::<ContentId>().ok()).collect(),
                    entrypoint: "chamber-serve".into(),
                };
                // Lê layers do LayerStore
                let layer_store = match vacuum::LayerStore::open(self.store.layers_dir()) {
                    Ok(s) => s,
                    Err(_) => return Response::Err { message: "layer store indisponível".into() },
                };
                let mut layers_data = Vec::new();
                for lid in &void.layers {
                    if let Some(bytes) = layer_store.get(lid) {
                        layers_data.push((*lid, bytes));
                    }
                }
                let n_layers = void.layers.len();
                let env = Envelope::IonMigrate {
                    ion: ion_name.clone(),
                    void,
                    layers: layers_data,
                };
                if let Ok(bytes) = env.encode() {
                    let _ = self.hyphae.broadcast_lattice(bytes);
                }
                self.ion_hosts.insert(ion_name.clone(), target);
                Response::Ok { message: format!("ion `{ion_name}` Void + {n_layers} layers enviado para migração") }
            }
            Request::Zones => {
                let mut msg = String::new();
                for (prefix, custodians) in &self.known_zones {
                    msg.push_str(&format!("zone {}: {} custodians\n", prefix, custodians.len()));
                    for c in custodians {
                        msg.push_str(&format!("  {}\n", c.short()));
                    }
                }
                if msg.is_empty() {
                    msg = "nenhuma zona conhecida".into();
                }
                Response::Ok { message: msg.trim().to_string() }
            }
            Request::StoreList => {
                let catalog = self.catalog.lock().unwrap();
                let spores: Vec<_> = catalog.list_public_spores().into_iter().cloned().collect();
                Response::StoreList { spores }
            }
            Request::StoreCaps => {
                let caps = mycelium_store::EmulatorRunner::detect_capabilities();
                Response::StoreCaps { caps }
            }
            Request::StoreLaunch { id, engine, sandbox } => {
                let catalog = self.catalog.lock().unwrap();
                let spore = match catalog.get_spore(&id) {
                    Some(s) => s,
                    None => return Response::Err {
                        message: format!("spore '{}' não encontrado no catálogo", id),
                    },
                };
                let spore = spore.clone();
                let caps = mycelium_store::EmulatorRunner::detect_capabilities();
                let forced = engine.as_deref().map(|e| {
                    use mycelium_store::ExecutionEngineType;
                    match e {
                        "native" => ExecutionEngineType::Native,
                        "retroarch" => ExecutionEngineType::RetroArchLibretro,
                        "mame" => ExecutionEngineType::MAME,
                        "qemu" => ExecutionEngineType::QEMU,
                        "wasm" => ExecutionEngineType::WebAssembly,
                        "cloud" => ExecutionEngineType::P2PCloudStream,
                        _ => ExecutionEngineType::Native,
                    }
                });
                let resolved = mycelium_store::EmulatorRunner::resolve_best_engine(&spore, &caps, forced);
                let game_path = self.store.root.join("store").join(&spore.main_binary_file);
                match mycelium_store::EmulatorRunner::launch(&spore, &game_path, resolved.clone(), sandbox) {
                    Ok(_child) => Response::StoreLaunched {
                        spore_id: spore.id,
                        engine: format!("{:?}", resolved),
                        message: format!("{} lançado via {:?}", spore.title, resolved),
                    },
                    Err(e) => Response::Err {
                        message: format!("falha ao lançar: {}", e),
                    },
                }
            }
            Request::StorePublish { id, title, platform } => {
                let mut catalog = self.catalog.lock().unwrap();
                let plat = platform.to_lowercase();
                let platform = match plat.as_str() {
                    "snes" => mycelium_store::TargetPlatform::SNES,
                    "nes" => mycelium_store::TargetPlatform::NES,
                    "megadrive" | "genesis" => mycelium_store::TargetPlatform::MegaDrive,
                    "msdos" | "dos" => mycelium_store::TargetPlatform::MSDOS,
                    "win98" | "win95" => mycelium_store::TargetPlatform::Windows98,
                    "arcade" | "mame" => mycelium_store::TargetPlatform::ArcadeMame,
                    "mac" | "ppc" => mycelium_store::TargetPlatform::PowerPCMac,
                    _ => mycelium_store::TargetPlatform::NativeSystem,
                };
                let spore = mycelium_store::SoftwareSpore {
                    id: id.clone(),
                    title,
                    description: "Publicado via daemon Mycelium Store".into(),
                    developer_or_publisher: "Comunidade Mycelium".into(),
                    release_year: 2000,
                    platform,
                    category: "software".into(),
                    tags: vec!["p2p".into(), "spore".into()],
                    license: mycelium_store::SporeLicense::Proprietary,
                    main_binary_file: format!("{}.bin", id),
                    content_id: mycelium_core::ContentId::of(id.as_bytes()),
                    execution_matrix: mycelium_store::ExecutionMatrix {
                        recommended: mycelium_store::ExecutionEngineType::Native,
                        supports_native: true,
                        libretro_core: None,
                        mame_driver: None,
                        qemu_config: None,
                        supports_wasm: true,
                        supports_p2p_stream: true,
                    },
                    requirements: mycelium_store::spore::HardwareRequirements::default(),
                    extra_args: vec![],
                    cover_image_url: None,
                };
                catalog.insert_spore(spore).map(|_| {
                    Response::Ok {
                        message: format!("spore '{}' registrado no catálogo", id),
                    }
                }).unwrap_or_else(|e| Response::Err {
                    message: format!("erro ao salvar catálogo: {}", e),
                })
            }
            Request::RepoPublish { message, leaves } => {
                let n = leaves.len();
                let bytes: usize = leaves.iter().map(|l| l.content.len()).sum();
                match self.publish_repo(message, leaves) {
                    Ok(id) => Response::RepoPublished {
                        cid: id.to_string(),
                        leaves: n,
                        bytes,
                    },
                    Err(e) => Response::Err {
                        message: e.to_string(),
                    },
                }
            }
            Request::RepoClone { cid } => match cid.parse::<ContentId>() {
                Ok(id) => match self.bank.recall(&id) {
                    Some(p) => Response::RepoCloneResult {
                        message: format!(
                            "repo {} — \"{}\" ({} leaves)",
                            id.short(),
                            p.message,
                            p.leaves.len()
                        ),
                        leaves: p.leaves.clone(),
                    },
                    None => {
                        self.hyphae.dht_get(dht_key(&id));
                        Response::Err {
                            message: format!(
                                "repo {} ausente localmente; consulta DHT disparada — tente de novo em alguns segundos",
                                id.short()
                            ),
                        }
                    }
                },
                Err(e) => Response::Err { message: e },
            },
            Request::Transfer {
                to,
                amount,
                nutrient,
                kind,
                memo,
                asset,
            } => {
                let nut = nutrient
                    .parse::<mycelium_core::Nutrient>()
                    .map_err(|_| format!("nutriente inválido: {}", nutrient));
                let kind = kind
                    .parse::<mycelium_nutrients::TxKind>()
                    .map_err(|_| format!("kind inválido: {}", kind));
                match (nut, kind) {
                    (Ok(n), Ok(k)) => match self.transfer(&to, amount, n, k, memo, asset) {
                        Ok(tx) => Response::TransferResult {
                            tx_id: tx.short(),
                            kind: k.as_str().to_string(),
                            nutrient: n.to_string(),
                            amount,
                            to,
                        },
                        Err(e) => Response::Err { message: e },
                    },
                    (Err(e), _) | (_, Err(e)) => Response::Err { message: e },
                }
            }
            Request::LedgerInfo => Response::LedgerReport {
                pubkey: self.wallet_pubkey_hex(),
                balances: self.ledger.balances.clone(),
                history: self.ledger.history.clone(),
                transfers: self.ledger.recent_transfers().to_vec(),
            },
            Request::AssetRegister {
                id,
                name,
                kind,
                description,
                location,
                shares_total,
                price_per_share,
            } => {
                let kind = kind
                    .parse::<crate::assets::AssetKind>()
                    .map_err(|_| format!("kind inválido: {}", kind));
                match kind {
                    Ok(k) => {
                        let record = crate::assets::AssetRecord {
                            id: id.clone(),
                            name,
                            kind: k,
                            description,
                            location,
                            shares_total,
                            price_per_share,
                            owner: self.wallet_pubkey(),
                        };
                        match self.assets.register(record) {
                            Ok(()) => {
                                self.assets.save(&self.home).ok();
                                Response::Ok {
                                    message: format!("ativo '{}' registado", id),
                                }
                            }
                            Err(e) => Response::Err { message: e },
                        }
                    }
                    Err(e) => Response::Err { message: e },
                }
            }
            Request::AssetList => Response::AssetListResult {
                assets: self.assets.assets.clone(),
            },
            Request::AssetShares { id } => Response::AssetSharesResult {
                asset: id.clone(),
                holdings: self.assets.holdings_of(&id),
            },
            Request::AssetTransfer { asset, shares, to } => {
                let to = hex::decode(&to)
                    .map_err(|e| format!("pubkey destino inválida: {e}"))
                    .and_then(|v| v.try_into().map_err(|_| "pubkey precisa 32 bytes".to_string()));
                match to {
                    Ok(to_pubkey) => match self.assets.transfer_shares(&asset, &self.wallet_pubkey(), &to_pubkey, shares) {
                        Ok(()) => {
                            self.assets.save(&self.home).ok();
                            Response::Ok {
                                message: format!("{} cotas transferidas", shares),
                            }
                        }
                        Err(e) => Response::Err { message: e },
                    },
                    Err(e) => Response::Err { message: e },
                }
            }
            Request::CompanyRegister { name, shares_total } => {
                let name_for_record = name.clone();
                let record = crate::assets::AssetRecord {
                    id: name.clone(),
                    name: name_for_record,
                    kind: crate::assets::AssetKind::Company,
                    description: "Empresa/cooperativa (Fase 4)".into(),
                    location: None,
                    shares_total,
                    price_per_share: 1,
                    owner: self.wallet_pubkey(),
                };
                match self.assets.register(record) {
                    Ok(()) => {
                        self.assets.save(&self.home).ok();
                        Response::Ok {
                            message: format!("empresa '{}' registada", name),
                        }
                    }
                    Err(e) => Response::Err { message: e },
                }
            }
            Request::CompanyPayout { name, total } => {
                let holdings = self.assets.holdings_of(&name);
                let total_shares: u64 = holdings.iter().map(|h| h.shares).sum();
                if total_shares == 0 {
                    return Response::Err {
                        message: "nenhuma cota emitida".into(),
                    };
                }
                let per_share = total / total_shares;
                let mut paid = 0;
                for h in holdings {
                    let share = h.shares * per_share;
                    self.ledger.feed_kind(
                        mycelium_core::Nutrient::Atp,
                        share,
                        format!("dividendo empresa {}", name),
                        Some(mycelium_nutrients::TxKind::Revenue),
                    );
                    paid += share;
                }
                Response::Ok {
                    message: format!("distribuído {} ATP como dividendo", paid),
                }
            }
            Request::Shutdown => Response::Ok {
                message: "encerrando".into(),
            },
        }
    }

    pub async fn run(mut self, mut control_rx: mpsc::Receiver<ControlMsg>) -> Result<(), OrganismError> {
        self.store.write_pid()?;

        let bind: std::net::SocketAddr =
            format!("127.0.0.1:{}", self.state.horizon_port)
                .parse()
                .map_err(|e| OrganismError::Msg(format!("{e}")))?;
        let handle = match serve_horizon(bind, self.horizon.clone()).await {
            Ok(h) => h,
            Err(e) if e.contains("Address already in use") || e.contains("os error 98") => {
                tracing::warn!(
                    port = self.state.horizon_port,
                    "Event Horizon ocupado — a usar porta efémera (127.0.0.1:0)"
                );
                let fallback: std::net::SocketAddr = "127.0.0.1:0"
                    .parse()
                    .map_err(|e| OrganismError::Msg(format!("{e}")))?;
                serve_horizon(fallback, self.horizon.clone())
                    .await
                    .map_err(OrganismError::Msg)?
            }
            Err(e) => return Err(OrganismError::Msg(e)),
        };
        self.state.horizon_port = handle.bind.port();
        tracing::info!(
            url = %format!("http://{}/", handle.bind),
            "event horizon escutando — curl http://127.0.0.1:{}/<ion>/",
            self.state.horizon_port
        );
        self.horizon_handle = Some(handle);

        // Expõe a Mycelium Store UI + API como um ion no Event Horizon.
        let store_catalog = self.catalog.clone();
        let store_home = self.home.clone();
        let store_horizon = self.horizon.clone();
        tokio::spawn(async move {
            let store_router = mycelium_store::create_store_router(&store_home, store_catalog);
            let ui_router = mycelium_store::create_store_ui_router(&store_home);
            let app = store_router.merge(ui_router);
            let store_bind: std::net::SocketAddr = "127.0.0.1:0"
                .parse()
                .expect("bind valid");
            let listener = match tokio::net::TcpListener::bind(store_bind).await {
                Ok(l) => l,
                Err(e) => {
                    tracing::error!(error = %e, "falha ao escutar store server");
                    return;
                }
            };
            let local = listener.local_addr().expect("local addr");
            tracing::info!(
                url = %format!("http://127.0.0.1:{}/store/", local),
                "mycelium store server escutando"
            );
            {
                let mut table = store_horizon.write().unwrap();
                table.expose(
                    "store.mycelium",
                    Orbit {
                        ion: "store".into(),
                        node: NodeId::derive(b"store"),
                        mass: 100,
                        resistance: 0,
                        upstream: format!("http://{}", local),
                    },
                );
            }
            let _ = axum::serve(listener, app).await;
        });

        // Expõe o browser de código soberano (ion `src`) no Event Horizon.
        let src_home = self.home.clone();
        let src_horizon = self.horizon.clone();
        tokio::spawn(async move {
            let app = crate::src_ion::create_src_router(&src_home);
            let src_bind: std::net::SocketAddr = "127.0.0.1:0"
                .parse()
                .expect("bind valid");
            let listener = match tokio::net::TcpListener::bind(src_bind).await {
                Ok(l) => l,
                Err(e) => {
                    tracing::error!(error = %e, "falha ao escutar src server");
                    return;
                }
            };
            let local = listener.local_addr().expect("local addr");
            tracing::info!(
                url = %format!("http://127.0.0.1:{}/src/", local),
                "mycelium src ion escutando"
            );
            {
                let mut table = src_horizon.write().unwrap();
                table.expose(
                    "src.mycelium",
                    Orbit {
                        ion: "src".into(),
                        node: NodeId::derive(b"src"),
                        mass: 100,
                        resistance: 0,
                        upstream: format!("http://{}", local),
                    },
                );
            }
            let _ = axum::serve(listener, app).await;
        });

        // Expõe o catálogo de seeds públicas/privadas (ion `seeds`) no Event Horizon.
        let seeds_home = self.home.clone();
        let seeds_horizon = self.horizon.clone();
        tokio::spawn(async move {
            let app = crate::seeds_ion::create_seeds_router(&seeds_home);
            let seeds_bind: std::net::SocketAddr = "127.0.0.1:0"
                .parse()
                .expect("bind valid");
            let listener = match tokio::net::TcpListener::bind(seeds_bind).await {
                Ok(l) => l,
                Err(e) => {
                    tracing::error!(error = %e, "falha ao escutar seeds server");
                    return;
                }
            };
            let local = listener.local_addr().expect("local addr");
            tracing::info!(
                url = %format!("http://127.0.0.1:{}/seeds/", local),
                "mycelium seeds ion escutando"
            );
            {
                let mut table = seeds_horizon.write().unwrap();
                table.expose(
                    "seeds.mycelium",
                    Orbit {
                        ion: "seeds".into(),
                        node: NodeId::derive(b"seeds"),
                        mass: 100,
                        resistance: 0,
                        upstream: format!("http://{}", local),
                    },
                );
            }
            let _ = axum::serve(listener, app).await;
        });

        let pheromone = self
            .gland
            .secrete_membrane(Trail::default(), Duration::from_secs(3600), self.membrane)
            .map_err(|e| OrganismError::Msg(e.to_string()))?;
        let pheromone_bytes =
            serde_json::to_vec(&pheromone).map_err(|e| OrganismError::Msg(e.to_string()))?;
        let mut secreted = false;
        let mut persist_tick = tokio::time::interval(Duration::from_secs(15));
        let mut heartbeat = tokio::time::interval(Duration::from_secs(3600));
        let mut seed_tick = tokio::time::interval(Duration::from_secs(120));
        let mut duckdns_tick = tokio::time::interval(Duration::from_secs(300));
        let mut physarum_tick = tokio::time::interval(Duration::from_secs(5));
        let mut nostr_tick = tokio::time::interval(Duration::from_secs(45));
        let mut metrics_tick = tokio::time::interval(Duration::from_secs(30));
        let mut balance_tick = tokio::time::interval(Duration::from_secs(60));
        let mut zone_tick = tokio::time::interval(Duration::from_secs(120));
        // Primeiro tick imediato já foi coberto na germinação; atrasa o próximo.
        seed_tick.tick().await;
        // DuckDNS: espera um pouco para ter listen addrs.
        duckdns_tick.tick().await;
        physarum_tick.tick().await;
        nostr_tick.tick().await;
        metrics_tick.tick().await;
        balance_tick.tick().await;
        zone_tick.tick().await;

        if self.sporocarp {
            tracing::info!("sporocarp ativo — relay + DNS (se DUCKDNS_*) — sem UPnP");
        }
        tracing::info!(
            membrane = %self.membrane,
            "política de membrana"
        );

        tracing::info!(
            node = %self.gland.node_id().short(),
            peer = %self.hyphae.peer_id(),
            "organismo despertou"
        );

        loop {
            tokio::select! {
                biased;

                msg = control_rx.recv() => {
                    match msg {
                        Some(ControlMsg { request, reply }) => {
                            let shutdown = matches!(request, Request::Shutdown);
                            let resp = self.handle_control(request);
                            let _ = reply.send(resp);
                            if shutdown {
                                break;
                            }
                        }
                        None => break,
                    }
                }

                _ = persist_tick.tick() => {
                    let _ = self.persist();
                }

                _ = physarum_tick.tick() => {
                    self.physarum_tick(0.5);
                }

                _ = nostr_tick.tick() => {
                    #[cfg(feature = "nostr-transport")]
                    if self.enable_nostr_transport {
                        let relay = self.nostr_relay.clone();
                        match self
                            .hyphae
                            .nostr_discover_and_dial(&relay, &mut self.nostr_dialed)
                            .await
                        {
                            Ok(n) if n > 0 => {
                                tracing::info!(dialed = n, "nostr-transport: peers dialados")
                            }
                            Ok(_) => {}
                            Err(e) => tracing::debug!(error = %e, "nostr-transport discover"),
                        }
                    }
                    #[cfg(not(feature = "nostr-transport"))]
                    {
                        let _ = self.enable_nostr_transport;
                    }
                }

                _ = metrics_tick.tick() => {
                    let report = self.status_report();
                    let mut prom = String::new();
                    prom.push_str(&format!("# HELP mycelium_neighbors Número de vizinhos\n"));
                    prom.push_str(&format!("# TYPE mycelium_neighbors gauge\n"));
                    prom.push_str(&format!("mycelium_neighbors {}\n", report.neighbors));
                    prom.push_str(&format!("# HELP mycelium_plots Plots no Spore Bank\n"));
                    prom.push_str(&format!("# TYPE mycelium_plots gauge\n"));
                    prom.push_str(&format!("mycelium_plots {}\n", report.plots));
                    prom.push_str(&format!("# HELP mycelium_signals Signals no TheField\n"));
                    prom.push_str(&format!("# TYPE mycelium_signals gauge\n"));
                    prom.push_str(&format!("mycelium_signals {}\n", report.signals));
                    prom.push_str(&format!("# HELP mycelium_ions Ions em órbita\n"));
                    prom.push_str(&format!("# TYPE mycelium_ions gauge\n"));
                    prom.push_str(&format!("mycelium_ions {}\n", report.ions.len()));
                    prom.push_str(&format!("# HELP mycelium_atp Saldo de ATP\n"));
                    prom.push_str(&format!("# TYPE mycelium_atp gauge\n"));
                    prom.push_str(&format!("mycelium_atp {}\n", report.atp));
                    prom.push_str(&format!("# HELP mycelium_enzymes Saldo de Enzymes\n"));
                    prom.push_str(&format!("# TYPE mycelium_enzymes gauge\n"));
                    prom.push_str(&format!("mycelium_enzymes {}\n", report.enzymes));
                    prom.push_str(&format!("# HELP mycelium_mycelia Saldo de Mycelia\n"));
                    prom.push_str(&format!("# TYPE mycelium_mycelia gauge\n"));
                    prom.push_str(&format!("mycelium_mycelia {}\n", report.mycelia));
                    prom.push_str(&format!("# HELP mycelium_spores Saldo de Spores\n"));
                    prom.push_str(&format!("# TYPE mycelium_spores gauge\n"));
                    prom.push_str(&format!("mycelium_spores {}\n", report.spores));
                    prom.push_str(&format!("# HELP mycelium_resilience Saldo de Resilience\n"));
                    prom.push_str(&format!("# TYPE mycelium_resilience gauge\n"));
                    prom.push_str(&format!("mycelium_resilience {}\n", report.resilience));
                    prom.push_str(&format!("# HELP mycelium_anastomoses Conexões totais formadas\n"));
                    prom.push_str(&format!("# TYPE mycelium_anastomoses counter\n"));
                    prom.push_str(&format!("mycelium_anastomoses {}\n", report.anastomoses));
                    prom.push_str(&format!("# HELP mycelium_messages_in Mensagens gossip recebidas\n"));
                    prom.push_str(&format!("# TYPE mycelium_messages_in counter\n"));
                    prom.push_str(&format!("mycelium_messages_in {}\n", report.messages_in));
                    prom.push_str(&format!("# HELP mycelium_messages_out Mensagens gossip enviadas\n"));
                    prom.push_str(&format!("# TYPE mycelium_messages_out counter\n"));
                    prom.push_str(&format!("mycelium_messages_out {}\n", report.messages_out));
                    prom.push_str(&format!("# HELP mycelium_isotope_atoms Atoms no Nucleus\n"));
                    prom.push_str(&format!("# TYPE mycelium_isotope_atoms gauge\n"));
                    prom.push_str(&format!("mycelium_isotope_atoms {}\n", report.isotope_atoms));
                    prom.push_str(&format!("# HELP mycelium_membrane Membrana atual\n"));
                    prom.push_str(&format!("# TYPE mycelium_membrane gauge\n"));
                    prom.push_str(&format!("mycelium_membrane{{membrane=\"{}\"}} 1\n", report.membrane));
                    prom.push_str(&format!("# HELP mycelium_physarum_phase Fase Physarum\n"));
                    prom.push_str(&format!("# TYPE mycelium_physarum_phase gauge\n"));
                    prom.push_str(&format!("mycelium_physarum_phase{{phase=\"{}\"}} 1\n", report.physarum_phase));
                    prom.push_str(&format!("# HELP mycelium_uptime_segundos Uptime do ledger (heartbeat)\n"));
                    prom.push_str(&format!("# TYPE mycelium_uptime_segundos counter\n"));
                    prom.push_str(&format!("mycelium_uptime_hours 1\n"));
                    let mut table = self.horizon.write().unwrap();
                    table.set_metrics(prom);
                }

                _ = balance_tick.tick() => {
                    let clock = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    let mut balances = HashMap::new();
                    for n in Nutrient::ALL {
                        balances.insert(n, self.ledger.balance(n));
                    }
                    let env = Envelope::BalanceSync {
                        node_id: self.gland.node_id(),
                        balances,
                        clock,
                    };
                    if let Ok(bytes) = env.encode() {
                        let _ = self.hyphae.broadcast_lattice(bytes);
                    }
                }

                _ = zone_tick.tick() => {
                    if !self.state.ions.is_empty() {
                        let prefix = format!("Qm{}", self.gland.node_id().short());
                        let env = Envelope::ZoneAnnounce {
                            prefix,
                            custodian: self.gland.node_id(),
                        };
                        if let Ok(bytes) = env.encode() {
                            let _ = self.hyphae.broadcast_lattice(bytes);
                        }
                    }
                }

                _ = heartbeat.tick() => {
                    self.ledger.heartbeat(1);
                    let _ = self.store.save_ledger(&self.ledger);
                }

                _ = duckdns_tick.tick() => {
                    if self.sporocarp {
                        let hyphae_addr = self.hyphae.best_public_addr().map(|a| {
                            with_membrane_flag(&a.to_string(), self.membrane)
                        });
                        let token = std::env::var("DUCKDNS_TOKEN").ok();
                        let domain = std::env::var("DUCKDNS_DOMAIN").ok();
                        if let (Some(token), Some(domain), Some(txt)) = (token, domain, hyphae_addr) {
                            tokio::task::spawn_blocking(move || {
                                if let Err(e) = SeedBook::publish_duckdns_txt(&domain, &token, &txt) {
                                    tracing::warn!("DuckDNS publish: {e}");
                                }
                            });
                        }
                    }
                }

                _ = seed_tick.tick() => {
                    let addrs = self.seed_book.multiaddrs_for(self.membrane);
                    if !addrs.is_empty() {
                        let n = self.hyphae.reach_seeds(&addrs);
                        if n > 0 {
                            tracing::debug!(reached = n, "re-bootstrap de seeds");
                        }
                    }
                    // Relay mesh: esporocarp alcançável anuncia; folhas tentam circuit.
                    if self.assume_reachable && (self.sporocarp || matches!(self.membrane, Membrane::Esporocarp)) {
                        if let Err(e) = self.hyphae.publish_relay_mesh_ad() {
                            tracing::debug!("relay mesh ad: {e}");
                        }
                    } else if !self.sporocarp {
                        self.hyphae.try_mesh_relay_circuits();
                    }
                    self.hyphae.mailbox_poll();
                    // Reinicia chambers mortas.
                    let dead: Vec<String> = {
                        let mut names = Vec::new();
                        for (name, chamber) in self.chambers.iter_mut() {
                            if !chamber.healthy() {
                                names.push(name.clone());
                            }
                        }
                        names
                    };
                    for name in dead {
                        if let Some(c) = self.chambers.get_mut(&name) {
                            if let Err(e) = c.awaken() {
                                tracing::warn!(ion = %name, "awaken falhou: {e}");
                            } else if let Some(proc) = self.chambers.get(&name) {
                                let host = format!(
                                    "sporocarp.mycelium/{}",
                                    self.gland.node_id().short()
                                );
                                let mut table = self.horizon.write().unwrap();
                                table.expose(
                                    &host,
                                    Orbit {
                                        ion: name.clone(),
                                        node: self.gland.node_id(),
                                        mass: self.resources.cpu_cores as u64 * 10 + 1,
                                        resistance: 0,
                                        upstream: proc.upstream.clone(),
                                    },
                                );
                            }
                        }
                    }
                }

                event = self.hyphae.pulse() => {
                    match event {
                        Some(HyphaEvent::Rooted { address }) => {
                            tracing::info!(%address, "enraizado");
                            let _ = self.persist();
                        }
                        Some(HyphaEvent::SporocarpCircuit { src, dst }) => {
                            if self.sporocarp {
                                self.ledger.feed(
                                    Nutrient::Atp,
                                    1,
                                    format!("sporocarp-relay:{src}->{dst}"),
                                );
                                self.ledger.feed(Nutrient::Spores, 1, "sporocarp-relay");
                                let _ = self.store.save_ledger(&self.ledger);
                            }
                        }
                        Some(HyphaEvent::NeighborSniffed { peer })
                        | Some(HyphaEvent::Anastomosis { peer }) => {
                            tracing::info!(%peer, "hifa viva");
                            if !secreted {
                                if let Ok(true) = self.hyphae.secrete(pheromone_bytes.clone()) {
                                    secreted = true;
                                }
                                for id in self.bank.ids().to_vec() {
                                    if let Ok(bytes) = self.bank.spore_print(&id) {
                                        if let Ok(plot) = serde_json::from_slice::<Plot>(&bytes) {
                                            let env = Envelope::SporePrint { plot };
                                            if let Ok(encoded) = env.encode() {
                                                let _ = self.hyphae.broadcast_lattice(encoded);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Some(HyphaEvent::Atrophy { peer }) => {
                            tracing::debug!(%peer, "hifa atrofiada");
                        }
                        Some(HyphaEvent::LatticeReceived { data, .. }) => {
                            match Envelope::decode(&data) {
                                Ok(env) => {
                                    if let Err(e) = self.handle_envelope(env) {
                                        tracing::warn!("envelope: {e}");
                                    }
                                }
                                Err(e) => tracing::warn!("envelope inválido: {e}"),
                            }
                        }
                        Some(HyphaEvent::PheromoneReceived { .. }) => {}
                        Some(HyphaEvent::RecordFound { key, value }) => {
                            if key.starts_with(RELAY_DHT_PREFIX) {
                                if let Ok(adv) =
                                    serde_json::from_slice::<RelayAdvertisement>(&value)
                                {
                                    self.hyphae.ingest_relay_ad(adv);
                                    if !self.sporocarp {
                                        self.hyphae.try_mesh_relay_circuits();
                                    }
                                }
                            } else if key.starts_with(MAILBOX_DHT_PREFIX) {
                                if let Ok(msg) = serde_json::from_slice::<MailboxMessage>(&value) {
                                    if mycelium_hyphae::is_expired(&msg) {
                                        tracing::debug!(id = %msg.id_hex, "mailbox expirada");
                                    } else if msg.to == self.hyphae.peer_id().to_string() {
                                        tracing::info!(
                                            from = %msg.from,
                                            id = %msg.id_hex,
                                            ctype = ?msg.content_type,
                                            "mailbox DHT"
                                        );
                                        if let Err(e) = self.hyphae.mailbox_ack(&msg.id_hex) {
                                            tracing::debug!("mailbox ack: {e}");
                                        }
                                        // Entrega mínima: Generic → log; IsotopeAtom → absorb se Atom JSON
                                        if matches!(
                                            msg.content_type,
                                            mycelium_hyphae::MailboxContentType::IsotopeAtom
                                        ) {
                                            if let Ok(frame) =
                                                serde_json::from_slice::<(String, Atom)>(
                                                    &msg.payload,
                                                )
                                            {
                                                let (key, atom) = frame;
                                                self.nucleus.absorb(&key, atom);
                                                let _ = self.persist();
                                            } else if let Ok(atom) =
                                                serde_json::from_slice::<Atom>(&msg.payload)
                                            {
                                                // Payload legado sem chave — ignora absorb.
                                                tracing::debug!(
                                                    clock = atom.clock,
                                                    "mailbox isotope sem chave"
                                                );
                                            }
                                        }
                                    }
                                }
                            } else if let Some(id) =
                                mycelium_sporebank::content_id_from_dht_key(&key)
                            {
                                match self.bank.absorb(&value) {
                                    Ok(_) => tracing::info!(plot = %id.short(), "esporo recuperado do DHT"),
                                    Err(e) => tracing::warn!("absorb DHT: {e}"),
                                }
                                let _ = self.persist();
                            } else if let Some(layer_id) = content_id_from_layer_dht_key(&key) {
                                match LayerStore::open(self.store.layers_dir()) {
                                    Ok(store) => match store.put(&value) {
                                        Ok(stored) => {
                                            tracing::info!(
                                                layer = %stored.short(),
                                                expected = %layer_id.short(),
                                                "layer recuperada do DHT"
                                            );
                                        }
                                        Err(e) => tracing::warn!("layer DHT put: {e}"),
                                    },
                                    Err(e) => tracing::warn!("layer store: {e}"),
                                }
                                let _ = self.persist();
                            }
                        }
                        Some(HyphaEvent::RecordNotFound { key }) => {
                            tracing::debug!(key = %hex::encode(&key), "DHT miss");
                        }
                        Some(HyphaEvent::NeighborEvaporated { .. }) | None => {}
                    }
                }
            }
        }

        // Decompõe chambers (Drop também mata, mas explícito é mais claro).
        for (_, mut c) in self.chambers.drain() {
            c.decompose();
        }
        if let Some(h) = self.horizon_handle.take() {
            h.shutdown();
        }
        self.persist()?;
        self.store.clear_runtime_files();
        tracing::info!("organismo hibernou — estado persistido");
        Ok(())
    }
}

/// GhostID determinístico da carteira do nó: derivado do seed do gland
/// (estável por nó, efémero pela camada de assinatura NIP-01).
fn ghost_for_node(gland_seed: [u8; 32]) -> mycelium_ghostid::GhostId {
    let seed = mycelium_core::ContentId::of(b"mycelium-value-layer-v1")
        .0
        .iter()
        .zip(gland_seed.iter())
        .map(|(a, b)| a ^ b)
        .collect::<Vec<u8>>()
        .try_into()
        .unwrap_or(gland_seed);
    mycelium_ghostid::GhostId::from_secret_bytes(seed, 60 * 60 * 24 * 365 * 100)
        .unwrap_or_else(|_| mycelium_ghostid::GhostId::spawn_quick(60 * 60 * 24 * 365).unwrap())
}
