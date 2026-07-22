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
    #[error("{0}")]
    Msg(String),
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
        // Primeiro tick imediato já foi coberto na germinação; atrasa o próximo.
        seed_tick.tick().await;
        // DuckDNS: espera um pouco para ter listen addrs.
        duckdns_tick.tick().await;
        physarum_tick.tick().await;
        nostr_tick.tick().await;

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
