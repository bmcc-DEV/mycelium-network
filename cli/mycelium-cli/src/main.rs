//! # mycelium — CLI do substrato vivo

use axum::routing::get;
use axum::{Json, Router};
use clap::{Parser, Subcommand};
use mycelium_core::Resources;
use mycelium_hyphae::{SeedBook, DEFAULT_BOOTSTRAP_URL, DEFAULT_DNS_SEED_NAME};
use mycelium_node::{call, run_daemon, DaemonOptions, NodeStore, Request, Response};
use serde_json::json;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Parser)]
#[command(
    name = "mycelium",
    about = "Mycelium Network — o substrato vivo do The Lattice",
    version
)]
struct Cli {
    #[arg(long, global = true, env = "MYCELIUM_HOME")]
    home: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Sprout {
        #[arg(long, default_value = "1cpu,1gb,10gb")]
        contribute: String,
    },
    Daemon {
        #[arg(long, default_value = "1cpu,1gb,10gb")]
        contribute: String,
        /// Seed/bootstrap multiaddr (repetível). Aceita `/dnsaddr/...`.
        #[arg(long = "bootstrap")]
        bootstrap: Vec<String>,
        /// Arquivo local de seeds (uma multiaddr por linha).
        #[arg(long = "seed-file")]
        seed_file: Option<PathBuf>,
        /// Baixa o catálogo público de seeds (além da LAN).
        #[arg(long = "public-bootstrap")]
        public_bootstrap: bool,
        /// URL do catálogo (default: github mycelium-network/seeds).
        #[arg(long = "bootstrap-url")]
        bootstrap_url: Option<String>,
        /// Multiaddr de escuta (repetível). Ex.: `/ip4/0.0.0.0/tcp/4001`
        #[arg(long = "listen")]
        listen: Vec<String>,
        /// Porta do Event Horizon HTTP (Singularity).
        #[arg(long, default_value_t = 7474)]
        horizon_port: u16,
        /// Desliga mDNS — discovery só via seed book / --bootstrap.
        #[arg(long = "no-mdns")]
        no_mdns: bool,
        /// IP público anunciado (quando listen é 0.0.0.0). Env: MYCELIUM_ANNOUNCE_IP.
        #[arg(long = "announce-ip", env = "MYCELIUM_ANNOUNCE_IP")]
        announce_ip: Option<String>,
        /// IPv6 público anunciado (quando listen é `::`). Env: MYCELIUM_ANNOUNCE_IP6.
        #[arg(long = "announce-ip6", env = "MYCELIUM_ANNOUNCE_IP6")]
        announce_ip6: Option<String>,
        /// Opera como circuit relay v2 (seed público). Gera control.token se sem env.
        #[arg(long = "relay")]
        relay: bool,
        /// Volunteer Sporocarp: relay + publish DNS TXT + crédito ATP.
        #[arg(long = "sporocarp")]
        sporocarp: bool,
        /// Override da membrana (floresta|raiz|folha|esporocarp).
        #[arg(long = "membrane", value_parser = parse_membrane)]
        membrane: Option<mycelium_core::Membrane>,
        /// Declara inbound TCP/QUIC alcançável (auto-esporocarp se IPv6/announce).
        /// Env: MYCELIUM_REACHABLE=1
        #[arg(long = "assume-reachable", env = "MYCELIUM_REACHABLE")]
        assume_reachable: bool,
        /// Escuta webrtc-direct (requer `cargo build --features webrtc`).
        #[arg(long = "webrtc")]
        webrtc: bool,
        /// Porta UDP webrtc-direct.
        #[arg(long = "webrtc-port", default_value_t = 4002)]
        webrtc_port: u16,
        /// Transporte libp2p sobre Nostr (força ON). Sem flag: auto em folha/floresta.
        #[arg(long = "nostr-transport", env = "MYCELIUM_NOSTR_TRANSPORT")]
        nostr_transport: bool,
        /// Desliga Nostr transport (mesmo em folha/floresta).
        #[arg(long = "no-nostr-transport", conflicts_with = "nostr_transport")]
        no_nostr_transport: bool,
        /// Relay Nostr WSS para o transporte (default nos.lol).
        #[arg(long = "nostr-relay", env = "MYCELIUM_NOSTR_RELAY")]
        nostr_relay: Option<String>,
        /// Depreciado: ignorado (Política de Membrana — sem UPnP).
        #[arg(long = "upnp")]
        upnp: bool,
    },
    Status,
    Sow {
        #[arg(long, default_value = "init")]
        message: String,
        #[arg(long, default_value = "main.rs")]
        path: String,
        #[arg(long, default_value = "fn main() {}")]
        content: String,
        /// Fragmenta com QEL (formato k,n — default 3,7). Requer --features nostr.
        #[arg(long, value_name = "K,N")]
        qel: Option<String>,
        /// Publica anúncio NIP-94 + shards via relays Nostr (wss:// outbound).
        #[arg(long)]
        nostr: bool,
        /// Usa GhostID efémero secp256k1 para assinar eventos Nostr.
        #[arg(long)]
        ghost: bool,
        /// Pubkey Nostr hex do destinatário (NIP-44); sem isto shards vão em plaintext assinado.
        #[arg(long = "to")]
        recipient: Option<String>,
        /// Hybrid Theory: QEL + Nostr + blockstore local (ipfs-blocks/).
        #[arg(long)]
        hybrid: bool,
    },
    Signal {
        #[arg(long)]
        plot: String,
        #[arg(long, default_value_t = 1)]
        quorum: usize,
        #[arg(long, default_value = "webapp")]
        ion: String,
        #[arg(long, default_value = "ci")]
        name: String,
    },
    Resonate {
        #[arg(long)]
        signal: String,
    },
    Recall {
        #[arg(long)]
        plot: String,
        /// Reconstrói via shards QEL (Nostr).
        #[arg(long)]
        qel: bool,
        /// Busca shards em relays Nostr.
        #[arg(long)]
        nostr: bool,
        /// Threshold QEL (default 3).
        #[arg(long, default_value_t = 3)]
        qel_threshold: u8,
        /// Hybrid: local → Nostr → blockstore ipfs local.
        #[arg(long)]
        hybrid: bool,
    },
    Bootstrap {
        #[arg(long)]
        addr: String,
    },
    /// Gerencia o seed book local (bootstrap público).
    Seeds {
        #[command(subcommand)]
        action: SeedsCmd,
    },
    /// Escreve estado no Isotope (propaga por hifas).
    IsotopePut {
        #[arg(long)]
        key: String,
        #[arg(long)]
        value: String,
        #[arg(long)]
        clock: Option<u64>,
    },
    /// Lê estado do Isotope (local ou Decay pelas hifas).
    IsotopeGet {
        #[arg(long)]
        key: String,
    },
    /// One-shot: sow → signal → espera ion no Horizon (fluxo do manifesto).
    Deploy {
        #[arg(long)]
        plot: Option<String>,
        #[arg(long, default_value = "init")]
        message: String,
        #[arg(long, default_value = "build.sh")]
        path: String,
        #[arg(long, default_value = "#!/bin/sh\nmkdir -p dist\necho ok > dist/index.html\n")]
        content: String,
        #[arg(long, default_value = "webapp")]
        ion: String,
        #[arg(long, default_value = "ci")]
        name: String,
        #[arg(long, default_value_t = 1)]
        quorum: usize,
        /// Segundos máximos à espera do ion.
        #[arg(long, default_value_t = 30)]
        timeout: u64,
    },
    Shutdown,
    /// Mostra balance local + de peers.
    Balance,
    /// Migra um Ion para outro nó.
    IonMigrate {
        #[arg(long)]
        ion: String,
        #[arg(long)]
        target: String,
    },
    /// Mostra zonas de crescimento conhecidas.
    Zones,
    /// Entropy: Shamir Secret Sharing com meia-vida.
    Entropy {
        #[command(subcommand)]
        action: EntropyCmd,
    },
    /// CandidateRelay (kind 39401/39406): descoberta + backchannel CGNAT↔CGNAT.
    Candidate {
        #[command(subcommand)]
        cmd: Option<CandidateCmd>,
        /// Repetir com jitter 30–300s (só em discover sem subcomando).
        #[arg(long)]
        r#loop: bool,
        /// Uma ronda e sai (default se sem --loop).
        #[arg(long)]
        once: bool,
        /// Relays wss:// (repetível). Default = pool público.
        #[arg(long = "relay")]
        relays: Vec<String>,
    },
    #[command(hide = true)]
    ChamberServe {
        #[arg(long)]
        port: u16,
        #[arg(long)]
        ion: String,
        #[arg(long)]
        root: PathBuf,
    },
    /// App Store / Steam P2P de jogos e software antigo/legado
    Store {
        #[command(subcommand)]
        action: StoreCmd,
    },
    /// Distribuição de código soberana via P2P (sem GitHub)
    Repo {
        #[command(subcommand)]
        action: RepoCmd,
    },
}

