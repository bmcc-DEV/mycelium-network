//! Métrica de Hilbert e projectores.

use crate::Tropical;

/// `d_H(x,y) = max(x−y) − min(x−y)`.
pub fn hilbert_distance(x: &[Tropical], y: &[Tropical]) -> f64 {
    assert_eq!(x.len(), y.len());
    let diffs: Vec<f64> = x
        .iter()
        .zip(y.iter())
        .filter(|(a, b)| !a.is_zero() && !b.is_zero())
        .map(|(a, b)| a.0 - b.0)
        .collect();
    if diffs.is_empty() {
        return f64::INFINITY;
    }
    let max_diff = diffs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let min_diff = diffs.iter().copied().fold(f64::INFINITY, f64::min);
    max_diff - min_diff
}

pub fn project_onto_submodule(x: &[Tropical], basis: &[Vec<Tropical>]) -> Vec<Tropical> {
    let n = x.len();
    let mut result = vec![Tropical::ZERO; n];
    for v in basis {
        let mut alpha = Tropical(f64::INFINITY);
        for i in 0..n {
            if !v[i].is_zero() {
                let a = Tropical(x[i].0 - v[i].0);
                alpha = Tropical(alpha.0.min(a.0));
            }
        }
        for i in 0..n {
            result[i] = result[i].oplus(alpha.otimes(v[i]));
        }
    }
    result
}

pub fn cyclic_projector(
    x0: &[Tropical],
    bases: &[Vec<Vec<Tropical>>],
    max_iter: usize,
    epsilon: f64,
) -> (Vec<Tropical>, f64, usize) {
    let mut x = x0.to_vec();
    let mut prev = x.clone();
    for iter in 0..max_iter {
        for basis in bases {
            x = project_onto_submodule(&x, basis);
        }
        let dist = hilbert_distance(&x, &prev);
        if dist < epsilon {
            let lambda = x
                .iter()
                .zip(prev.iter())
                .map(|(a, b)| a.0 - b.0)
                .sum::<f64>()
                / x.len() as f64;
            return (x, lambda, iter + 1);
        }
        prev = x.clone();
    }
    (x, 0.0, max_iter)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hilbert_zero_on_equal() {
        let x = vec![Tropical(1.0), Tropical(2.0)];
        assert!((hilbert_distance(&x, &x) - 0.0).abs() < 1e-12);
    }
}
