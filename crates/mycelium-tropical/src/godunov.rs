//! Godunov/RSA helpers + CFL + reflex instability.

use crate::Tropical;

#[derive(Clone, Debug)]
pub struct NodeState {
    pub potential: Tropical,
    pub conductances: Vec<(usize, f64)>,
    pub flows: Vec<(usize, f64)>,
    pub last_update_ms: u64,
}

#[derive(Clone, Debug)]
pub struct CflConfig {
    pub sigma: f64,
    pub min_dt_ms: f64,
    pub max_dt_ms: f64,
}

impl Default for CflConfig {
    fn default() -> Self {
        Self {
            sigma: 0.4,
            min_dt_ms: 100.0,
            max_dt_ms: 5000.0,
        }
    }
}

pub fn cfl_timestep(
    config: &CflConfig,
    min_latency_ms: f64,
    max_processing_speed: f64,
    gossip_speed: f64,
) -> f64 {
    let denom = (max_processing_speed + gossip_speed).max(1e-9);
    let dt = config.sigma * min_latency_ms / denom;
    dt.clamp(config.min_dt_ms, config.max_dt_ms)
}

/// Reconstruct–Solve–Average (Bellman + amortecimento).
pub fn rsa_step(
    state: &mut NodeState,
    neighbor_states: &[(usize, Tropical)],
    weights: &[f64],
    dt: f64,
) {
    let mut new_potential = Tropical::ZERO;
    for (idx, (_peer, peer_pot)) in neighbor_states.iter().enumerate() {
        let w = weights.get(idx).copied().unwrap_or(0.0);
        let candidate = Tropical(w).otimes(*peer_pot);
        new_potential = new_potential.oplus(candidate);
    }
    let w = 0.7;
    let w_prime = 0.3;
    state.potential = Tropical(w * new_potential.0 + w_prime * state.potential.0);

    if !state.flows.is_empty() {
        let total: f64 = state.flows.iter().map(|(_, q)| *q).sum();
        if total.abs() > 1e-6 {
            let correction = total / state.flows.len() as f64;
            for (_, q) in &mut state.flows {
                *q -= correction;
            }
        }
    }
    state.last_update_ms = state.last_update_ms.saturating_add((dt * 1000.0) as u64);
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReflexStatus {
    Stable,
    Warning { correlation: f64 },
    Unstable { correlation: f64, growth_rate: f64 },
    InsufficientData,
}

pub fn detect_reflex_instability(
    latency_history: &[f64],
    message_count_history: &[f64],
) -> ReflexStatus {
    if latency_history.len() < 10 || message_count_history.len() < 10 {
        return ReflexStatus::InsufficientData;
    }
    let n = latency_history.len().min(message_count_history.len());
    let lat = &latency_history[latency_history.len() - n..];
    let msg = &message_count_history[message_count_history.len() - n..];
    let correlation = pearson(lat, msg);
    let half = n / 2;
    let corr_first = pearson(&lat[..half], &msg[..half]);
    let corr_second = pearson(&lat[half..], &msg[half..]);

    if correlation > 0.7 && corr_second > corr_first + 0.1 {
        ReflexStatus::Unstable {
            correlation,
            growth_rate: corr_second - corr_first,
        }
    } else if correlation > 0.5 {
        ReflexStatus::Warning { correlation }
    } else {
        ReflexStatus::Stable
    }
}

fn pearson(x: &[f64], y: &[f64]) -> f64 {
    let n = x.len().min(y.len());
    if n == 0 {
        return 0.0;
    }
    let mx: f64 = x[..n].iter().sum::<f64>() / n as f64;
    let my: f64 = y[..n].iter().sum::<f64>() / n as f64;
    let mut cov = 0.0;
    let mut vx = 0.0;
    let mut vy = 0.0;
    for i in 0..n {
        let dx = x[i] - mx;
        let dy = y[i] - my;
        cov += dx * dy;
        vx += dx * dx;
        vy += dy * dy;
    }
    cov / (vx.sqrt() * vy.sqrt() + 1e-10)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cfl_clamped() {
        let cfg = CflConfig::default();
        let dt = cfl_timestep(&cfg, 1.0, 1000.0, 1000.0);
        assert!((dt - cfg.min_dt_ms).abs() < 1e-9);
    }
}