#[derive(Subcommand)]
enum StoreCmd {
    /// Lista jogos e softwares legados cadastrados no catálogo P2P
    List,
    /// Mostra as capacidades de emulação do sistema host (QEMU, MAME, RetroArch, bwrap)
    Caps,
    /// Executa um jogo ou software legado por ID
    Launch {
        #[arg(long)]
        id: String,
        /// Força o motor de execução (native, retroarch, mame, qemu, wasm, cloud)
        #[arg(long)]
        engine: Option<String>,
        /// Sandboxing estrito com bubblewrap
        #[arg(long, default_value_t = false)]
        sandbox: bool,
    },
    /// Publica uma nova ROM / software legado no SporeBank
    Publish {
        #[arg(long)]
        id: String,
        #[arg(long)]
        title: String,
        #[arg(long)]
        platform: String,
        #[arg(long)]
        binary: PathBuf,
        /// Licença de distribuição (shareware, freeware, open_source, public_domain, proprietary).
        /// Default: proprietary (só local / BYOR — não distribuído na rede)
        #[arg(long, default_value = "proprietary")]
        license: String,
    },
}

#[derive(Subcommand)]
enum RepoCmd {
    /// Publica uma árvore de código como Plot multi-leaf no SporeBank (DHT + gossip)
    Publish {
        /// Diretório raiz do repositório a publicar.
        #[arg(short, long)]
        dir: PathBuf,
        /// Mensagem/descrição do commit (ex.: "v0.1.0 — store P2P").
        #[arg(short, long, default_value = "mycelium-launcher-store")]
        message: String,
    },
    /// Reconstrói uma árvore de código a partir de um ContentId
    Clone {
        /// ContentId (Qm…) do repo publicado.
        #[arg(long)]
        cid: String,
        /// Diretório de destino da árvore reconstruída.
        #[arg(short, long)]
        dest: PathBuf,
    },
    /// Lista os repos disponíveis no SporeBank local
    List,
}

#[derive(Subcommand)]
enum CandidateCmd {
    /// Escuta mensagens backchannel (NIP-44, kind 39406) e re-anuncia presença.
    Listen {
        #[arg(long)]
        r#loop: bool,
    },
    /// Envia texto cifrado a um ghost peer (`--to` = pubkey hex 64 chars).
    Send {
        #[arg(long)]
        to: String,
        #[arg(short = 'm', long)]
        message: String,
    },
    /// Mostra o GhostID da sessão local (para o outro lado usar em `--to`).
    Whoami,
    /// Apaga `candidate.session` (novo GhostID na próxima vez).
    Reset,
}

#[derive(Subcommand)]
enum EntropyCmd {
    /// Fragmenta um segredo em N Shades.
    Shatter {
        #[arg(short, long)]
        secret: String,
        #[arg(short = 'k', long, default_value_t = 3)]
        threshold: u8,
        #[arg(short = 'n', long, default_value_t = 5)]
        total: u8,
    },
    /// Reconstrói o segredo a partir das Shades em custódia.
    Reconstruct {
        #[arg(short = 'k', long, default_value_t = 3)]
        threshold: u8,
    },
    /// Mostra as Shades armazenadas.
    Status,
}

#[derive(Subcommand)]
enum SeedsCmd {
    /// Lista seeds em `{home}/seeds.txt`.
    List,
    /// Adiciona uma multiaddr ao seed book.
    Add { addr: String },
    /// Baixa o catálogo público e mescla no seed book.
    Fetch {
        #[arg(long)]
        url: Option<String>,
        /// Nome DNS TXT do Spore Bank. Sem valor → default `_mycelium.seeds.duckdns.org`.
        #[arg(long, num_args = 0..=1, default_missing_value = DEFAULT_DNS_SEED_NAME)]
        dns: Option<String>,
    },
    /// Catálogo estruturado de seeds públicas e privadas (`{home}/seeds/catalog.json`).
    Catalog {
        #[command(subcommand)]
        action: SeedCatalogCmd,
    },
}

#[derive(Subcommand)]
enum SeedCatalogCmd {
    /// Lista seeds do catálogo.
    List {
        /// Filtro de visibilidade: public | private | all.
        #[arg(long, default_value = "all")]
        visibility: String,
    },
    /// Adiciona um seed ao catálogo.
    Add {
        #[arg(long)]
        id: Option<String>,
        #[arg(long)]
        name: String,
        #[arg(long)]
        multiaddr: String,
        /// public | private
        #[arg(long, default_value = "public")]
        visibility: String,
        #[arg(long)]
        membrane: Option<String>,
        #[arg(long)]
        region: Option<String>,
        #[arg(long)]
        operator: Option<String>,
        /// Opera como circuit relay v2.
        #[arg(long, default_value_t = false)]
        relay: bool,
        /// Inbound verificado.
        #[arg(long, default_value_t = false)]
        verified: bool,
    },
    /// Remove um seed do catálogo por id.
    Remove {
        #[arg(long)]
        id: String,
    },
    /// Exporta as seeds públicas no formato `seeds/mainnet.txt`.
    Export {
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

fn main() {
    let cli = Cli::parse();

    let filter = if matches!(cli.command, Commands::ChamberServe { .. }) {
        "warn"
    } else {
        "info"
    };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(filter)),
        )
        .with_target(false)
        .compact()
        .init();

    let home = resolve_home(cli.home);
    let rt = tokio::runtime::Runtime::new().expect("tokio");

