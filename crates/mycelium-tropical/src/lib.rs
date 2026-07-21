//! # mycelium-tropical
//!
//! Álgebra Max-Plus (semiring idempotente) como motor de decisão distribuída.
//!
//! - ⊕ = max, ⊗ = +, zero = −∞, unidade = 0
//! - Bellman: `(Bf)ᵢ = supⱼ(bᵢⱼ + fⱼ)`
//! - Physarum: fluxo adaptativo e poda
//! - Godunov/CFL + detecção de reflex instability
//!
//! Portável a partir da tese unificada Mycelium; independe do daemon.

mod bellman;
mod godunov;
mod hilbert;
mod landscape;
mod physarum;

pub use bellman::{weights_from_network, BellmanOperator};
pub use godunov::{
    cfl_timestep, detect_reflex_instability, rsa_step, CflConfig, NodeState, ReflexStatus,
};
pub use hilbert::{cyclic_projector, hilbert_distance, project_onto_submodule};
pub use landscape::AdaptiveLandscape;
pub use physarum::{HyphaState, MyceliumPhase, PhysarumNetwork};

/// Elemento do semiring max-plus ℝ ∪ {−∞}.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tropical(pub f64);

impl Tropical {
    pub const ZERO: Tropical = Tropical(f64::NEG_INFINITY);
    pub const ONE: Tropical = Tropical(0.0);

    pub fn oplus(self, other: Self) -> Self {
        Tropical(self.0.max(other.0))
    }

    pub fn otimes(self, other: Self) -> Self {
        if self.is_zero() || other.is_zero() {
            return Tropical::ZERO;
        }
        Tropical(self.0 + other.0)
    }

    pub fn leq(self, other: Self) -> bool {
        self.0 <= other.0
    }

    pub fn is_zero(self) -> bool {
        self.0.is_infinite() && self.0.is_sign_negative()
    }

    pub fn value(self) -> f64 {
        self.0
    }
}

/// Matriz max-plus (row-major).
#[derive(Clone, Debug)]
pub struct TropicalMatrix {
    pub rows: usize,
    pub cols: usize,
    pub data: Vec<Tropical>,
}

impl TropicalMatrix {
    pub fn new(rows: usize, cols: usize) -> Self {
        Self {
            rows,
            cols,
            data: vec![Tropical::ZERO; rows * cols],
        }
    }

    pub fn identity(n: usize) -> Self {
        let mut m = Self::new(n, n);
        for i in 0..n {
            m.set(i, i, Tropical::ONE);
        }
        m
    }

    pub fn get(&self, i: usize, j: usize) -> Tropical {
        self.data[i * self.cols + j]
    }

    pub fn set(&mut self, i: usize, j: usize, val: Tropical) {
        self.data[i * self.cols + j] = val;
    }

    /// `(A ⊗ B)ᵢₖ = ⊕ⱼ (Aᵢⱼ ⊗ Bⱼₖ)`
    pub fn otimes(&self, other: &Self) -> Self {
        assert_eq!(self.cols, other.rows);
        let mut result = Self::new(self.rows, other.cols);
        for i in 0..self.rows {
            for k in 0..other.cols {
                let mut acc = Tropical::ZERO;
                for j in 0..self.cols {
                    acc = acc.oplus(self.get(i, j).otimes(other.get(j, k)));
                }
                result.set(i, k, acc);
            }
        }
        result
    }

    /// `(A ⊗ x)ᵢ = ⊕ⱼ (Aᵢⱼ ⊗ xⱼ)`
    pub fn apply(&self, x: &[Tropical]) -> Vec<Tropical> {
        assert_eq!(self.cols, x.len());
        (0..self.rows)
            .map(|i| {
                let mut acc = Tropical::ZERO;
                for j in 0..self.cols {
                    acc = acc.oplus(self.get(i, j).otimes(x[j]));
                }
                acc
            })
            .collect()
    }

    /// Estrela de Kleene truncada: `A* ≈ I ⊕ A ⊕ … ⊕ A^max_iter`.
    pub fn kleene_star(&self, max_iter: usize) -> Self {
        assert_eq!(self.rows, self.cols);
        let n = self.rows;
        let mut result = Self::identity(n);
        let mut power = self.clone();
        for _ in 0..max_iter {
            for i in 0..n {
                for j in 0..n {
                    let val = result.get(i, j).oplus(power.get(i, j));
                    result.set(i, j, val);
                }
            }
            power = power.otimes(self);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oplus_otimes_basics() {
        assert_eq!(Tropical(1.0).oplus(Tropical(2.0)), Tropical(2.0));
        assert_eq!(Tropical(1.0).otimes(Tropical(2.0)), Tropical(3.0));
        assert!(Tropical::ZERO.otimes(Tropical(5.0)).is_zero());
    }

    #[test]
    fn matrix_apply_and_kleene() {
        let mut a = TropicalMatrix::new(2, 2);
        a.set(0, 0, Tropical::ONE);
        a.set(0, 1, Tropical(-1.0));
        a.set(1, 0, Tropical(-2.0));
        a.set(1, 1, Tropical::ONE);
        let x = vec![Tropical(0.0), Tropical(0.0)];
        let y = a.apply(&x);
        assert_eq!(y[0], Tropical(0.0));
        let star = a.kleene_star(4);
        assert!(!star.get(0, 0).is_zero());
    }
}
