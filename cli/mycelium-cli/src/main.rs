//! # mycelium — CLI do substrato vivo

use axum::routing::get;
use axum::{Json, Router};
use clap::{Parser, Subcommand};
use mycelium_core::Resources;
use mycelium_hyphae::{SeedBook, DEFAULT_BOOTSTRAP_URL};
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
    },
    Status,
    Sow {
        #[arg(long, default_value = "init")]
        message: String,
        #[arg(long, default_value = "main.rs")]
        path: String,
        #[arg(long, default_value = "fn main() {}")]
        content: String,
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
    /// Lê estado do Nucleus Isotope local.
    IsotopeGet {
        #[arg(long)]
        key: String,
    },
    Shutdown,
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
enum SeedsCmd {
    /// Lista seeds em `{home}/seeds.txt`.
    List,
    /// Adiciona uma multiaddr ao seed book.
    Add { addr: String },
    /// Baixa o catálogo público e mescla no seed book.
    Fetch {
        #[arg(long)]
        url: Option<String>,
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
            },
        )),
        Commands::Status => rt.block_on(status(&home)),
        Commands::Sow {
            message,
            path,
            content,
        } => rt.block_on(rpc(
            &home,
            Request::Sow {
                message,
                path,
                content,
            },
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
        Commands::Recall { plot } => rt.block_on(rpc(&home, Request::Recall { plot })),
        Commands::Bootstrap { addr } => {
            rt.block_on(rpc(&home, Request::Bootstrap { addr }))
        }
        Commands::Seeds { action } => seeds_cmd(&home, action),
        Commands::IsotopePut { key, value, clock } => rt.block_on(rpc(
            &home,
            Request::IsotopePut { key, value, clock },
        )),
        Commands::IsotopeGet { key } => {
            rt.block_on(rpc(&home, Request::IsotopeGet { key }))
        }
        Commands::Shutdown => rt.block_on(rpc(&home, Request::Shutdown)),
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
        SeedsCmd::Fetch { url } => {
            let url = url.unwrap_or_else(|| DEFAULT_BOOTSTRAP_URL.to_string());
            let mut book = SeedBook::new();
            book.load_file(&path).map_err(|e| e.to_string())?;
            let n = book.fetch_url(&url).map_err(|e| e.to_string())?;
            book.save_file(&path).map_err(|e| e.to_string())?;
            println!("[🍄] +{n} seeds de {url} → {}", path.display());
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
) -> Result<(), String> {
    let resources = Resources::from_str(contribute).map_err(|e| e.to_string())?;
    opts.contribute = Some(resources);
    println!("[🍄] Despertando organismo em {}…", home.display());
    println!("[🍄] Event Horizon em http://127.0.0.1:{}/", opts.horizon_port);
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
        println!("[🍄] Announce IP: {ip}");
    }
    if !opts.listen.is_empty() {
        println!("[🍄] Listen: {:?}", opts.listen);
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
    if sock.exists() {
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

async fn rpc(home: &PathBuf, request: Request) -> Result<(), String> {
    let sock = home.join("mycelium.sock");
    print_response(call(&sock, request).await?)
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
