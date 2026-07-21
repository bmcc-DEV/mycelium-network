//! # mycelium-distancebridge
//!
//! Fase 0: seleção de transportes por potencial (paisagem adaptativa).
//! Port conceptual de ET-COSMIC `distanceBridge.ts` + `qrcMotor.ts`.
//! Sem BLE/LoRa/SMS físicos neste sprint.

use mycelium_qel::TransportHint;
use serde::{Deserialize, Serialize};

/// Contexto ambiental (o que a folha “sente”).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TransportContext {
    pub has_internet: bool,
    pub ipfs_peers: usize,
    pub relay_available: bool,
    pub lora_available: bool,
    pub gsm_available: bool,
    pub proximity_peers: usize,
}

/// Potencial estimado de entrega ∈ [0, 1].
pub fn transport_potential(transport: &TransportHint, ctx: &TransportContext) -> f64 {
    match transport {
        TransportHint::Nostr => {
            if ctx.has_internet {
                0.95
            } else {
                0.0
            }
        }
        TransportHint::Ipfs => {
            if ctx.has_internet && ctx.ipfs_peers > 0 {
                0.8
            } else if ctx.has_internet {
                0.1
            } else {
                0.0
            }
        }
        TransportHint::RelayMesh => {
            if ctx.relay_available {
                0.9
            } else {
                0.0
            }
        }
        TransportHint::LoRa => {
            if ctx.lora_available {
                0.6
            } else {
                0.0
            }
        }
        TransportHint::Sms => {
            if ctx.gsm_available {
                0.7
            } else {
                0.0
            }
        }
        TransportHint::Proximity => {
            if ctx.proximity_peers > 0 {
                0.5
            } else {
                0.0
            }
        }
        TransportHint::Dtn => 0.3,
        TransportHint::Visual => 0.2,
        TransportHint::Any => 0.1,
    }
}

/// Top-N transportes por potencial (gradiente ascent local).
pub fn select_transports(ctx: &TransportContext, n: usize) -> Vec<(TransportHint, f64)> {
    let all = [
        TransportHint::Nostr,
        TransportHint::Ipfs,
        TransportHint::RelayMesh,
        TransportHint::LoRa,
        TransportHint::Sms,
        TransportHint::Proximity,
        TransportHint::Dtn,
        TransportHint::Visual,
    ];
    let mut scored: Vec<(TransportHint, f64)> = all
        .into_iter()
        .map(|t| {
            let p = transport_potential(&t, ctx);
            (t, p)
        })
        .filter(|(_, p)| *p > 0.0)
        .collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(n);
    scored
}

/// Ordem de fallback estilo DistanceBridge (preferred + restantes).
pub fn fallback_order(preferred: TransportHint, ctx: &TransportContext) -> Vec<TransportHint> {
    let mut ranked = select_transports(ctx, 8);
    ranked.sort_by(|a, b| {
        if a.0 == preferred {
            return std::cmp::Ordering::Less;
        }
        if b.0 == preferred {
            return std::cmp::Ordering::Greater;
        }
        b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut out: Vec<TransportHint> = ranked.into_iter().map(|(t, _)| t).collect();
    if !out.contains(&preferred) {
        out.insert(0, preferred);
    }
    out
}

/// Modo “Anderson cage”: só canais de malha/curto alcance (sem outbound frágil).
pub fn anderson_cage_channels() -> Vec<TransportHint> {
    vec![
        TransportHint::RelayMesh,
        TransportHint::Proximity,
        TransportHint::Dtn,
        TransportHint::Visual,
    ]
}

/// Hints QEL hybrid a partir da paisagem: top mailbox → primeiros `k` shards;
/// melhor store (Ipfs) → restantes.
pub fn hybrid_hints_from_landscape(
    ctx: &TransportContext,
    threshold: u8,
    total: u8,
) -> Vec<TransportHint> {
    let ranked = select_transports(ctx, 8);
    let mailbox = ranked
        .iter()
        .find(|(t, _)| matches!(t, TransportHint::Nostr | TransportHint::RelayMesh | TransportHint::Sms))
        .map(|(t, _)| t.clone())
        .unwrap_or(TransportHint::Nostr);
    let store = ranked
        .iter()
        .find(|(t, _)| matches!(t, TransportHint::Ipfs | TransportHint::Dtn | TransportHint::Visual))
        .map(|(t, _)| t.clone())
        .unwrap_or(TransportHint::Ipfs);

    (0..total as usize)
        .map(|i| {
            if (i as u8) < threshold {
                mailbox.clone()
            } else {
                store.clone()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn internet_prefers_nostr() {
        let ctx = TransportContext {
            has_internet: true,
            ipfs_peers: 0,
            relay_available: false,
            ..Default::default()
        };
        let top = select_transports(&ctx, 1);
        assert_eq!(top[0].0, TransportHint::Nostr);
    }

    #[test]
    fn fallback_puts_preferred_first() {
        let ctx = TransportContext {
            has_internet: true,
            relay_available: true,
            ..Default::default()
        };
        let order = fallback_order(TransportHint::RelayMesh, &ctx);
        assert_eq!(order[0], TransportHint::RelayMesh);
    }

    #[test]
    fn hybrid_hints_split_mailbox_store() {
        let ctx = TransportContext {
            has_internet: true,
            ipfs_peers: 1,
            ..Default::default()
        };
        let hints = hybrid_hints_from_landscape(&ctx, 3, 7);
        assert_eq!(hints.len(), 7);
        assert!(hints[..3].iter().all(|h| *h == TransportHint::Nostr));
        assert!(hints[3..].iter().all(|h| *h == TransportHint::Ipfs));
    }
}
