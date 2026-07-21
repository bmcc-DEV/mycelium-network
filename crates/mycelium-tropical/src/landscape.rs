//! Paisagem adaptativa — gradiente local + ruído.

use rand::Rng;

pub struct AdaptiveLandscape {
    pub potentials: Vec<f64>,
    pub gradients: Vec<f64>,
    pub noise_scale: f64,
    pub learning_rate: f64,
}

impl AdaptiveLandscape {
    pub fn new(n: usize, noise_scale: f64, learning_rate: f64) -> Self {
        Self {
            potentials: vec![0.0; n],
            gradients: vec![0.0; n],
            noise_scale,
            learning_rate,
        }
    }

    pub fn step(&mut self, neighbors: &[Vec<(usize, f64)>], dt: f64) {
        let mut rng = rand::thread_rng();
        for i in 0..self.potentials.len() {
            let mut grad = 0.0;
            let mut count = 0;
            if let Some(ns) = neighbors.get(i) {
                for &(j, weight) in ns {
                    if j < self.potentials.len() {
                        grad += weight * (self.potentials[j] - self.potentials[i]);
                        count += 1;
                    }
                }
            }
            if count > 0 {
                grad /= count as f64;
            }
            self.gradients[i] = grad;
            let noise: f64 = rng.gen_range(-1.0..1.0) * self.noise_scale;
            self.potentials[i] += self.learning_rate * (grad + noise) * dt;
        }
    }

    pub fn local_peaks(&self, neighbors: &[Vec<(usize, f64)>]) -> Vec<usize> {
        let mut peaks = Vec::new();
        for i in 0..self.potentials.len() {
            let is_peak = neighbors
                .get(i)
                .map(|ns| {
                    ns.iter()
                        .all(|&(j, _)| j >= self.potentials.len() || self.potentials[i] >= self.potentials[j])
                })
                .unwrap_or(true);
            if is_peak {
                peaks.push(i);
            }
        }
        peaks
    }
}
