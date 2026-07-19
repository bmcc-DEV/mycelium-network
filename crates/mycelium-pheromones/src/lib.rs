//! # mycelium-pheromones
//!
//! Reconhecimento Químico: nenhum login, nenhuma conta. Um nó "existe" no
//! Mycelium pelo cheiro que deixa — uma identidade ed25519 que assina um
//! `trail` de contribuições. Feromônios evaporam (`decay`) se o nó fica
//! inativo, e sinais de `alarm` se espalham em onda quando um nó malicioso
//! é detectado.

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use mycelium_core::NodeId;
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Erros do sistema de feromônios.
#[derive(Debug, thiserror::Error)]
pub enum PheromoneError {
    #[error("assinatura inválida: o cheiro não bate com a glândula")]
    BadSignature,
    #[error("feromônio evaporou (decay expirado)")]
    Evaporated,
    #[error("falha de serialização: {0}")]
    Codec(#[from] serde_json::Error),
}

/// A "glândula" do nó: a chave privada que secreta feromônios.
pub struct Gland {
    signing_key: SigningKey,
}

impl Gland {
    /// Gera uma nova identidade (um nó recém-germinado).
    pub fn germinate() -> Self {
        Self {
            signing_key: SigningKey::generate(&mut rand::rngs::OsRng),
        }
    }

    /// Restaura uma glândula a partir de sementes salvas.
    pub fn from_seed(seed: [u8; 32]) -> Self {
        Self {
            signing_key: SigningKey::from_bytes(&seed),
        }
    }

    /// Bytes secretos para persistência local.
    pub fn seed(&self) -> [u8; 32] {
        self.signing_key.to_bytes()
    }

    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    /// Identidade pública derivada da chave.
    pub fn node_id(&self) -> NodeId {
        NodeId::derive(self.verifying_key().as_bytes())
    }

    /// Secreta um pacote de feromônio assinado.
    pub fn secrete(&self, trail: Trail, ttl: Duration) -> Result<Pheromone, PheromoneError> {
        let body = PheromoneBody {
            identity: self.verifying_key().to_bytes(),
            scent: trail.scent(),
            trail,
            emitted_at_secs: now_secs(),
            decay_secs: ttl.as_secs(),
            alarm: None,
        };
        let payload = serde_json::to_vec(&body)?;
        let signature = self.signing_key.sign(&payload);
        Ok(Pheromone {
            body,
            signature: signature.to_bytes().to_vec(),
        })
    }
}

/// Uma contribuição registrada no trail.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Contribution {
    CpuCycles { vectors_executed: u64 },
    Ram { chamber_hours: u64 },
    Storage { gib_hours: u64 },
    Bandwidth { gib_relayed: u64 },
    Uptime { hours: u64 },
}

impl Contribution {
    /// Peso da contribuição no scent (heurística simples do protótipo).
    fn weight(&self) -> f64 {
        match self {
            Contribution::CpuCycles { vectors_executed } => *vectors_executed as f64 * 0.02,
            Contribution::Ram { chamber_hours } => *chamber_hours as f64 * 0.01,
            Contribution::Storage { gib_hours } => *gib_hours as f64 * 0.005,
            Contribution::Bandwidth { gib_relayed } => *gib_relayed as f64 * 0.01,
            Contribution::Uptime { hours } => *hours as f64 * 0.001,
        }
    }
}

/// Histórico de contribuições do nó. No manifesto o `scent` é um zk-proof
/// da reputação acumulada; neste protótipo é um score determinístico
/// derivado do trail (o zk-proof é um TODO documentado).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Trail {
    pub contributions: Vec<Contribution>,
}

impl Trail {
    pub fn record(&mut self, c: Contribution) {
        self.contributions.push(c);
    }

