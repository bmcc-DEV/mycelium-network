//! # mycelium-core
//!
//! Tipos fundamentais do substrato Mycelium Network: identificadores,
//! recursos, esporos e o trait [`FruitingBody`] que todo componente do
//! The Lattice implementa para "brotar" do micélio.

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

/// Identificador de um nó do micélio: hash BLAKE3 da chave pública.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub [u8; 32]);

impl NodeId {
    /// Deriva um `NodeId` a partir de bytes arbitrários (ex.: chave pública).
    pub fn derive(material: &[u8]) -> Self {
        Self(*blake3::hash(material).as_bytes())
    }

    /// Forma curta legível (8 primeiros bytes em hex).
    pub fn short(&self) -> String {
        hex::encode(&self.0[..8])
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

impl fmt::Debug for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NodeId({})", self.short())
    }
}

impl FromStr for NodeId {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = hex::decode(s).map_err(|e| format!("NodeId inválido: {e}"))?;
        if bytes.len() != 32 {
            return Err(format!("NodeId precisa de 32 bytes ({} obtidos)", bytes.len()));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(NodeId(arr))
    }
}

impl Serialize for NodeId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for NodeId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct V;
        impl Visitor<'_> for V {
            type Value = NodeId;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("hex NodeId")
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<NodeId, E> {
                v.parse().map_err(E::custom)
            }
        }
        deserializer.deserialize_str(V)
    }
}

/// Endereço content-addressed usado por Giggs, Vacuum e Isotope.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ContentId(pub [u8; 32]);

impl ContentId {
    pub fn of(bytes: &[u8]) -> Self {
        Self(*blake3::hash(bytes).as_bytes())
    }

    pub fn short(&self) -> String {
        hex::encode(&self.0[..8])
    }
}

impl fmt::Display for ContentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Qm{}", hex::encode(self.0))
    }
}

impl fmt::Debug for ContentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ContentId({})", self.short())
    }
}

/// Aceita hex puro (64 chars) ou prefixo `Qm` + hex.
impl FromStr for ContentId {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let hex_str = s.strip_prefix("Qm").unwrap_or(s);
        let bytes = hex::decode(hex_str).map_err(|e| format!("ContentId inválido: {e}"))?;
        if bytes.len() != 32 {
            return Err(format!(
                "ContentId precisa de 32 bytes ({} obtidos)",
                bytes.len()
            ));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(ContentId(arr))
    }
}

impl Serialize for ContentId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // String form — funciona como chave de HashMap no JSON.
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for ContentId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct V;
        impl Visitor<'_> for V {
            type Value = ContentId;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("ContentId (Qm… ou hex)")
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<ContentId, E> {
                v.parse().map_err(E::custom)
            }
        }
        deserializer.deserialize_str(V)
    }
}

/// Recursos que um nó contribui ao substrato.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Resources {
    /// Núcleos de CPU dedicados a Vectors do Inertia.
    pub cpu_cores: u32,
    /// RAM (em MiB) dedicada a Chambers do Vacuum.
    pub ram_mib: u64,
    /// Storage (em GiB) dedicado a Nuclei do Isotope e Voids do Vacuum.
    pub storage_gib: u64,
    /// Banda (em Mbps) dedicada às hifas e ao Singularity.
    pub bandwidth_mbps: u64,
}

/// Erro de parse de uma declaração de contribuição.
#[derive(Debug, thiserror::Error)]
#[error("contribuição inválida: {0} (esperado ex.: 2cpu,4gb,100gb ou 2cpu,4gb,100gb,50mbps)")]
pub struct ParseResourcesError(String);

impl FromStr for Resources {
    type Err = ParseResourcesError;

