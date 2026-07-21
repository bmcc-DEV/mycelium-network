//! GhostID como descoerência — espelho conceptual de ET-COSMIC `destroyGhostId`.

use crate::GhostId;

/// Estado de descoerência de um GhostID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecoherenceState {
    /// Keypair gerado, ainda não observado por relays.
    Measured,
    /// Pubkey vista por N relays (banho térmico).
    Decohered { relay_count: usize },
    /// TTL expirado ou destruído explicitamente.
    Collapsed,
}

impl GhostId {
    /// Estado actual (TTL + observação de relays).
    pub fn decoherence_state(&self, relay_count: usize) -> DecoherenceState {
        if self.is_expired() {
            DecoherenceState::Collapsed
        } else if relay_count > 0 {
            DecoherenceState::Decohered { relay_count }
        } else {
            DecoherenceState::Measured
        }
    }

    /// Incerteza ∝ 1/N (mais relays → identidade mais “clássica”).
    pub fn identity_uncertainty(&self, relay_count: usize) -> f64 {
        if relay_count == 0 {
            1.0
        } else {
            1.0 / relay_count as f64
        }
    }

    /// Segundos até colapso por TTL.
    pub fn decoherence_time_remaining(&self) -> f64 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let elapsed = now.saturating_sub(self.created_at_secs());
        self.ttl_secs().saturating_sub(elapsed) as f64
    }

    /// Destruição explícita (como `destroyGhostId` no ET-COSMIC).
    /// Consome `self`; o `Drop` faz zeroize do material.
    pub fn decohere(self) {
        drop(self);
    }
}
