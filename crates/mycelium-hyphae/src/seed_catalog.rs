//! Catálogo de seeds — públicas e privadas.
//!
//! Um seed público é um nó alcançável a partir da internet (inbound verificado),
//! seguro de anunciar a qualquer nó. Um seed privado é um nó de LAN / convite /
//! organização interna — só entra no bootstrap local, nunca no catálogo público.
//!
//! Ficheiro: `{home}/seeds/catalog.json`.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Visibilidade de um seed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SeedVisibility {
    Public,
    Private,
}

impl SeedVisibility {
    pub fn as_str(&self) -> &'static str {
        match self {
            SeedVisibility::Public => "public",
            SeedVisibility::Private => "private",
        }
    }
}

/// Entrada do catálogo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeedEntry {
    /// Identificador estável (ex.: `seed-eu-01`).
    pub id: String,
    /// Nome amigável do operador / nó.
    pub name: String,
    /// Multiaddr completa (com `/p2p/<PeerId>` e sufixo de membrana opcional).
    pub multiaddr: String,
    /// Público (bootstrap global) ou privado (bootstrap local / convite).
    pub visibility: SeedVisibility,
    /// Membrana do seed (`esporocarp`, `floresta`, `raiz`, `folha`).
    pub membrane: Option<String>,
    /// Região do operador (ex.: `EU-WEST`, `BR-SP`).
    pub region: Option<String>,
    /// Operador responsável (ex.: `core`, `comunidade`).
    pub operator: Option<String>,
    /// Se o seed opera como circuit relay v2.
    pub relay: bool,
    /// Inbound verificado via `verify-sporocarp.sh`.
    pub verified: bool,
    /// Última vez que o inbound foi confirmado (ISO 8601).
    pub last_seen: Option<String>,
    /// Notas livres.
    pub notes: Option<String>,
}

/// O catálogo completo (públicas + privadas) num JSON.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SeedCatalog {
    pub seeds: Vec<SeedEntry>,
}

impl SeedCatalog {
    pub fn catalog_path(home: &Path) -> std::path::PathBuf {
        home.join("seeds").join("catalog.json")
    }

    /// Abre (ou cria vazio) o catálogo em `{home}/seeds/catalog.json`.
    pub fn open(home: impl AsRef<Path>) -> Result<Self, String> {
        let path = Self::catalog_path(home.as_ref());
        if path.exists() {
            let bytes = std::fs::read(&path).map_err(|e| format!("ler {}: {e}", path.display()))?;
            serde_json::from_slice(&bytes).map_err(|e| format!("parse {}: {e}", path.display()))
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self, home: impl AsRef<Path>) -> Result<(), String> {
        let path = Self::catalog_path(home.as_ref());
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
        }
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| format!("encode: {e}"))?;
        std::fs::write(&path, bytes).map_err(|e| format!("gravar {}: {e}", path.display()))
    }

    /// Adiciona uma entrada; gera `id` estável se ausente. Rejeita multiaddr inválida
    /// ou id duplicado.
    pub fn add(&mut self, mut entry: SeedEntry) -> Result<(), String> {
        if entry.id.trim().is_empty() {
            entry.id = format!(
                "seed-{}",
                mycelium_core::ContentId::of(entry.multiaddr.as_bytes()).short()
            );
        }
        if self.seeds.iter().any(|s| s.id == entry.id) {
            return Err(format!("id duplicado: {}", entry.id));
        }
        let (base, _) = crate::split_membrane_suffix(&entry.multiaddr);
        base.parse::<libp2p::Multiaddr>()
            .map_err(|e| format!("multiaddr inválida: {base}: {e}"))?;
        self.seeds.push(entry);
        Ok(())
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let before = self.seeds.len();
        self.seeds.retain(|s| s.id != id);
        self.seeds.len() != before
    }

    pub fn get(&self, id: &str) -> Option<&SeedEntry> {
        self.seeds.iter().find(|s| s.id == id)
    }

    /// Filtra por visibilidade (`None` = todas).
    pub fn list(&self, visibility: Option<SeedVisibility>) -> Vec<&SeedEntry> {
        match visibility {
            Some(v) => self.seeds.iter().filter(|s| s.visibility == v).collect(),
            None => self.seeds.iter().collect(),
        }
    }

    pub fn public_entries(&self) -> Vec<&SeedEntry> {
        self.list(Some(SeedVisibility::Public))
    }

    pub fn private_entries(&self) -> Vec<&SeedEntry> {
        self.list(Some(SeedVisibility::Private))
    }

    /// Linhas para `seeds/mainnet.txt` (só públicas **verificadas**, com metadados em comentário).
    pub fn to_mainnet_lines(&self) -> Vec<String> {
        let mut out = Vec::new();
        for s in self.public_entries().into_iter().filter(|s| s.verified) {
            let mut comment = s.name.clone();
            if let Some(r) = &s.region {
                comment.push_str(&format!(" ({r})"));
            }
            if let Some(op) = &s.operator {
                comment.push_str(&format!(" — {op}"));
            }
            if s.relay {
                comment.push_str(" [relay]");
            }
            comment.push_str(" [inbound ✓]");
            out.push(format!("# {comment}"));
            out.push(s.multiaddr.clone());
        }
        out
    }

    /// Quantidade de seeds públicas (todas).
    pub fn public_count(&self) -> usize {
        self.public_entries().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> SeedEntry {
        SeedEntry {
            id: "eu-01".into(),
            name: "Core EU".into(),
            multiaddr: "/ip4/203.0.113.10/tcp/4001/p2p/12D3KooWDzvdGJd1c7b6Ec7ftjVrAuwUK95z1ndeCV4m9wcZmfCY/esporocarp".into(),
            visibility: SeedVisibility::Public,
            membrane: Some("esporocarp".into()),
            region: Some("EU-WEST".into()),
            operator: Some("core".into()),
            relay: true,
            verified: true,
            last_seen: Some("2026-08-03".into()),
            notes: None,
        }
    }

    #[test]
    fn persists_roundtrip() {
        let home = std::env::temp_dir().join(format!(
            "seedcat-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        {
            let mut c = SeedCatalog::open(&home).unwrap();
            c.add(sample()).unwrap();
            c.save(&home).unwrap();
        }
        let c = SeedCatalog::open(&home).unwrap();
        assert_eq!(c.seeds.len(), 1);
        assert_eq!(c.public_entries().len(), 1);
        assert_eq!(c.private_entries().len(), 0);
        let lines = c.to_mainnet_lines();
        assert!(lines[0].starts_with("# Core EU"));
        assert!(lines[1].contains("/ip4/203.0.113.10"));
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn private_excluded_from_export() {
        let home = std::env::temp_dir().join("seedcat-priv-test");
        let mut c = SeedCatalog::open(&home).unwrap();
        let mut priv_entry = sample();
        priv_entry.id = "lan-01".into();
        priv_entry.visibility = SeedVisibility::Private;
        c.add(priv_entry).unwrap();
        assert!(c.to_mainnet_lines().is_empty());
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn rejects_invalid_multiaddr() {
        let mut c = SeedCatalog::default();
        let mut bad = sample();
        bad.multiaddr = "não-e-um-multiaddr".into();
        assert!(c.add(bad).is_err());
    }
}
