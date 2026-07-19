//! Seed book — bootstrap público além da LAN.
//!
//! Fontes (em ordem de merge):
//! 1. Multiaddrs embutidos / passados na CLI
//! 2. Arquivo local (`seeds.txt` no home do nó, ou `--seed-file`)
//! 3. URL HTTP(S) (`MYCELIUM_BOOTSTRAP_URL` ou `--public-bootstrap`)
//!
//! Formato do arquivo (uma entrada por linha):
//! ```text
//! # comentário
//! /ip4/203.0.113.10/tcp/4001/p2p/12D3KooW...
//! /dnsaddr/bootstrap.mycelium.network
//! ```

use crate::HyphaeError;
use libp2p::Multiaddr;
use std::collections::BTreeSet;
use std::path::Path;

/// URL padrão do catálogo público (sobrescrevível).
/// Aponta ao `seeds/mainnet.txt` deste monorepo quando publicado no GitHub.
pub const DEFAULT_BOOTSTRAP_URL: &str =
    "https://raw.githubusercontent.com/bmcc-DEV/mycelium-network/main/seeds/mainnet.txt";

/// Livro de sementes: peers conhecidos para bootstrap remoto.
#[derive(Debug, Clone, Default)]
pub struct SeedBook {
    seeds: BTreeSet<String>,
}

impl SeedBook {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.seeds.len()
    }

    pub fn is_empty(&self) -> bool {
        self.seeds.is_empty()
    }

    pub fn add(&mut self, addr: impl AsRef<str>) -> Result<(), HyphaeError> {
        let s = addr.as_ref().trim();
        if s.is_empty() || s.starts_with('#') {
            return Ok(());
        }
        let _: Multiaddr = s
            .parse()
            .map_err(|e| HyphaeError::Addr(format!("{s}: {e}")))?;
        self.seeds.insert(s.to_string());
        Ok(())
    }

    pub fn extend_str<I, S>(&mut self, iter: I) -> Result<(), HyphaeError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        for s in iter {
            self.add(s)?;
        }
        Ok(())
    }

    /// Carrega linhas de um arquivo texto.
    pub fn load_file(&mut self, path: impl AsRef<Path>) -> Result<usize, HyphaeError> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(0);
        }
        let text = std::fs::read_to_string(path)
            .map_err(|e| HyphaeError::Addr(format!("lendo {}: {e}", path.display())))?;
        let before = self.seeds.len();
        self.parse_text(&text)?;
        Ok(self.seeds.len() - before)
    }

    /// Persiste o livro em disco.
    pub fn save_file(&self, path: impl AsRef<Path>) -> Result<(), HyphaeError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| HyphaeError::Addr(e.to_string()))?;
        }
        let mut body = String::from("# Mycelium seed book\n");
        for s in &self.seeds {
            body.push_str(s);
            body.push('\n');
        }
        std::fs::write(path, body).map_err(|e| HyphaeError::Addr(e.to_string()))?;
        Ok(())
    }

    pub fn parse_text(&mut self, text: &str) -> Result<(), HyphaeError> {
        for line in text.lines() {
            self.add(line)?;
        }
        Ok(())
    }

    /// Baixa um catálogo HTTP(S) de seeds (bloqueante — chamar em task).
    pub fn fetch_url(&mut self, url: &str) -> Result<usize, HyphaeError> {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .user_agent("mycelium-seedbook/0.1")
            .build()
            .map_err(|e| HyphaeError::Addr(e.to_string()))?;
        let text = client
            .get(url)
            .send()
            .and_then(|r| r.error_for_status()?.text())
            .map_err(|e| HyphaeError::Addr(format!("bootstrap url {url}: {e}")))?;
        let before = self.seeds.len();
        self.parse_text(&text)?;
        Ok(self.seeds.len() - before)
    }

    /// Multiaddrs prontos para [`crate::HyphaeNode::reach`].
    pub fn multiaddrs(&self) -> Vec<Multiaddr> {
        self.seeds
            .iter()
            .filter_map(|s| s.parse().ok())
            .collect()
    }

    pub fn as_strings(&self) -> Vec<String> {
        self.seeds.iter().cloned().collect()
    }

    /// Monta o livro a partir das fontes padrão do nó.
    pub fn assemble(
        home: &Path,
        cli_seeds: &[String],
        seed_file: Option<&Path>,
        public_bootstrap: bool,
        bootstrap_url: Option<&str>,
    ) -> Result<Self, HyphaeError> {
        let mut book = SeedBook::new();
        book.extend_str(cli_seeds.iter().map(|s| s.as_str()))?;

        let home_seeds = home.join("seeds.txt");
        book.load_file(&home_seeds)?;

        if let Some(path) = seed_file {
            book.load_file(path)?;
        }

        if public_bootstrap {
            let url = bootstrap_url.unwrap_or(DEFAULT_BOOTSTRAP_URL);
            match book.fetch_url(url) {
                Ok(n) => tracing::info!(%url, added = n, "catálogo público de seeds carregado"),
                Err(e) => tracing::warn!(%url, "falha ao buscar seeds públicos: {e}"),
            }
        } else if let Ok(url) = std::env::var("MYCELIUM_BOOTSTRAP_URL") {
            match book.fetch_url(&url) {
                Ok(n) => tracing::info!(%url, added = n, "MYCELIUM_BOOTSTRAP_URL carregada"),
                Err(e) => tracing::warn!(%url, "MYCELIUM_BOOTSTRAP_URL falhou: {e}"),
            }
        }

        Ok(book)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_seed_file_ignoring_comments() {
        let mut book = SeedBook::new();
        book.parse_text("# hi\n\n/dnsaddr/bootstrap.mycelium.network\n")
            .unwrap();
        assert_eq!(book.len(), 1);
    }

    #[test]
    fn rejects_garbage() {
        let mut book = SeedBook::new();
        assert!(book.add("not-a-multiaddr").is_err());
    }

    #[test]
    fn accepts_dnsaddr() {
        let mut book = SeedBook::new();
        book.add("/dnsaddr/bootstrap.mycelium.network").unwrap();
        assert_eq!(book.len(), 1);
    }

    #[test]
    fn roundtrip_file() {
        let dir = std::env::temp_dir().join(format!(
            "seeds-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("seeds.txt");
        let mut a = SeedBook::new();
        a.add("/dnsaddr/bootstrap.mycelium.network").unwrap();
        a.save_file(&path).unwrap();
        let mut b = SeedBook::new();
        assert_eq!(b.load_file(&path).unwrap(), 1);
        assert_eq!(a.as_strings(), b.as_strings());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