    let result = match cli.command {
        Commands::Sprout { contribute } => rt.block_on(sprout(&home, &contribute)),
        Commands::Daemon {
            contribute,
            bootstrap,
            seed_file,
            public_bootstrap,
            bootstrap_url,
            listen,
            horizon_port,
            no_mdns,
            announce_ip,
            announce_ip6,
            relay,
            sporocarp,
            membrane,
            assume_reachable,
            webrtc,
            webrtc_port,
            nostr_transport,
            no_nostr_transport,
            nostr_relay,
            upnp,
        } => rt.block_on(daemon(
            &home,
            &contribute,
            DaemonOptions {
                contribute: None, // preenchido abaixo
                bootstrap,
                horizon_port,
                listen,
                seed_file,
                public_bootstrap,
                bootstrap_url,
                no_mdns,
                announce_ip,
                announce_ip6,
                enable_relay: relay || sporocarp,
                sporocarp,
                membrane,
                assume_reachable,
                enable_webrtc: webrtc,
                webrtc_port,
                nostr_transport: if no_nostr_transport {
                    Some(false)
                } else if nostr_transport {
                    Some(true)
                } else {
                    None
                },
                nostr_relay,
            },
            upnp,
        )),
        Commands::Status => rt.block_on(status(&home)),
        Commands::Sow {
            message,
            path,
            content,
            qel,
            nostr,
            ghost,
            recipient,
            hybrid,
        } => rt.block_on(sow_cmd(
            &home, message, path, content, qel, nostr, ghost, recipient, hybrid,
        )),
        Commands::Signal {
            plot,
            quorum,
            ion,
            name,
        } => rt.block_on(rpc(
            &home,
            Request::Signal {
                plot,
                quorum,
                ion,
                name,
            },
        )),
        Commands::Resonate { signal } => {
            rt.block_on(rpc(&home, Request::Resonate { signal }))
        }
        Commands::Recall {
            plot,
            qel,
            nostr,
            qel_threshold,
            hybrid,
        } => rt.block_on(recall_cmd(&home, plot, qel, nostr, qel_threshold, hybrid)),
        Commands::Bootstrap { addr } => {
            rt.block_on(rpc(&home, Request::Bootstrap { addr }))
        }
        Commands::Seeds { action } => seeds_cmd(&home, action),
        Commands::IsotopePut { key, value, clock } => rt.block_on(rpc(
            &home,
            Request::IsotopePut { key, value, clock },
        )),
        Commands::IsotopeGet { key } => rt.block_on(isotope_get_poll(&home, key)),
        Commands::Deploy {
            plot,
            message,
            path,
            content,
            ion,
            name,
            quorum,
            timeout,
        } => rt.block_on(deploy(
            &home,
            DeployOpts {
                plot,
                message,
                path,
                content,
                ion,
                name,
                quorum,
                timeout,
            },
        )),
        Commands::Shutdown => rt.block_on(rpc(&home, Request::Shutdown)),
        Commands::Balance => rt.block_on(rpc(&home, Request::Balance)),
        Commands::IonMigrate { ion, target } => rt.block_on(rpc(&home, Request::IonMigrate { ion, target })),
        Commands::Zones => rt.block_on(rpc(&home, Request::Zones)),
        Commands::Entropy { action } => rt.block_on(entropy_cmd(&home, action)),
        Commands::Candidate {
            cmd,
            r#loop,
            once: _,
            relays,
        } => rt.block_on(candidate_cmd(&home, cmd, r#loop, relays)),
        Commands::ChamberServe { port, ion, root } => {
            rt.block_on(chamber_serve(port, ion, root))
        }
        Commands::Store { action } => store_cmd(&home, action),
        Commands::Repo { action } => rt.block_on(repo_cmd(&home, action)),
    };

    if let Err(e) = result {
        eprintln!("[🍄] {e}");
        std::process::exit(1);
    }
}

fn resolve_home(override_home: Option<PathBuf>) -> PathBuf {
    if let Some(p) = override_home {
        return p;
    }
    directories::ProjectDirs::from("network", "Mycelium", "mycelium")
        .map(|d| d.data_local_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".mycelium"))
}

fn parse_membrane(s: &str) -> Result<mycelium_core::Membrane, String> {
    s.parse()
}

fn seeds_cmd(home: &PathBuf, action: SeedsCmd) -> Result<(), String> {
    let path = home.join("seeds.txt");
    match action {
        SeedsCmd::List => {
            let mut book = SeedBook::new();
            book.load_file(&path).map_err(|e| e.to_string())?;
            if book.is_empty() {
                println!("[🍄] seed book vazio ({})", path.display());
                println!("[🍄] dica: mycelium seeds fetch  ou  --public-bootstrap");
            } else {
                println!("[🍄] {} seeds em {}", book.len(), path.display());
                for s in book.as_strings() {
                    println!("  {s}");
                }
            }
            Ok(())
        }
        SeedsCmd::Add { addr } => {
            let mut book = SeedBook::new();
            book.load_file(&path).map_err(|e| e.to_string())?;
            book.add(&addr).map_err(|e| e.to_string())?;
            book.save_file(&path).map_err(|e| e.to_string())?;
            println!("[🍄] seed adicionada: {addr}");
            Ok(())
        }
        SeedsCmd::Fetch { url, dns } => {
            let mut book = SeedBook::new();
            book.load_file(&path).map_err(|e| e.to_string())?;
            let mut added = 0usize;
            if let Some(name) = dns {
                let name = if name.is_empty() {
                    DEFAULT_DNS_SEED_NAME.to_string()
                } else {
                    name
                };
                let n = book.fetch_dns_txt(&name).map_err(|e| e.to_string())?;
                added += n;
                println!("[🍄] +{n} seeds DNS TXT `{name}`");
            } else if url.is_none() {
                // Sem flags: HTTP legado (comportamento anterior).
                let u = DEFAULT_BOOTSTRAP_URL.to_string();
                let n = book.fetch_url(&u).map_err(|e| e.to_string())?;
                added += n;
                println!("[🍄] +{n} seeds de {u}");
            }
            if let Some(u) = url {
                let n = book.fetch_url(&u).map_err(|e| e.to_string())?;
                added += n;
                println!("[🍄] +{n} seeds de {u}");
            }
            book.save_file(&path).map_err(|e| e.to_string())?;
            println!("[🍄] total +{added} → {}", path.display());
            for s in book.as_strings() {
                println!("  {s}");
            }
            Ok(())
        }
        SeedsCmd::Catalog { action } => seed_catalog_cmd(home, action),
    }
}

fn seed_catalog_cmd(home: &PathBuf, action: SeedCatalogCmd) -> Result<(), String> {
    use mycelium_hyphae::{SeedCatalog, SeedEntry, SeedVisibility};
    match action {
        SeedCatalogCmd::List { visibility } => {
            let catalog = SeedCatalog::open(home)?;
            let filter = match visibility.as_str() {
                "all" => None,
                "public" => Some(SeedVisibility::Public),
                "private" => Some(SeedVisibility::Private),
                other => {
                    return Err(format!(
                        "visibilidade desconhecida: '{other}' (use public|private|all)"
                    ))
                }
            };
            let entries = catalog.list(filter);
            println!("\n[🍄] Catálogo de seeds ({}) — {}", entries.len(), SeedCatalog::catalog_path(home).display());
            for s in entries {
                let verified = if s.verified { "✓" } else { "✗" };
                println!("  • [{}] {} ({})", s.visibility.as_str(), s.id, s.name);
                println!("      multiaddr : {}", s.multiaddr);
                println!(
                    "      meta      : membrana={} região={} operador={} relay={} inbound={}",
                    s.membrane.as_deref().unwrap_or("-"),
                    s.region.as_deref().unwrap_or("-"),
                    s.operator.as_deref().unwrap_or("-"),
                    s.relay,
                    verified
                );
            }
            Ok(())
        }
        SeedCatalogCmd::Add {
            id,
            name,
            multiaddr,
            visibility,
            membrane,
            region,
            operator,
            relay,
            verified,
        } => {
            let visibility = match visibility.as_str() {
                "public" => SeedVisibility::Public,
                "private" => SeedVisibility::Private,
                other => return Err(format!("visibilidade inválida: '{other}' (use public|private)")),
            };
            let mut catalog = SeedCatalog::open(home)?;
            catalog.add(SeedEntry {
                id: id.unwrap_or_default(),
                name,
                multiaddr,
                visibility,
                membrane,
                region,
                operator,
                relay,
                verified,
                last_seen: None,
                notes: None,
            })?;
            catalog.save(home)?;
            println!("[🍄] Seed adicionada ao catálogo.");
            Ok(())
        }
        SeedCatalogCmd::Remove { id } => {
            let mut catalog = SeedCatalog::open(home)?;
            if catalog.remove(&id) {
                catalog.save(home)?;
                println!("[🍄] Seed '{id}' removida.");
            } else {
                println!("[🍄] Seed '{id}' não encontrada.");
            }
            Ok(())
        }
        SeedCatalogCmd::Export { out } => {
            let catalog = SeedCatalog::open(home)?;
            let lines = catalog.to_mainnet_lines();
            let n_seeds = lines.iter().filter(|l| !l.starts_with('#')).count();
            let dest = out.unwrap_or_else(|| home.join("seeds").join("mainnet.txt"));
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut body = String::from("# Mycelium Network — seeds públicas verificadas (do catálogo)\n# Só entradas com inbound verificado entram aqui.\n");
            for l in &lines {
                body.push_str(l);
                body.push('\n');
            }
            std::fs::write(&dest, body).map_err(|e| format!("gravar {}: {e}", dest.display()))?;
            println!("[🍄] Exportadas {} seeds públicas verificadas → {}", n_seeds, dest.display());
            Ok(())
        }
    }
}

async fn sprout(home: &PathBuf, contribute: &str) -> Result<(), String> {
    println!("[🍄] Semente germinando...");
    let store = NodeStore::open(home).map_err(|e| e.to_string())?;
    let gland = store.load_or_create_gland().map_err(|e| e.to_string())?;
    let resources = Resources::from_str(contribute).map_err(|e| e.to_string())?;
    store
        .save_resources(&resources)
        .map_err(|e| e.to_string())?;
    let mut ledger = store.load_ledger();
    if ledger.history().is_empty() {
        ledger.pledge(&resources);
        store.save_ledger(&ledger).map_err(|e| e.to_string())?;
    }
    println!(
        "[🍄] Identidade persistida: {} (NodeId {})",
        gland.node_id().short(),
        gland.node_id()
    );
    println!("[🍄] Home: {}", home.display());
    println!("[🍄] Pronto. Suba o organismo com: mycelium daemon");
    Ok(())
}

async fn daemon(
    home: &PathBuf,
    contribute: &str,
    mut opts: DaemonOptions,
    upnp_flag: bool,
) -> Result<(), String> {
    let resources = Resources::from_str(contribute).map_err(|e| e.to_string())?;
    opts.contribute = Some(resources);
    println!("[🍄] Despertando organismo em {}…", home.display());
    println!("[🍄] Event Horizon em http://127.0.0.1:{}/", opts.horizon_port);
    if upnp_flag {
        println!("[🍄] --upnp ignorado — Política de Membrana (sem STUN/UPnP)");
    }
    if opts.public_bootstrap {
        println!(
            "[🍄] Bootstrap público: {}",
            opts.bootstrap_url
                .as_deref()
                .unwrap_or(DEFAULT_BOOTSTRAP_URL)
        );
    }
    if opts.no_mdns {
        println!("[🍄] mDNS desligado — só seed book / bootstrap");
    }
    if let Some(ip) = &opts.announce_ip {
        println!("[🍄] Announce IP (raiz IPv4 declarada): {ip}");
    }
    if let Some(ip6) = &opts.announce_ip6 {
        println!("[🍄] Announce IPv6: {ip6}");
    }
    if let Some(m) = opts.membrane {
        println!("[🍄] Membrana forçada: {m}");
    }
    if opts.sporocarp {
        println!("[🍄] Sporocarp (relay + DNS) ligado — membrana esporocarp");
    } else if opts.enable_relay {
        println!("[🍄] Relay server (circuit v2) ligado");
    }
    if !opts.listen.is_empty() {
        println!("[🍄] Listen: {:?}", opts.listen);
    } else {
        println!("[🍄] Listen: auto conforme membrana (folha=loopback IPv4)");
    }
    if std::env::var("MYCELIUM_CONTROL_TOKEN").ok().filter(|t| !t.is_empty()).is_some() {
        println!("[🍄] Control socket com auth (MYCELIUM_CONTROL_TOKEN)");
    }
    println!(
        "[🍄] Ctrl-C ou `mycelium --home {} shutdown` para hibernar",
        home.display()
    );

    let home_for_signal = home.clone();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        let sock = home_for_signal.join("mycelium.sock");
        let _ = call(&sock, Request::Shutdown).await;
    });

    run_daemon(home.clone(), opts)
        .await
        .map_err(|e| e.to_string())
}