    /// Score de reputação em [0, 1). Novatos cheiram a ~0.42 por cortesia
    /// do manifesto; o cheiro melhora com contribuições.
    pub fn scent(&self) -> f64 {
        let raw: f64 = self.contributions.iter().map(Contribution::weight).sum();
        let s = 0.42 + raw / (raw + 10.0) * 0.57;
        (s * 100.0).round() / 100.0
    }
}

/// Sinal de perigo: espalha-se em onda pelo gossip.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Alarm {
    /// Nó malicioso detectado.
    pub suspect: NodeId,
    /// Motivo legível.
    pub reason: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PheromoneBody {
    /// Chave pública ed25519 do emissor.
    pub identity: [u8; 32],
    /// Reputação acumulada (protótipo: score; futuro: zk-proof).
    pub scent: f64,
    /// Histórico de contribuições.
    pub trail: Trail,
    /// Momento da emissão (segundos UNIX).
    pub emitted_at_secs: u64,
    /// TTL — o feromônio evapora depois disso.
    pub decay_secs: u64,
    /// Sinal de perigo opcional.
    pub alarm: Option<Alarm>,
}

/// Pacote de feromônio assinado, pronto para ser "cheirado" por vizinhos.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pheromone {
    pub body: PheromoneBody,
    pub signature: Vec<u8>,
}

impl Pheromone {
    pub fn node_id(&self) -> NodeId {
        NodeId::derive(&self.body.identity)
    }

    /// "Cheira" o feromônio: verifica assinatura e evaporação.
    pub fn sniff(&self) -> Result<f64, PheromoneError> {
        let key = VerifyingKey::from_bytes(&self.body.identity)
            .map_err(|_| PheromoneError::BadSignature)?;
        let payload = serde_json::to_vec(&self.body)?;
        let sig_bytes: [u8; 64] = self
            .signature
            .as_slice()
            .try_into()
            .map_err(|_| PheromoneError::BadSignature)?;
        key.verify(&payload, &Signature::from_bytes(&sig_bytes))
            .map_err(|_| PheromoneError::BadSignature)?;

        let expires = self.body.emitted_at_secs + self.body.decay_secs;
        if now_secs() > expires {
            return Err(PheromoneError::Evaporated);
        }
        Ok(self.body.scent)
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn novice_smells_like_042() {
        assert_eq!(Trail::default().scent(), 0.42);
    }

    #[test]
    fn scent_improves_with_contributions() {
        let mut trail = Trail::default();
        trail.record(Contribution::CpuCycles {
            vectors_executed: 10_000,
        });
        assert!(trail.scent() > 0.42);
        assert!(trail.scent() < 1.0);
    }

    #[test]
    fn secreted_pheromone_can_be_sniffed() {
        let gland = Gland::germinate();
        let p = gland
            .secrete(Trail::default(), Duration::from_secs(3600))
            .unwrap();
        assert_eq!(p.sniff().unwrap(), 0.42);
        assert_eq!(p.node_id(), gland.node_id());
    }

    #[test]
    fn tampered_pheromone_smells_wrong() {
        let gland = Gland::germinate();
        let mut p = gland
            .secrete(Trail::default(), Duration::from_secs(3600))
            .unwrap();
        p.body.scent = 0.99;
        assert!(matches!(p.sniff(), Err(PheromoneError::BadSignature)));
    }

    #[test]
    fn evaporated_pheromone_is_rejected() {
        let gland = Gland::germinate();
        let mut p = gland
            .secrete(Trail::default(), Duration::from_secs(0))
            .unwrap();
        p.body.emitted_at_secs = 0;
        // assinatura fica inválida ao mexer no corpo; re-assina para testar decay
        let payload = serde_json::to_vec(&p.body).unwrap();
        let sig = gland.signing_key_for_tests().sign(&payload);
        p.signature = sig.to_bytes().to_vec();
        assert!(matches!(p.sniff(), Err(PheromoneError::Evaporated)));
    }
}

#[cfg(test)]
impl Gland {
    fn signing_key_for_tests(&self) -> &SigningKey {
        &self.signing_key
    }
}
