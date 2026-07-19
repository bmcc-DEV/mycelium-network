//! Persistência em disco do organismo.

use mycelium_core::Resources;
use mycelium_hyphae::HyphaMetrics;
use mycelium_nutrients::Ledger;
use mycelium_pheromones::Gland;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thefield::Field;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("codec: {0}")]
    Codec(#[from] serde_json::Error),
    #[error("{0}")]
    Msg(String),
}

/// Ion persistido — suficiente para re-frutificar a Chamber no reboot.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IonRecord {
    pub name: String,
    pub plot: String,
    pub pipeline: String,
}

/// Estado persistido do TheField + Ions implantados + métricas.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct OrganismState {
    pub field: Field,
    pub ions: Vec<IonRecord>,
    pub hypha_metrics: HyphaMetrics,
    pub bootstrap: Vec<String>,
    pub processed_signals: Vec<String>,
    /// Porta do Event Horizon HTTP (default 7474).
    #[serde(default = "default_horizon_port")]
    pub horizon_port: u16,
}

fn default_horizon_port() -> u16 {
    7474
}

/// Home do nó: identidade, ledger, resources, estado do organismo.
pub struct NodeStore {
    pub root: PathBuf,
}

impl NodeStore {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, StoreError> {
        let root = root.as_ref().to_path_buf();
        std::fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    pub fn socket_path(&self) -> PathBuf {
        self.root.join("mycelium.sock")
    }

    pub fn pid_path(&self) -> PathBuf {
        self.root.join("mycelium.pid")
    }

    pub fn listen_addrs_path(&self) -> PathBuf {
        self.root.join("listen_addrs.json")
    }

    pub fn chambers_dir(&self) -> PathBuf {
        self.root.join("chambers")
    }

    fn seed_path(&self) -> PathBuf {
        self.root.join("gland.seed")
    }

    fn ledger_path(&self) -> PathBuf {
        self.root.join("ledger.json")
    }

    fn resources_path(&self) -> PathBuf {
        self.root.join("resources.json")
    }

    fn state_path(&self) -> PathBuf {
        self.root.join("organism.json")
    }

    pub fn load_or_create_gland(&self) -> Result<Gland, StoreError> {
        let path = self.seed_path();
        if path.exists() {
            let bytes = std::fs::read(&path)?;
            if bytes.len() != 32 {
                return Err(StoreError::Msg(
                    "gland.seed corrompido (esperado 32 bytes)".into(),
                ));
            }
            let mut seed = [0u8; 32];
            seed.copy_from_slice(&bytes);
            Ok(Gland::from_seed(seed))
        } else {
            let gland = Gland::germinate();
            std::fs::write(&path, gland.seed())?;
            Ok(gland)
        }
    }

    pub fn load_ledger(&self) -> Ledger {
        std::fs::read(self.ledger_path())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save_ledger(&self, ledger: &Ledger) -> Result<(), StoreError> {
        std::fs::write(self.ledger_path(), serde_json::to_vec_pretty(ledger)?)?;
        Ok(())
    }

    pub fn save_resources(&self, resources: &Resources) -> Result<(), StoreError> {
        std::fs::write(
            self.resources_path(),
            serde_json::to_vec_pretty(resources)?,
        )?;
        Ok(())
    }

    pub fn load_resources(&self) -> Option<Resources> {
        std::fs::read(self.resources_path())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
    }

    pub fn load_state(&self) -> OrganismState {
        let mut state: OrganismState = std::fs::read(self.state_path())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        if state.horizon_port == 0 {
            state.horizon_port = default_horizon_port();
        }
        state
    }

    pub fn save_state(&self, state: &OrganismState) -> Result<(), StoreError> {
        std::fs::write(self.state_path(), serde_json::to_vec_pretty(state)?)?;
        Ok(())
    }

    pub fn save_listen_addrs(&self, addrs: &[String]) -> Result<(), StoreError> {
        std::fs::write(self.listen_addrs_path(), serde_json::to_vec_pretty(addrs)?)?;
        Ok(())
    }

    pub fn load_listen_addrs(&self) -> Vec<String> {
        std::fs::read(self.listen_addrs_path())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn write_pid(&self) -> Result<(), StoreError> {
        std::fs::write(self.pid_path(), std::process::id().to_string())?;
        Ok(())
    }

    pub fn clear_runtime_files(&self) {
        let _ = std::fs::remove_file(self.socket_path());
        let _ = std::fs::remove_file(self.pid_path());
    }
}
