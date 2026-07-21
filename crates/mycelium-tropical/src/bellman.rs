//! Operador de Bellman — decisão distribuída max-plus.

use crate::{Tropical, TropicalMatrix};

/// `(Bf)ᵢ = ⊕ⱼ (bᵢⱼ ⊗ fⱼ)`.
pub struct BellmanOperator {
    pub weights: TropicalMatrix,
    pub n: usize,
}

impl BellmanOperator {
    pub fn new(weights: TropicalMatrix) -> Self {
        let n = weights.rows;
        Self { weights, n }
    }

    pub fn apply(&self, f: &[Tropical]) -> Vec<Tropical> {
        self.weights.apply(f)
    }

    /// Itera até ponto fixo (deslocamento uniforme ≈ autovalor).
    pub fn iterate_to_fixed_point(
        &self,
        f0: &[Tropical],
        max_iter: usize,
        epsilon: f64,
    ) -> (Vec<Tropical>, f64, usize) {
        let mut f = f0.to_vec();
        let mut prev = f.clone();

        for iter in 0..max_iter {
            f = self.apply(&f);
            let max_diff = f
                .iter()
                .zip(prev.iter())
                .map(|(a, b)| (a.0 - b.0).abs())
                .fold(0.0_f64, f64::max);

            if max_diff < epsilon {
                let lambda = f
                    .iter()
                    .zip(prev.iter())
                    .map(|(a, b)| a.0 - b.0)
                    .sum::<f64>()
                    / self.n as f64;
                return (f, lambda, iter + 1);
            }
            prev = f.clone();
        }
        (f, 0.0, max_iter)
    }

    /// Arestas críticas: `bᵢⱼ + fⱼ ≈ fᵢ`.
    pub fn critical_graph(&self, f_star: &[Tropical]) -> Vec<usize> {
        let mut critical = Vec::new();
        for i in 0..self.n {
            for j in 0..self.n {
                let bij = self.weights.get(i, j);
                if bij.is_zero() {
                    continue;
                }
                let lhs = bij.otimes(f_star[j]);
                if (lhs.0 - f_star[i].0).abs() < 1e-9 {
                    if !critical.contains(&i) {
                        critical.push(i);
                    }
                    if !critical.contains(&j) {
                        critical.push(j);
                    }
                }
            }
        }
        critical
    }
}

/// `bᵢⱼ = −lat + ln(bw) + ln(trust)`.
pub fn weights_from_network(
    n: usize,
    latencies_ms: &[Vec<f64>],
    bandwidths_mbps: &[Vec<f64>],
    trust: &[Vec<f64>],
) -> TropicalMatrix {
    let mut w = TropicalMatrix::new(n, n);
    for i in 0..n {
        for j in 0..n {
            if i == j {
                w.set(i, j, Tropical::ONE);
                continue;
            }
            let lat = latencies_ms[i][j];
            let bw = bandwidths_mbps[i][j];
            let tr = trust[i][j];
            if lat <= 0.0 || bw <= 0.0 || tr <= 0.0 {
                w.set(i, j, Tropical::ZERO);
            } else {
                w.set(i, j, Tropical(-lat + bw.ln() + tr.ln()));
            }
        }
    }
    w
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bellman_prefers_strong_edge() {
        let mut w = TropicalMatrix::new(2, 2);
        w.set(0, 0, Tropical::ONE);
        w.set(0, 1, Tropical(5.0));
        w.set(1, 0, Tropical(-10.0));
        w.set(1, 1, Tropical::ONE);
        let op = BellmanOperator::new(w);
        let f = vec![Tropical(0.0), Tropical(0.0)];
        let out = op.apply(&f);
        assert_eq!(out[0], Tropical(5.0));
    }
}
