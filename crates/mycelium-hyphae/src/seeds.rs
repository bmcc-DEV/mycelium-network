//! Seed book — bootstrap público além da LAN.
//!
//! Fontes (em ordem de merge):
//! 1. Multiaddrs embutidos / passados na CLI
//! 2. Arquivo local (`seeds.txt` no home do nó, ou `--seed-file`)
//! 3. DNS TXT (`MYCELIUM_DNS_SEEDS` / `--dns` / public bootstrap)
//! 4. URL HTTP(S) (`MYCELIUM_BOOTSTRAP_URL` ou `--public-bootstrap`)
//!
//! Formato do arquivo / TXT (uma entrada por linha ou por string TXT):
//! ```text
//! # comentário
//! /ip4/203.0.113.10/tcp/4001/p2p/12D3KooW.../raiz
//! /ip6/2001:db8::1/tcp/4001/p2p/12D3KooW.../floresta
//! /ip6/2001:db8::2/tcp/4001/p2p/12D3KooW.../esporocarp
//! mycelium=/ip6/2001:db8::1/tcp/4001/p2p/12D3KooW...
//! /dnsaddr/bootstrap.mycelium.network
//! ```
//!
//! Sufixos de membrana (`/floresta|/raiz|/folha|/esporocarp`) são opcionais
//! (legado sem flag = ordenação IPv6-first clássica).

use crate::membrane::seed_dial_rank;
use crate::HyphaeError;
use hickory_resolver::config::{ResolverConfig, ResolverOpts};
use hickory_resolver::Resolver;
use libp2p::Multiaddr;
use mycelium_core::Membrane;
use std::collections::BTreeSet;
use std::path::Path;

/// URL padrão do catálogo público (sobrescrevível).
pub const DEFAULT_BOOTSTRAP_URL: &str =
    "https://raw.githubusercontent.com/bmcc-DEV/mycelium-network/main/seeds/mainnet.txt";

/// Nome DNS TXT padrão do Spore Bank (DuckDNS / HE / Cloudflare).
pub const DEFAULT_DNS_SEED_NAME: &str = "_mycelium.seeds.duckdns.org";

/// Separates multiaddr from optional membrane suffix.
pub fn split_membrane_suffix(raw: &str) -> (&str, Option<Membrane>) {
    for (suf, m) in [
        ("/esporocarp", Membrane::Esporocarp),
        ("/floresta", Membrane::Floresta),
        ("/raiz", Membrane::Raiz),
        ("/folha", Membrane::Folha),
    ] {
        if let Some(rest) = raw.strip_suffix(suf) {
            return (rest, Some(m));
        }
    }
    (raw, None)
}

/// Anexa flag de membrana a uma multiaddr (Spore Bank publish).
pub fn with_membrane_flag(multiaddr: &str, membrane: Membrane) -> String {
    let (base, _) = split_membrane_suffix(multiaddr.trim());
    format!("{base}{}", membrane.seed_suffix())
}

/// Livro de sementes: peers conhecidos para bootstrap remoto.
#[derive(Debug, Clone, Default)]
pub struct SeedBook {
    /// Entradas com possível sufixo `/floresta` etc.
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
        let mut s = addr.as_ref().trim().to_string();
        if s.is_empty() || s.starts_with('#') {
            return Ok(());
        }
        if let Some(rest) = s.strip_prefix("mycelium=") {
            s = rest.trim().to_string();
        }
        let (base, flag) = split_membrane_suffix(&s);
        let _: Multiaddr = base
            .parse()
            .map_err(|e| HyphaeError::Addr(format!("{base}: {e}")))?;
        let stored = if let Some(m) = flag {
            with_membrane_flag(base, m)
        } else {
            base.to_string()
        };
        self.seeds.insert(stored);
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

    /// Resolve registros TXT e importa multiaddrs (Spore Bank DNS).
    pub fn fetch_dns_txt(&mut self, name: &str) -> Result<usize, HyphaeError> {
        let resolver = Resolver::new(ResolverConfig::default(), ResolverOpts::default())
            .map_err(|e| HyphaeError::Addr(format!("dns resolver: {e}")))?;
        let response = resolver
            .txt_lookup(name)
            .map_err(|e| HyphaeError::Addr(format!("dns TXT {name}: {e}")))?;
        let before = self.seeds.len();
        for record in response.iter() {
            let text: String = record
                .txt_data()
                .iter()
                .map(|b| String::from_utf8_lossy(b).into_owned())
                .collect::<Vec<_>>()
                .join("");
            for part in text.split(|c: char| c == '\n' || c == ';' || c == ',') {
                let _ = self.add(part.trim());
            }
        }
        Ok(self.seeds.len() - before)
    }

