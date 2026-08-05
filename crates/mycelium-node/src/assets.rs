//! # Registro RWA — ativos do mundo real (Fase 3)
//!
//! Ativos físicos (estúdios, espaços maker, veículos, colecionáveis) e
//! empresas/cooperativas (Fase 4) são registados com cotas fracionadas
//! ("Mycelial Equity"). A transferência de cotas gera royalty automático
//! ao autor original. Persistência: `{home}/assets.json`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Categoria de ativo físico / empresa.
#[derive(Clone, Debug, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    /// Estúdio de gravação / espaço criativo.
    Studio,
    /// Espaço maker / laboratório.
    Maker,
    /// Imóvel de uso partilhado.
    RealEstate,
    /// Veículo de logística local.
    Vehicle,
    /// Colecionável / item físico (phygital).
    Collectible,
    /// Empresa / cooperativa independente (Fase 4).
    Company,
}

impl AssetKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Studio => "studio",
            Self::Maker => "maker",
            Self::RealEstate => "real_estate",
            Self::Vehicle => "vehicle",
            Self::Collectible => "collectible",
            Self::Company => "company",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "studio" => Some(AssetKind::Studio),
            "maker" => Some(AssetKind::Maker),
            "real_estate" => Some(AssetKind::RealEstate),
            "vehicle" => Some(AssetKind::Vehicle),
            "collectible" => Some(AssetKind::Collectible),
            "company" => Some(AssetKind::Company),
            _ => None,
        }
    }
}

impl std::str::FromStr for AssetKind {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_str(s).ok_or_else(|| format!("kind inválido: {}", s))
    }
}

/// Um ativo físico / empresa com cotas fracionadas.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssetRecord {
    pub id: String,
    pub name: String,
    pub kind: AssetKind,
    pub description: String,
    pub location: Option<String>,
    /// Total de cotas emitidas.
    pub shares_total: u64,
    /// Preço de 1 cota em nutrientes (Atp).
    pub price_per_share: u64,
    /// GhostID pubkey x-only do registante/autor.
    pub owner: [u8; 32],
}

/// Cota detida por um holder.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShareHolding {
    pub holder: [u8; 32],
    pub shares: u64,
}

/// O registo completo.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AssetRegistry {
    pub assets: Vec<AssetRecord>,
    pub holdings: HashMap<String, Vec<ShareHolding>>,
}

impl AssetRegistry {
    pub fn open(home: impl AsRef<Path>) -> Result<Self, String> {
        let path = home.as_ref().join("assets.json");
        if path.exists() {
            let bytes = std::fs::read(&path).map_err(|e| format!("ler {}: {e}", path.display()))?;
            serde_json::from_slice(&bytes).map_err(|e| format!("parse {}: {e}", path.display()))
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self, home: impl AsRef<Path>) -> Result<(), String> {
        let path = home.as_ref().join("assets.json");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
        }
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| format!("encode: {e}"))?;
        std::fs::write(&path, bytes).map_err(|e| format!("gravar {}: {e}", path.display()))
    }

    /// Regista um ativo e emite todas as cotas ao registante.
    pub fn register(
        &mut self,
        record: AssetRecord,
    ) -> Result<(), String> {
        if self.assets.iter().any(|a| a.id == record.id) {
            return Err(format!("ativo '{}' já registado", record.id));
        }
        self.holdings.insert(
            record.id.clone(),
            vec![ShareHolding {
                holder: record.owner,
                shares: record.shares_total,
            }],
        );
        self.assets.push(record);
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&AssetRecord> {
        self.assets.iter().find(|a| a.id == id)
    }

    /// Transfere cotas entre holders.
    pub fn transfer_shares(
        &mut self,
        asset_id: &str,
        from: &[u8; 32],
        to: &[u8; 32],
        shares: u64,
    ) -> Result<(), String> {
        let asset = self
            .get(asset_id)
            .ok_or_else(|| format!("ativo '{asset_id}' não encontrado"))?;
        let _ = asset; // confirm asset exists

        let entry = self.holdings.entry(asset_id.to_string()).or_default();

        let from_idx = entry
            .iter()
            .position(|h| h.holder == *from)
            .ok_or_else(|| "remetente não detém cotas deste ativo".to_string())?;
        if entry[from_idx].shares < shares {
            return Err("cota insuficiente".to_string());
        }
        entry[from_idx].shares -= shares;
        if entry[from_idx].shares == 0 {
            entry.remove(from_idx);
        }
        if let Some(h) = entry.iter_mut().find(|h| h.holder == *to) {
            h.shares += shares;
        } else {
            entry.push(ShareHolding {
                holder: *to,
                shares,
            });
        }
        Ok(())
    }

    pub fn holdings_of(&self, asset_id: &str) -> Vec<ShareHolding> {
        self.holdings
            .get(asset_id)
            .map(|v| v.iter().cloned().collect())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reg(home: &Path) -> AssetRegistry {
        let mut r = AssetRegistry::default();
        r.register(AssetRecord {
            id: "estudio-x".into(),
            name: "Estúdio X".into(),
            kind: AssetKind::Studio,
            description: "estúdio de gravação".into(),
            location: Some("SP".into()),
            shares_total: 100,
            price_per_share: 5,
            owner: [1u8; 32],
        })
        .unwrap();
        r
    }

    #[test]
    fn register_and_transfer_shares() {
        let home = std::env::temp_dir().join(format!(
            "assets-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut r = reg(&home);
        assert_eq!(r.holdings_of("estudio-x")[0].shares, 100);
        r.transfer_shares("estudio-x", &[1u8; 32], &[2u8; 32], 30)
            .unwrap();
        assert_eq!(r.holdings_of("estudio-x")[0].shares, 70);
        assert_eq!(r.holdings_of("estudio-x")[1].shares, 30);
        r.save(&home).unwrap();
        let loaded = AssetRegistry::open(&home).unwrap();
        assert_eq!(loaded.assets.len(), 1);
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn cannot_overdraw() {
        let home = std::env::temp_dir().join("assets-overdraw");
        let mut r = reg(&home);
        assert!(r
            .transfer_shares("estudio-x", &[1u8; 32], &[2u8; 32], 999)
            .is_err());
        std::fs::remove_dir_all(&home).ok();
    }
}
