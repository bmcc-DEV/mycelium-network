//! QEL como protecção topológica (sólitons) — port conceptual ET-COSMIC / tese Mycelium.

use crate::QelShard;

/// Carga topológica de um conjunto de shards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TopologicalCharge {
    /// Nenhum shard.
    Vacuum,
    /// Insuficiente para reconstruir.
    Fragment { present: usize, needed: usize },
    /// Suficiente (estável por topologia k-of-n).
    Soliton { charge: usize, stability: usize },
}

/// Carga = relação present vs threshold.
pub fn topological_charge(shards: &[QelShard]) -> TopologicalCharge {
    if shards.is_empty() {
        return TopologicalCharge::Vacuum;
    }
    let threshold = shards[0].threshold as usize;
    let present = shards.len();
    if present >= threshold {
        TopologicalCharge::Soliton {
            charge: threshold,
            stability: present - threshold + 1,
        }
    } else {
        TopologicalCharge::Fragment {
            present,
            needed: threshold - present,
        }
    }
}

/// Invariante: mesmo nonce, content_id, total, threshold.
pub fn verify_topological_invariant(shards: &[QelShard]) -> bool {
    if shards.is_empty() {
        return true;
    }
    let nonce = shards[0].nonce;
    let cid = &shards[0].content_id;
    let total = shards[0].total;
    let threshold = shards[0].threshold;
    shards.iter().all(|s| {
        s.nonce == nonce
            && s.content_id == *cid
            && s.total == total
            && s.threshold == threshold
    })
}

/// Remove shards cujo nonce aparece também em `shards_b` (aniquilação).
pub fn annihilate(shards_a: &[QelShard], shards_b: &[QelShard]) -> Vec<QelShard> {
    let nonces_b: std::collections::HashSet<[u8; 16]> =
        shards_b.iter().map(|s| s.nonce).collect();
    shards_a
        .iter()
        .filter(|s| !nonces_b.contains(&s.nonce))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{fragment, QelConfig};

    #[test]
    fn charge_soliton_when_k_met() {
        let shards = fragment(
            b"topo",
            "Qmtopo",
            &QelConfig {
                threshold: 2,
                total: 3,
                ttl_secs: 60,
            },
        )
        .unwrap();
        let subset = &shards[..2];
        assert!(matches!(
            topological_charge(subset),
            TopologicalCharge::Soliton { .. }
        ));
        assert!(verify_topological_invariant(subset));
    }
}