    /// Publica uma multiaddr no DuckDNS TXT (`DUCKDNS_TOKEN` + domain).
    pub fn publish_duckdns_txt(domain: &str, token: &str, multiaddr: &str) -> Result<(), HyphaeError> {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .user_agent("mycelium-sporocarp/0.1")
            .build()
            .map_err(|e| HyphaeError::Addr(e.to_string()))?;
        let domain = domain
            .trim()
            .trim_end_matches(".duckdns.org")
            .trim_end_matches('.');
        let body = client
            .get("https://www.duckdns.org/update")
            .query(&[
                ("domains", domain),
                ("token", token),
                ("txt", multiaddr),
                ("verbose", "true"),
            ])
            .send()
            .and_then(|r| r.error_for_status()?.text())
            .map_err(|e| HyphaeError::Addr(format!("duckdns: {e}")))?;
        if body.to_ascii_lowercase().contains("ok") {
            tracing::info!(%domain, "DuckDNS TXT atualizado (spore bank)");
            Ok(())
        } else {
            Err(HyphaeError::Addr(format!("duckdns resposta: {body}")))
        }
    }

    /// Multiaddrs para dial, filtrados/ordenados pela membrana local.
    pub fn multiaddrs_for(&self, local: Membrane) -> Vec<Multiaddr> {
        let mut ranked: Vec<(u8, Multiaddr)> = Vec::new();
        for entry in &self.seeds {
            let (base, remote) = split_membrane_suffix(entry);
            let Some(rank) = seed_dial_rank(local, remote) else {
                continue;
            };
            if let Ok(addr) = base.parse::<Multiaddr>() {
                ranked.push((rank, addr));
            }
        }
        ranked.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then_with(|| crate::addr_family_rank(&a.1).cmp(&crate::addr_family_rank(&b.1)))
                .then_with(|| a.1.to_string().cmp(&b.1.to_string()))
        });
        ranked.into_iter().map(|(_, a)| a).collect()
    }

    /// Multiaddrs prontos para dial (legado: IPv6 primeiro, sem filtro de folha).
    pub fn multiaddrs(&self) -> Vec<Multiaddr> {
        self.multiaddrs_for(Membrane::Floresta)
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

        let dns_name = std::env::var("MYCELIUM_DNS_SEEDS").ok();
        if public_bootstrap || dns_name.is_some() {
            let name = dns_name
                .as_deref()
                .unwrap_or(DEFAULT_DNS_SEED_NAME);
            match book.fetch_dns_txt(name) {
                Ok(n) => tracing::info!(%name, added = n, "DNS TXT Spore Bank carregado"),
                Err(e) => tracing::warn!(%name, "DNS TXT Spore Bank: {e}"),
            }
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
    fn accepts_mycelium_prefix_and_ipv6_sort() {
        let mut book = SeedBook::new();
        book.add("/ip4/203.0.113.1/tcp/4001").unwrap();
        book.add("/ip6/2001:db8::1/tcp/4001").unwrap();
        let addrs = book.multiaddrs();
        assert!(addrs[0].to_string().starts_with("/ip6/"));
    }

    #[test]
    fn accepts_dnsaddr() {
        let mut book = SeedBook::new();
        book.add("/dnsaddr/bootstrap.mycelium.network").unwrap();
        assert_eq!(book.len(), 1);
    }

    #[test]
    fn parse_txt_blob() {
        let mut book = SeedBook::new();
        book.parse_text("mycelium=/ip4/9.9.9.9/tcp/1\n/ip6/::1/tcp/2\n")
            .unwrap();
        assert_eq!(book.len(), 2);
    }

    #[test]
    fn membrane_flags_parse_and_filter() {
        let mut book = SeedBook::new();
        book.add("/ip6/2001:db8::1/tcp/4001/floresta").unwrap();
        book.add("/ip4/203.0.113.5/tcp/4001/raiz").unwrap();
        book.add("/ip4/198.51.100.1/tcp/4001/folha").unwrap();
        book.add("/ip6/2001:db8::2/tcp/4001/esporocarp").unwrap();

        let for_folha = book.multiaddrs_for(Membrane::Folha);
        assert!(for_folha.iter().all(|a| {
            let s = a.to_string();
            !s.contains("198.51.100.1")
        }));
        assert!(for_folha[0].to_string().contains("2001:db8::2"));

        let for_floresta = book.multiaddrs_for(Membrane::Floresta);
        assert!(for_floresta[0].to_string().contains("2001:db8::1"));
        assert!(!for_floresta.iter().any(|a| a.to_string().contains("198.51.100.1")));
    }

    #[test]
    fn with_flag_roundtrip() {
        let s = with_membrane_flag("/ip6/2001:db8::1/tcp/4001", Membrane::Esporocarp);
        assert_eq!(s, "/ip6/2001:db8::1/tcp/4001/esporocarp");
        let (base, m) = split_membrane_suffix(&s);
        assert_eq!(base, "/ip6/2001:db8::1/tcp/4001");
        assert_eq!(m, Some(Membrane::Esporocarp));
    }
}