    /// Aceita a sintaxe do manifesto: `2cpu,4gb,100gb[,50mbps]`.
    /// O primeiro `gb` é interpretado como RAM, o segundo como storage.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut out = Resources::default();
        let mut saw_ram = false;
        for part in s.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            let lower = part.to_ascii_lowercase();
            let (num, unit) = lower.split_at(
                lower
                    .find(|c: char| !c.is_ascii_digit() && c != '.')
                    .ok_or_else(|| ParseResourcesError(part.into()))?,
            );
            let value: f64 = num.parse().map_err(|_| ParseResourcesError(part.into()))?;
            match unit {
                "cpu" | "cores" => out.cpu_cores = value as u32,
                "gb" | "gib" if !saw_ram => {
                    out.ram_mib = (value * 1024.0) as u64;
                    saw_ram = true;
                }
                "gb" | "gib" => out.storage_gib = value as u64,
                "mb" | "mib" if !saw_ram => {
                    out.ram_mib = value as u64;
                    saw_ram = true;
                }
                "tb" | "tib" => out.storage_gib = (value * 1024.0) as u64,
                "mbps" => out.bandwidth_mbps = value as u64,
                "gbps" => out.bandwidth_mbps = (value * 1000.0) as u64,
                _ => return Err(ParseResourcesError(part.into())),
            }
        }
        Ok(out)
    }
}

/// As cinco moedas bioquímicas do Nutrient Cycling.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Nutrient {
    /// Tokens de energia — recompensa por CPU cycles.
    Atp,
    /// Prioridade de acesso — recompensa por RAM.
    Enzymes,
    /// Direitos de armazenamento futuro — recompensa por storage.
    Mycelia,
    /// Reputação e governança — recompensa por bandwidth.
    Spores,
    /// Imunidade a banimento — recompensa por uptime.
    Resilience,
}

impl Nutrient {
    pub const ALL: [Nutrient; 5] = [
        Nutrient::Atp,
        Nutrient::Enzymes,
        Nutrient::Mycelia,
        Nutrient::Spores,
        Nutrient::Resilience,
    ];
}

impl fmt::Display for Nutrient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Nutrient::Atp => "ATP",
            Nutrient::Enzymes => "Enzymes",
            Nutrient::Mycelia => "Mycelia",
            Nutrient::Spores => "Spores",
            Nutrient::Resilience => "Resilience",
        };
        f.write_str(name)
    }
}

/// Saúde de um corpo de frutificação.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vitality {
    /// Brotando: ainda estabelecendo hifas.
    Sprouting,
    /// Frutificando: servindo tráfego/dados.
    Fruiting,
    /// Dormente: hibernando (ex.: Sclerotium em cold storage).
    Dormant,
    /// Decomposto: recursos devolvidos ao substrato.
    Decomposed,
}

/// Todo serviço do The Lattice é um corpo de frutificação: o cogumelo
/// visível que emerge do substrato invisível.
pub trait FruitingBody {
    /// Nome do fruto (ex.: "sporocarp", "chamber", "event-horizon").
    fn kind(&self) -> &'static str;

    /// Estado vital atual.
    fn vitality(&self) -> Vitality;

    /// Nutrientes que este fruto consome do substrato para viver.
    fn diet(&self) -> Vec<Nutrient>;

    /// Decompõe o fruto, devolvendo os recursos ao micélio.
    fn decompose(&mut self);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_id_is_deterministic() {
        assert_eq!(NodeId::derive(b"spore"), NodeId::derive(b"spore"));
        assert_ne!(NodeId::derive(b"spore"), NodeId::derive(b"mold"));
    }

    #[test]
    fn parses_manifesto_contribution_syntax() {
        let r: Resources = "2cpu,4gb,100gb".parse().unwrap();
        assert_eq!(r.cpu_cores, 2);
        assert_eq!(r.ram_mib, 4096);
        assert_eq!(r.storage_gib, 100);
        assert_eq!(r.bandwidth_mbps, 0);

        let r: Resources = "8cpu,16gb,1tb,1gbps".parse().unwrap();
        assert_eq!(r.cpu_cores, 8);
        assert_eq!(r.ram_mib, 16384);
        assert_eq!(r.storage_gib, 1024);
        assert_eq!(r.bandwidth_mbps, 1000);
    }

    #[test]
    fn rejects_garbage() {
        assert!("banana".parse::<Resources>().is_err());
        assert!("2xyz".parse::<Resources>().is_err());
    }
}
