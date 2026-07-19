//! # mycelium-node
//!
//! O organismo do nó: daemon persistente que une hifas, Spore Bank,
//! TheField, Inertia, Vacuum e Plasma — com estado em disco e plano de
//! controle via Unix socket.

mod control;
mod organism;
mod protocol;
mod store;

pub use control::{call, serve, Request, Response, StatusReport};
pub use organism::{Organism, OrganismConfig, OrganismError};
pub use protocol::Envelope;
pub use store::{NodeStore, OrganismState};

use std::path::PathBuf;
use tokio::sync::mpsc;

/// Opções para despertar o daemon.
#[derive(Debug, Clone, Default)]
pub struct DaemonOptions {
    pub contribute: Option<mycelium_core::Resources>,
    pub bootstrap: Vec<String>,
    pub horizon_port: u16,
    pub listen: Vec<String>,
    pub seed_file: Option<PathBuf>,
    pub public_bootstrap: bool,
    pub bootstrap_url: Option<String>,
    /// Se true, desliga mDNS (só seeds/bootstrap).
    pub no_mdns: bool,
    /// IP público anunciado quando listen é 0.0.0.0.
    pub announce_ip: Option<String>,
}

/// Desperta o daemon: socket de controle + loop do organismo.
pub async fn run_daemon(home: PathBuf, opts: DaemonOptions) -> Result<(), OrganismError> {
    let organism = Organism::awaken(OrganismConfig {
        home: home.clone(),
        contribute: opts.contribute,
        bootstrap: opts.bootstrap,
        horizon_port: opts.horizon_port,
        listen: opts.listen,
        seed_file: opts.seed_file,
        public_bootstrap: opts.public_bootstrap,
        bootstrap_url: opts.bootstrap_url,
        enable_mdns: !opts.no_mdns,
        announce_ip: opts.announce_ip,
    })?;
    let sock = organism.home().join("mycelium.sock");
    let token = std::env::var("MYCELIUM_CONTROL_TOKEN").ok().filter(|t| !t.is_empty());
    let (tx, rx) = mpsc::channel(32);

    let serve_sock = sock.clone();
    tokio::spawn(async move {
        if let Err(e) = serve(&serve_sock, tx, token).await {
            tracing::error!("control socket: {e}");
        }
    });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    organism.run(rx).await
}