async fn status(home: &PathBuf) -> Result<(), String> {
    let sock = home.join("mycelium.sock");
    // Android/shell: Unix socket pode falhar; `call` cai para `mycelium.tcp`.
    if sock.exists() || sock.with_extension("tcp").exists() {
        return print_response(call(&sock, Request::Status).await?);
    }
    let store = NodeStore::open(home).map_err(|e| e.to_string())?;
    let gland = store.load_or_create_gland().map_err(|e| e.to_string())?;
    let ledger = store.load_ledger();
    let state = store.load_state();
    let addrs = store.load_listen_addrs();
    let ion_names: Vec<_> = state.ions.iter().map(|i| i.name.clone()).collect();
    println!("[🍄] Estado offline (daemon não está rodando)");
    println!("    home     : {}", home.display());
    println!("    NodeId   : {}", gland.node_id());
    println!("    listen   : {addrs:?}");
    println!("    ions     : {ion_names:?}");
    println!("    signals  : {}", state.field.len());
    println!(
        "    ATP={} Enzymes={} Mycelia={} Spores={} Resilience={}",
        ledger.balance(mycelium_core::Nutrient::Atp),
        ledger.balance(mycelium_core::Nutrient::Enzymes),
        ledger.balance(mycelium_core::Nutrient::Mycelia),
        ledger.balance(mycelium_core::Nutrient::Spores),
        ledger.balance(mycelium_core::Nutrient::Resilience),
    );
    Ok(())
}

async fn candidate_cmd(
    home: &PathBuf,
    cmd: Option<CandidateCmd>,
    do_loop: bool,
    relays: Vec<String>,
) -> Result<(), String> {
    #[cfg(not(feature = "nostr"))]
    {
        let _ = (home, cmd, do_loop, relays);
        return Err(
            "`mycelium candidate` requer `cargo build -p mycelium-cli --features nostr`".into(),
        );
    }
    #[cfg(feature = "nostr")]
    {
        use mycelium_nostr::{
            candidate_sleep_secs, run_candidate_round, run_listen_round, send_backchannel,
            CandidateSession, RelayPool,
        };
        use std::collections::HashSet;

        let pool = if relays.is_empty() {
            RelayPool::default_public()
        } else {
            RelayPool::new(relays)
        };

        match cmd {
            None => {
                loop {
                    match run_candidate_round(&pool).await {
                        Ok(r) => {
                            println!(
                                "[🍄] candidate: published={} discovered={} peers={} ghost={}…",
                                r.published,
                                r.discovered,
                                r.peer_count,
                                &r.self_ghost[..r.self_ghost.len().min(12)]
                            );
                            for p in &r.peers {
                                println!("  peer {}", p);
                            }
                            if r.peer_count == 0 {
                                println!(
                                    "[🍄] candidate: ainda 0 peers (ponto fixo). Outra folha no mesmo relay?"
                                );
                            } else {
                                println!(
                                    "[🍄] candidate: peers vistos — use `listen`/`send` para backchannel"
                                );
                            }
                        }
                        Err(e) => eprintln!("[🍄] candidate round falhou: {e}"),
                    }
                    if !do_loop {
                        break;
                    }
                    let sleep = candidate_sleep_secs();
                    println!("[🍄] candidate: próxima ronda em {sleep}s (jitter)");
                    tokio::time::sleep(std::time::Duration::from_secs(sleep)).await;
                }
                Ok(())
            }
            Some(CandidateCmd::Whoami) => {
                let (sess, _) = CandidateSession::load_or_create(home).map_err(|e| e.to_string())?;
                println!("[🍄] candidate ghost: {}", sess.pk_hex);
                println!("    sessão: {}", CandidateSession::path(home).display());
                println!("    TTL restante ~{}s (desde criação)", {
                    let age = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0)
                        .saturating_sub(sess.created_at);
                    sess.ttl_secs.saturating_sub(age)
                });
                Ok(())
            }
            Some(CandidateCmd::Reset) => {
                CandidateSession::clear(home).map_err(|e| e.to_string())?;
                println!("[🍄] candidate.session apagada");
                Ok(())
            }
            Some(CandidateCmd::Send { to, message }) => {
                let to = to.trim().to_lowercase();
                let (_, ghost) =
                    CandidateSession::load_or_create(home).map_err(|e| e.to_string())?;
                println!(
                    "[🍄] candidate send: from={}… → to={}…",
                    &ghost.nostr_pubkey_hex()[..12],
                    &to[..to.len().min(12)]
                );
                let id = send_backchannel(&pool, &ghost, &to, &message)
                    .await
                    .map_err(|e| e.to_string())?;
                println!("[🍄] enviado event {id}");
                println!(
                    "[🍄] o destinatário precisa de `mycelium candidate listen` com esse ghost"
                );
                Ok(())
            }
            Some(CandidateCmd::Listen { r#loop: listen_loop }) => {
                let (sess, ghost) =
                    CandidateSession::load_or_create(home).map_err(|e| e.to_string())?;
                println!("[🍄] candidate listen ghost: {}", sess.pk_hex);
                println!("[🍄] o outro lado: mycelium candidate send --to {} -m \"…\"", sess.pk_hex);
                let mut seen = HashSet::new();
                loop {
                    match run_listen_round(&pool, &ghost).await {
                        Ok((published, msgs)) => {
                            println!(
                                "[🍄] listen: announced={published} inbox={}",
                                msgs.len()
                            );
                            for m in msgs {
                                if seen.insert(m.event_id.clone()) {
                                    println!(
                                        "[🍄] ← {}… : {}",
                                        &m.from[..m.from.len().min(12)],
                                        m.text
                                    );
                                }
                            }
                        }
                        Err(e) => eprintln!("[🍄] listen round falhou: {e}"),
                    }
                    if !listen_loop {
                        break;
                    }
                    let sleep = 15u64;
                    tokio::time::sleep(std::time::Duration::from_secs(sleep)).await;
                }
                Ok(())
            }
        }
    }
}

