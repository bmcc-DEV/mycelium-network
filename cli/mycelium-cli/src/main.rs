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
        Commands::Candidate {
            cmd,
            r#loop,
            once: _,
            relays,
        } => rt.block_on(candidate_cmd(&home, cmd, r#loop, relays)),
        Commands::ChamberServe { port, ion, root } => {
            rt.block_on(chamber_serve(port, ion, root))
        }
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