async fn entropy_cmd(home: &PathBuf, action: EntropyCmd) -> Result<(), String> {
    let sock = home.join("mycelium.sock");
    match action {
        EntropyCmd::Shatter {
            secret,
            threshold,
            total,
        } => {
            let resp = call(
                &sock,
                Request::EntropyShatter {
                    secret,
                    threshold,
                    total,
                },
            )
            .await?;
            print_response(resp)
        }
        EntropyCmd::Reconstruct { threshold } => {
            // Poll com retry pra dar tempo das shades chegarem via gossip
            let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(5);
            let mut last = String::new();
            while tokio::time::Instant::now() < deadline {
                match call(&sock, Request::EntropyReconstruct { threshold }).await? {
                    Response::Ok { message } => {
                        println!("[🍄] {message}");
                        return Ok(());
                    }
                    Response::Err { message } => {
                        last = message;
                        if last.contains("não") && last.contains("custódia")  {
                            // erro definitivo, não de timeout
                            return Err(last);
                        }
                    }
                    Response::Status(_) => return Err("resposta inesperada".into()),
                    _ => return Err("resposta inesperada".into()),
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            }
            Err(format!("entropy reconstruct timeout: {last}"))
        }
        EntropyCmd::Status => {
            let resp = call(&sock, Request::EntropyStatus).await?;
            print_response(resp)
        }
    }
}

async fn sow_cmd(
    home: &PathBuf,
    message: String,
    path: String,
    content: String,
    qel: Option<String>,
    nostr: bool,
    ghost: bool,
    recipient: Option<String>,
    hybrid: bool,
) -> Result<(), String> {
    #[cfg(not(feature = "nostr"))]
    {
        if qel.is_some() || nostr || ghost || recipient.is_some() || hybrid {
            return Err(
                "sow --qel/--nostr/--ghost/--hybrid requer `cargo build -p mycelium-cli --features nostr`"
                    .into(),
            );
        }
    }
    let want_nostr = nostr || qel.is_some() || ghost || hybrid;
    let qel = if want_nostr && qel.is_none() {
        Some("3,7".into())
    } else {
        qel
    };
    let sock = home.join("mycelium.sock");
    let resp = call(
        &sock,
        Request::Sow {
            message,
            path,
            content,
            qel: qel.clone(),
            nostr: nostr || hybrid,
            ghost: ghost || nostr || hybrid,
            recipient: recipient.clone(),
        },
    )
    .await?;

    #[cfg(feature = "nostr")]
    if want_nostr {
        if let Response::Ok { message: ref msg } = resp {
            if let Some(id_str) = msg.strip_prefix("plot semeado: ") {
                let id_str = id_str.split(';').next().unwrap_or(id_str).trim();
                match publish_plot_nostr(
                    home,
                    id_str,
                    qel.as_deref(),
                    recipient.as_deref(),
                    hybrid,
                )
                .await
                {
                    Ok(extra) => {
                        println!("[🍄] {msg}{extra}");
                        return Ok(());
                    }
                    Err(e) => {
                        println!("[🍄] {msg}");
                        return Err(format!("plot local ok; nostr/qel falhou: {e}"));
                    }
                }
            }
        }
    }

    print_response(resp)
}

#[cfg(feature = "nostr")]
async fn publish_plot_nostr(
    home: &PathBuf,
    id_str: &str,
    qel_spec: Option<&str>,
    recipient: Option<&str>,
    hybrid: bool,
) -> Result<String, String> {
    use mycelium_core::ContentId;
    use mycelium_sporebank::SporeBank;
    use std::str::FromStr;

    let id = ContentId::from_str(id_str).map_err(|e| e.to_string())?;
    let bank = SporeBank::open(home).map_err(|e| e.to_string())?;
    let bytes = bank.spore_print(&id).map_err(|e| e.to_string())?;

    let (threshold, total) = parse_qel_kn(qel_spec)?;
    let cfg = mycelium_qel::QelConfig {
        threshold,
        total,
        ttl_secs: 86_400,
    };
    let ghost = mycelium_ghostid::GhostId::spawn_quick(cfg.ttl_secs).map_err(|e| e.to_string())?;
    let mut shards = if hybrid {
        mycelium_qel::fragment_hybrid(&bytes, &id.to_string(), &cfg).map_err(|e| e.to_string())?
    } else {
        mycelium_qel::fragment(&bytes, &id.to_string(), &cfg).map_err(|e| e.to_string())?
    };

    let mut landscape_note = String::new();
    if hybrid {
        let ctx = mycelium_distancebridge::TransportContext {
            has_internet: true,
            ipfs_peers: 1,
            relay_available: false,
            ..Default::default()
        };
        let ranked = mycelium_distancebridge::select_transports(&ctx, 3);
        landscape_note = ranked
            .iter()
            .map(|(t, p)| format!("{t:?}:{p:.2}"))
            .collect::<Vec<_>>()
            .join(",");
        let hints =
            mycelium_distancebridge::hybrid_hints_from_landscape(&ctx, threshold, total);
        for (shard, hint) in shards.iter_mut().zip(hints) {
            shard.transport = hint;
        }
    }

    let blake3_hex = hex::encode(blake3::hash(&bytes).as_bytes());
    let pool = mycelium_nostr::RelayPool::default_public().with_min_relays(1);
    // Publicar shards de mailbox (Nostr / RelayMesh / Sms); store fica no blockstore.
    let to_publish: Vec<_> = if hybrid {
        shards
            .iter()
            .filter(|s| {
                matches!(
                    s.transport,
                    mycelium_qel::TransportHint::Nostr
                        | mycelium_qel::TransportHint::RelayMesh
                        | mycelium_qel::TransportHint::Sms
                )
            })
            .cloned()
            .collect()
    } else {
        shards.iter().take(threshold as usize).cloned().collect()
    };
    let published = mycelium_nostr::publish_shards(
        &pool,
        &ghost,
        &to_publish,
        &blake3_hex,
        bytes.len(),
        recipient,
    )
    .await
    .map_err(|e| e.to_string())?;

    let mut extra = format!(
        "; qel={threshold},{total} nostr_publishes={published} ghost={}",
        ghost.nostr_pubkey_hex()
    );

    if hybrid {
        let store = mycelium_ipfs::BlockStore::open(home).map_err(|e| e.to_string())?;
        store.put_named(&id, &bytes).map_err(|e| e.to_string())?;
        let mut ipfs_shards = 0usize;
        for shard in shards.iter().filter(|s| {
            matches!(
                s.transport,
                mycelium_qel::TransportHint::Ipfs
                    | mycelium_qel::TransportHint::Dtn
                    | mycelium_qel::TransportHint::Visual
            )
        }) {
            let wire = serde_json::to_vec(shard).map_err(|e| e.to_string())?;
            let shard_key = format!("{}:shard:{}", id, shard.index);
            let shard_id = ContentId::of(shard_key.as_bytes());
            store
                .put_named(&shard_id, &wire)
                .map_err(|e| e.to_string())?;
            ipfs_shards += 1;
        }
        extra.push_str(&format!(
            " hybrid=1 ipfs_plot=1 ipfs_shards={ipfs_shards} landscape=[{landscape_note}]"
        ));
    }

    Ok(extra)
}

#[cfg(feature = "nostr")]
fn parse_qel_kn(spec: Option<&str>) -> Result<(u8, u8), String> {
    let s = spec.unwrap_or("3,7");
    let (k, n) = s
        .split_once(',')
        .ok_or_else(|| format!("qel inválido '{s}' — use k,n (ex. 3,7)"))?;
    Ok((
        k.trim()
            .parse()
            .map_err(|_| "qel threshold inválido".to_string())?,
        n.trim()
            .parse()
            .map_err(|_| "qel total inválido".to_string())?,
    ))
}

async fn recall_cmd(
    home: &PathBuf,
    plot: String,
    qel: bool,
    nostr: bool,
    qel_threshold: u8,
    hybrid: bool,
) -> Result<(), String> {
    #[cfg(not(feature = "nostr"))]
    {
        if qel || nostr || hybrid {
            return Err(
                "recall --qel/--nostr/--hybrid requer `cargo build -p mycelium-cli --features nostr`"
                    .into(),
            );
        }
    }

    #[cfg(feature = "nostr")]
    if qel || nostr || hybrid {
        // 1) SporeBank local
        if let Ok(bank) = mycelium_sporebank::SporeBank::open(home) {
            if let Ok(id) = plot.parse::<mycelium_core::ContentId>() {
                if let Some(p) = bank.recall(&id) {
                    println!(
                        "[🍄] plot {} — \"{}\" ({} leaves) [local]",
                        id.short(),
                        p.message,
                        p.leaves.len()
                    );
                    return Ok(());
                }
            }
        }

        // 2) Nostr QEL
        match recall_plot_nostr(home, &plot, qel_threshold).await {
            Ok(msg) => {
                println!("[🍄] {msg}");
                return Ok(());
            }
            Err(nostr_err) => {
                // 3) Hybrid: blockstore ipfs local
                if hybrid {
                    match recall_plot_ipfs(home, &plot).await {
                        Ok(msg) => {
                            println!("[🍄] {msg}");
                            return Ok(());
                        }
                        Err(ipfs_err) => {
                            let sock = home.join("mycelium.sock");
                            if sock.exists() || sock.with_extension("tcp").exists() {
                                let resp = call(
                                    &sock,
                                    Request::Recall {
                                        plot: plot.clone(),
                                        qel,
                                        nostr,
                                        qel_threshold: Some(qel_threshold),
                                    },
                                )
                                .await?;
                                print_response(resp)?;
                            }
                            return Err(format!(
                                "hybrid: nostr={nostr_err}; ipfs={ipfs_err}"
                            ));
                        }
                    }
                }

                let sock = home.join("mycelium.sock");
                if sock.exists() || sock.with_extension("tcp").exists() {
                    let resp = call(
                        &sock,
                        Request::Recall {
                            plot: plot.clone(),
                            qel,
                            nostr,
                            qel_threshold: Some(qel_threshold),
                        },
                    )
                    .await?;
                    print_response(resp)?;
                }
                return Err(format!("nostr/qel: {nostr_err}"));
            }
        }
    }

    let sock = home.join("mycelium.sock");
    print_response(
        call(
            &sock,
            Request::Recall {
                plot,
                qel,
                nostr,
                qel_threshold: None,
            },
        )
        .await?,
    )
}

#[cfg(feature = "nostr")]
async fn recall_plot_ipfs(home: &PathBuf, plot: &str) -> Result<String, String> {
    use mycelium_core::ContentId;
    use mycelium_sporebank::SporeBank;
    use std::str::FromStr;

    let id = ContentId::from_str(plot).map_err(|e| e.to_string())?;
    let store = mycelium_ipfs::BlockStore::open(home).map_err(|e| e.to_string())?;
    let bytes = store.get(&id).map_err(|e| e.to_string())?;
    let mut bank = SporeBank::open(home).map_err(|e| e.to_string())?;
    let absorbed = bank.absorb(&bytes).map_err(|e| e.to_string())?;
    let p = bank.recall(&absorbed);
    Ok(format!(
        "plot {} reconstruído via ipfs-blocks — \"{}\" ({} leaves)",
        absorbed.short(),
        p.map(|x| x.message.as_str()).unwrap_or("?"),
        p.map(|x| x.leaves.len()).unwrap_or(0)
    ))
}

#[cfg(feature = "nostr")]
async fn recall_plot_nostr(home: &PathBuf, plot: &str, threshold: u8) -> Result<String, String> {
    use mycelium_core::ContentId;
    use mycelium_sporebank::SporeBank;
    use std::str::FromStr;

    let id = ContentId::from_str(plot).map_err(|e| e.to_string())?;
    let pool = mycelium_nostr::RelayPool::default_public().with_min_relays(1);
    let shards = mycelium_nostr::fetch_shards(&pool, &id.to_string(), threshold, None)
        .await
        .map_err(|e| e.to_string())?;
    if shards.len() < threshold as usize {
        return Err(format!(
            "só {} shards Nostr (preciso {threshold})",
            shards.len()
        ));
    }
    let bytes = mycelium_qel::reconstruct(&shards).map_err(|e| e.to_string())?;
    let mut bank = SporeBank::open(home).map_err(|e| e.to_string())?;
    let absorbed = bank.absorb(&bytes).map_err(|e| e.to_string())?;
    let p = bank.recall(&absorbed);
    Ok(format!(
        "plot {} reconstruído via Nostr/QEL — \"{}\" ({} leaves)",
        absorbed.short(),
        p.map(|x| x.message.as_str()).unwrap_or("?"),
        p.map(|x| x.leaves.len()).unwrap_or(0)
    ))
}

async fn rpc(home: &PathBuf, request: Request) -> Result<(), String> {
    let sock = home.join("mycelium.sock");
    print_response(call(&sock, request).await?)
}

/// Poll IsotopeGet até ~3s (Decay pelas hifas).
async fn isotope_get_poll(home: &PathBuf, key: String) -> Result<(), String> {
    let sock = home.join("mycelium.sock");
    let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(3);
    let mut last_err = String::from("timeout");
    while tokio::time::Instant::now() < deadline {
        match call(&sock, Request::IsotopeGet { key: key.clone() }).await? {
            Response::Ok { message } => {
                println!("[🍄] {message}");
                return Ok(());
            }
            Response::Err { message } => {
                last_err = message;
                if !last_err.contains("decay em curso") {
                    return Err(last_err);
                }
            }
            Response::Status(_) => return Err("resposta inesperada".into()),
            _ => return Err("resposta inesperada".into()),
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    }
    Err(format!("isotope-get timeout: {last_err}"))
}

/// sow (opcional) → signal → espera ion → imprime URL do Event Horizon.
struct DeployOpts {
    plot: Option<String>,
    message: String,
    path: String,
    content: String,
    ion: String,
    name: String,
    quorum: usize,
    timeout: u64,
}

async fn deploy(home: &PathBuf, opts: DeployOpts) -> Result<(), String> {
    let sock = home.join("mycelium.sock");
    let plot_id = if let Some(p) = opts.plot {
        p
    } else {
        println!("[🍄] Semeando plot…");
        match call(
            &sock,
            Request::Sow {
                message: opts.message,
                path: opts.path,
                content: opts.content,
                qel: None,
                nostr: false,
                ghost: false,
                recipient: None,
            },
        )
        .await?
        {
            Response::Ok { message } => message
                .strip_prefix("plot semeado: ")
                .unwrap_or(&message)
                .to_string(),
            Response::Err { message } => return Err(message),
            Response::Status(_) => return Err("resposta inesperada no sow".into()),
            _ => return Err("resposta inesperada no sow".into()),
        }
    };
    println!("[🍄] Plot {plot_id}");
    println!("[🍄] Signal → ion `{}`…", opts.ion);
    match call(
        &sock,
        Request::Signal {
            plot: plot_id,
            quorum: opts.quorum,
            ion: opts.ion.clone(),
            name: opts.name,
        },
    )
    .await?
    {
        Response::Ok { message } => println!("[🍄] {message}"),
        Response::Err { message } => return Err(message),
        Response::Status(_) => return Err("resposta inesperada no signal".into()),
        _ => return Err("resposta inesperada no signal".into()),
    }

    let deadline =
        tokio::time::Instant::now() + tokio::time::Duration::from_secs(opts.timeout);
    while tokio::time::Instant::now() < deadline {
        if let Response::Status(s) = call(&sock, Request::Status).await? {
            if s.ions.iter().any(|n| n == &opts.ion) {
                let base = if s.event_horizon.ends_with('/') {
                    s.event_horizon.clone()
                } else {
                    format!("{}/", s.event_horizon)
                };
                let url = format!("{base}{}/", opts.ion);
                println!("[🍄] Vacuum Chamber pronta");
                println!("[🍄] Singularity Event Horizon: {url}");
                println!("[🍄] curl -s {url}");
                return Ok(());
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;
    }
    Err(format!(
        "deploy timeout: ion `{}` não apareceu em {}s",
        opts.ion, opts.timeout
    ))
}

fn print_response(resp: Response) -> Result<(), String> {
    match resp {
        Response::Ok { message } => {
            println!("[🍄] {message}");
            Ok(())
        }
        Response::Status(s) => {
            println!("[🍄] Organismo vivo");
            println!("    home       : {}", s.home);
            println!("    NodeId     : {}", s.node_id);
            println!("    PeerId     : {}", s.peer_id);
            println!("    listen     : {:?}", s.listen_addrs);
            println!("    vizinhos   : {}", s.neighbors);
            println!("    plots      : {}", s.plots);
            println!("    signals    : {}", s.signals);
            println!("    ions       : {:?}", s.ions);
            println!(
                "    isotope    : shard={}/{} atoms={}",
                s.isotope_shard, s.isotope_ring, s.isotope_atoms
            );
            if !s.event_horizon.is_empty() {
                println!("    horizon    : {}", s.event_horizon);
            }
            for ep in &s.ion_endpoints {
                println!("    chamber    : {ep}");
            }
            for ion in &s.ions {
                println!(
                    "    curl       : curl -s {}{ion}/",
                    if s.event_horizon.ends_with('/') {
                        s.event_horizon.clone()
                    } else {
                        format!("{}/", s.event_horizon)
                    }
                );
            }
            println!(
                "    nutrientes : ATP={} Enzymes={} Mycelia={} Spores={} Resilience={}",
                s.atp, s.enzymes, s.mycelia, s.spores, s.resilience
            );
            println!(
                "    hifas      : anastomoses={} atrophies={} msg_in={} msg_out={}",
                s.anastomoses, s.atrophies, s.messages_in, s.messages_out
            );
            if !s.membrane.is_empty() {
                println!("    membrana   : {}", s.membrane);
            }
            if s.sporocarp {
                println!("    sporocarp  : sim");
            }
            println!(
                "    wan_reach  : {}",
                if s.wan_reachable { "sim" } else { "nao" }
            );
            if s.is_relay {
                println!("    is_relay   : sim");
            }
            if let Some(r) = &s.active_relay {
                println!("    active_relay: {r}");
            }
            if !s.relay_health.is_empty() {
                println!("    relay_mesh : {}", s.relay_health);
            }
            if !s.physarum_phase.is_empty() {
                println!("    physarum   : {}", s.physarum_phase);
            }
            if let Some(dns) = &s.dns_seed {
                println!("    dns_seed   : {dns}");
            }
            Ok(())
        }
        Response::Err { message } => Err(message),
        _ => Err("resposta não suportada".into()),
    }
}

async fn chamber_serve(port: u16, ion: String, root: PathBuf) -> Result<(), String> {
    let message = std::fs::read_to_string(root.join("message.txt"))
        .or_else(|_| std::fs::read_to_string(root.join("rootfs/MESSAGE")))
        .unwrap_or_else(|_| ion.clone());
    let ion_name = ion.clone();
    let msg = message.clone();
    let built_html = std::fs::read_to_string(root.join("rootfs/index.html")).ok();

    let app = Router::new()
        .route(
            "/",
            get({
                let ion = ion_name.clone();
                let msg = msg.clone();
                move || {
                    let ion = ion.clone();
                    let msg = msg.clone();
                    async move {
                        Json(json!({
                            "ion": ion,
                            "message": msg,
                            "substrate": "mycelium",
                            "runtime": "vacuum-chamber",
                        }))
                    }
                }
            }),
        )
        .route("/health", get(|| async { Json(json!({"ok": true})) }))
        .route(
            "/index.html",
            get({
                let ion = ion_name;
                let msg = message;
                let built = built_html;
                move || {
                    let ion = ion.clone();
                    let msg = msg.clone();
                    let built = built.clone();
                    async move {
                        let body = built.unwrap_or_else(|| {
                            format!(
                                "<!doctype html><html><body style=\"font-family:system-ui;background:#0b1a14;color:#c8e6c9;padding:2rem\">\
                                <h1>🍄 {ion}</h1>\
                                <p>Servido por uma <b>Vacuum Chamber</b> (processo filho).</p>\
                                <pre>{msg}</pre>\
                                </body></html>"
                            )
                        });
                        (
                            [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
                            body,
                        )
                    }
                }
            }),
        );

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| e.to_string())?;
    axum::serve(listener, app)
        .await
        .map_err(|e| e.to_string())
}

fn store_cmd(home: &PathBuf, action: StoreCmd) -> Result<(), String> {
    let rt = tokio::runtime::Runtime::new().expect("tokio");
    rt.block_on(store_cmd_async(home, action))
}

async fn store_cmd_async(home: &PathBuf, action: StoreCmd) -> Result<(), String> {
    use mycelium_store::{
        catalog::StoreCatalog,
        runner::EmulatorRunner,
        spore::{ExecutionEngineType, SoftwareSpore, SporeLicense, TargetPlatform},
    };

    let sock = home.join("mycelium.sock");
    let daemon_running = sock.exists() || sock.with_extension("tcp").exists();

    match action {
        StoreCmd::List => {
            if daemon_running {
                return print_response(call(&sock, Request::StoreList).await?);
            }
            let catalog = StoreCatalog::open(home).map_err(|e| e.to_string())?;
            let spores = catalog.list_public_spores();
            println!("\n🎮 === MYCELIUM APP STORE — Catálogo P2P Retro === 🎮\n");
            for spore in spores {
                println!("🔹 ID: {}", spore.id);
                println!("   Título: {}", spore.title);
                println!("   Plataforma: {}", spore.platform.display_name());
                println!("   Ano: {}", spore.release_year);
                println!("   Licença: {}", spore.license.display_name());
                println!("   Recomendado: {:?}", spore.execution_matrix.recommended);
                println!("   Categorias/Tags: {:?}", spore.tags);
                println!("   ContentId: {}", hex::encode(spore.content_id.0));
                println!("------------------------------------------------------------");
            }
            Ok(())
        }
        StoreCmd::Caps => {
            if daemon_running {
                return print_response(call(&sock, Request::StoreCaps).await?);
            }
            let caps = EmulatorRunner::detect_capabilities();
            println!("\n⚡ === MYCELIUM STORE — Capacidades de Emulação Host === ⚡\n");
            println!(" ⚙️ QEMU Emulators:");
            for (arch, has) in caps.has_qemu {
                println!("    • qemu-system-{:<8}: {}", arch, if has { "✅ Instalado" } else { "❌ Ausente" });
            }
            println!(" ⚙️ RetroArch (Libretro): {}", if caps.has_retroarch { "✅ Instalado" } else { "❌ Ausente" });
            println!("    Cores encontrados: {:?}", caps.available_libretro_cores);
            println!(" ⚙️ MAME Arcade: {}", if caps.has_mame { "✅ Instalado" } else { "❌ Ausente" });
            println!(" ⚙️ Bubblewrap Sandbox (bwrap): {}\n", if caps.has_bwrap_sandbox { "✅ Disponível" } else { "❌ Não encontrado" });
            Ok(())
        }
        StoreCmd::Launch { id, engine, sandbox } => {
            if daemon_running {
                let req = Request::StoreLaunch { id, engine, sandbox };
                return print_response(call(&sock, req).await?);
            }
            let catalog = StoreCatalog::open(home).map_err(|e| e.to_string())?;
            let spore = catalog.get_spore(&id).ok_or_else(|| format!("Spore '{}' não encontrado", id))?;

            let caps = EmulatorRunner::detect_capabilities();

            let forced_engine = match engine.as_deref() {
                Some("native") => Some(ExecutionEngineType::Native),
                Some("retroarch") => Some(ExecutionEngineType::RetroArchLibretro),
                Some("mame") => Some(ExecutionEngineType::MAME),
                Some("qemu") => Some(ExecutionEngineType::QEMU),
                Some("wasm") => Some(ExecutionEngineType::WebAssembly),
                Some("cloud") => Some(ExecutionEngineType::P2PCloudStream),
                Some(other) => return Err(format!("Motor de execução desconhecido: '{}'", other)),
                None => None,
            };

            let resolved_engine = EmulatorRunner::resolve_best_engine(spore, &caps, forced_engine);
            println!("[🍄 Store] Preparando lançamento de '{}'...", spore.title);
            println!("[🍄 Store] Plataforma Alvo: {}", spore.platform.display_name());
            println!("[🍄 Store] Motor Escolhido: {:?}", resolved_engine);

            let dummy_path = home.join("store").join(&spore.main_binary_file);

            match EmulatorRunner::launch(spore, &dummy_path, resolved_engine, sandbox) {
                Ok(_child) => {
                    println!("[🍄 Store] Processo do emulador lançado com sucesso!");
                    Ok(())
                }
                Err(err) => Err(format!("Erro ao lançar o emulador: {}", err)),
            }
        }
        StoreCmd::Publish { id, title, platform, binary, license } => {
            if !binary.exists() {
                return Err(format!("Arquivo binário '{:?}' não existe", binary));
            }
            let bytes = std::fs::read(&binary).map_err(|e| e.to_string())?;

            let plat = match platform.to_lowercase().as_str() {
                "snes" => TargetPlatform::SNES,
                "nes" => TargetPlatform::NES,
                "megadrive" | "genesis" => TargetPlatform::MegaDrive,
                "msdos" | "dos" => TargetPlatform::MSDOS,
                "win98" | "win95" => TargetPlatform::Windows98,
                "arcade" | "mame" => TargetPlatform::ArcadeMame,
                "mac" | "ppc" => TargetPlatform::PowerPCMac,
                _ => TargetPlatform::NativeSystem,
            };

            let lic = match license.to_lowercase().as_str() {
                "shareware" => SporeLicense::Shareware,
                "freeware" => SporeLicense::Freeware,
                "open_source" | "opensource" | "open" => SporeLicense::OpenSource,
                "public_domain" | "publicdomain" | "pd" | "cc0" => SporeLicense::PublicDomain,
                _ => SporeLicense::Proprietary,
            };

            let mut catalog = StoreCatalog::open(home).map_err(|e| e.to_string())?;

            let main_file = binary.file_name().unwrap().to_string_lossy().to_string();
            let content_id = mycelium_core::ContentId::of(&bytes);

            let spore = SoftwareSpore {
                id: id.clone(),
                title,
                description: "Publicado via Mycelium Store CLI".to_string(),
                developer_or_publisher: "Comunidade Mycelium".to_string(),
                release_year: 2000,
                platform: plat,
                category: "software".to_string(),
                tags: vec!["p2p".to_string(), "spore".to_string()],
                license: lic,
                main_binary_file: main_file,
                content_id,
                execution_matrix: mycelium_store::ExecutionMatrix {
                    recommended: ExecutionEngineType::Native,
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

            let cid = catalog.publish_spore(spore, &bytes).map_err(|e| e.to_string())?;
            println!("[🍄 Store] Spore '{}' publicado com sucesso! ContentId: {}", id, hex::encode(cid.0));
            Ok(())
        }
    }
}

async fn repo_cmd(home: &PathBuf, action: RepoCmd) -> Result<(), String> {
    match action {
        RepoCmd::Publish { dir, message } => {
            if !dir.is_dir() {
                return Err(format!("diretório não encontrado: {:?}", dir));
            }
            let leaves = pack_tree(&dir)?;
            if leaves.is_empty() {
                return Err("nenhum arquivo para publicar".into());
            }
            let bytes: usize = leaves.iter().map(|l| l.content.len()).sum();
            println!("[🍄 Repo] Empacotando {} arquivos ({} bytes) de {:?}", leaves.len(), bytes, dir);
            println!("[🍄 Repo] Enviando para o daemon (SporeBank + DHT + gossip)...");
            let resp = call(&home.join("mycelium.sock"), Request::RepoPublish { message, leaves }).await?;
            match resp {
                Response::RepoPublished { cid, leaves, bytes } => {
                    println!("[🍄 Repo] ✅ Publicado!");
                    println!("    ContentId : {cid}");
                    println!("    Arquivos  : {leaves}");
                    println!("    Tamanho   : {bytes} bytes");
                    println!();
                    println!("  Distribua este ContentId (via Nostr/DHT). Qualquer nó pode:");
                    println!("    mycelium repo clone --cid {cid} --dest ./copia");
                    println!("    curl http://127.0.0.1:7474/src/{cid}/");
                    Ok(())
                }
                Response::Err { message } => Err(message),
                other => Err(format!("resposta inesperada: {:?}", other)),
            }
        }
        RepoCmd::Clone { cid, dest } => {
            let sock = home.join("mycelium.sock");
            let resp = call(&sock, Request::RepoClone { cid: cid.clone() }).await;
            match resp {
                Ok(Response::RepoCloneResult { message, leaves }) => {
                    println!("[🍄 Repo] {message}");
                    write_tree(&dest, &leaves)?;
                    let bytes: usize = leaves.iter().map(|l| l.content.len()).sum();
                    println!("[🍄 Repo] ✅ Árvore reconstruída em {:?} ({} arquivos, {} bytes)", dest, leaves.len(), bytes);
                    Ok(())
                }
                Ok(Response::Err { message }) => Err(message),
                Ok(other) => Err(format!("resposta inesperada: {:?}", other)),
                Err(e) => {
                    println!("[🍄 Repo] daemon offline ({e}) — tentando SporeBank local...");
                    match clone_from_local_bank(home, &cid) {
                        Some(leaves) => {
                            write_tree(&dest, &leaves)?;
                            let bytes: usize = leaves.iter().map(|l| l.content.len()).sum();
                            println!("[🍄 Repo] ✅ Árvore reconstruída do SporeBank local em {:?} ({} arquivos, {} bytes)", dest, leaves.len(), bytes);
                            Ok(())
                        }
                        None => Err(format!(
                            "repo {} não encontrado localmente. Suba o daemon (mycelium daemon) para buscar via DHT.",
                            cid
                        )),
                    }
                }
            }
        }
        RepoCmd::List => {
            let resp = call(&home.join("mycelium.sock"), Request::Status).await?;
            match resp {
                Response::Status(s) => {
                    println!("\n[🍄 Repo] Plots no SporeBank local: {}\n", s.plots);
                    for id in mycelium_store_list_local(home)? {
                        println!("    • {}", id);
                    }
                    Ok(())
                }
                _ => Err("resposta inesperada no status".into()),
            }
        }
    }
}

fn write_tree(dest: &PathBuf, leaves: &[giggs::Leaf]) -> Result<(), String> {
    std::fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    for leaf in leaves {
        let target = dest.join(&leaf.path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(&target, &leaf.content).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn clone_from_local_bank(home: &PathBuf, cid: &str) -> Option<Vec<giggs::Leaf>> {
    let hex_str = cid.strip_prefix("Qm").unwrap_or(cid);
    let plot_file = home.join("sporebank").join("plots").join(format!("{hex_str}.json"));
    let bytes = std::fs::read(&plot_file).ok()?;
    let plot: giggs::Plot = serde_json::from_slice(&bytes).ok()?;
    Some(plot.leaves)
}

fn pack_tree(dir: &PathBuf) -> Result<Vec<giggs::Leaf>, String> {
    fn skip(name: &str) -> bool {
        matches!(
            name,
            ".git" | "target" | "node_modules" | "dist" | "build" | ".cache"
                | "__pycache__" | ".venv" | "coverage" | ".data" | "graphify-out"
        )
    }
    fn walk(dir: &PathBuf, prefix: &str, out: &mut Vec<giggs::Leaf>) -> Result<(), String> {
        let entries = std::fs::read_dir(dir).map_err(|e| format!("ler {:?}: {e}", dir))?;
        for entry in entries {
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry.file_name().to_string_lossy().to_string();
            if skip(&name) {
                continue;
            }
            let rel = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            };
            let path = entry.path();
            if path.is_dir() {
                walk(&path, &rel, out)?;
            } else {
                let content = std::fs::read(&path).map_err(|e| format!("ler {:?}: {e}", path))?;
                out.push(giggs::Leaf { path: rel, content });
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(dir, "", &mut out)?;
    Ok(out)
}

fn mycelium_store_list_local(home: &PathBuf) -> Result<Vec<String>, String> {
    let bank_dir = home.join("sporebank").join("plots");
    let mut ids: Vec<String> = std::fs::read_dir(&bank_dir)
        .map_err(|e| format!("abrir sporebank: {e}"))?
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.strip_suffix(".json").map(|s| s.to_string())
        })
        .collect();
    ids.sort();
    Ok(ids)
}
