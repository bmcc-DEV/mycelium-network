//! Dinâmica Physarum — fluxo adaptativo e atrofia.

#[derive(Clone, Debug)]
pub struct HyphaState {
    pub conductance: f64,
    pub flow: f64,
    pub length: f64,
    pub active: bool,
    pub age_secs: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MyceliumPhase {
    Exploratory,
    Transport,
    Dormant,
}

pub struct PhysarumNetwork {
    pub n: usize,
    pub hyphae: Vec<Vec<HyphaState>>,
    pub potentials: Vec<f64>,
    pub alpha: f64,
    pub prune_threshold: f64,
}

impl PhysarumNetwork {
    pub fn new(n: usize, alpha: f64, prune_threshold: f64) -> Self {
        let hyphae = (0..n)
            .map(|i| {
                (0..n)
                    .map(|j| {
                        if i == j {
                            HyphaState {
                                conductance: 0.0,
                                flow: 0.0,
                                length: 0.0,
                                active: false,
                                age_secs: 0.0,
                            }
                        } else {
                            HyphaState {
                                conductance: 1.0,
                                flow: 0.0,
                                length: 1.0,
                                active: true,
                                age_secs: 0.0,
                            }
                        }
                    })
                    .collect()
            })
            .collect();
        Self {
            n,
            hyphae,
            potentials: vec![0.0; n],
            alpha,
            prune_threshold,
        }
    }

    /// Um passo: fluxo → adaptação de D → poda.
    pub fn step(&mut self, dt: f64) {
        for i in 0..self.n {
            for j in (i + 1)..self.n {
                if !self.hyphae[i][j].active {
                    continue;
                }
                let dp = self.potentials[i] - self.potentials[j];
                let d = self.hyphae[i][j].conductance;
                let l = self.hyphae[i][j].length.max(1e-9);
                let q = d * dp / l;
                self.hyphae[i][j].flow = q;
                self.hyphae[j][i].flow = -q;
            }
        }

        for i in 0..self.n {
            for j in (i + 1)..self.n {
                if !self.hyphae[i][j].active {
                    continue;
                }
                let q_abs = self.hyphae[i][j].flow.abs();
                let growth = q_abs / (1.0 + self.alpha * q_abs);
                let decay = self.hyphae[i][j].conductance;
                let dd = (growth - decay) * dt;
                let new_d = (self.hyphae[i][j].conductance + dd).max(0.0);
                self.hyphae[i][j].conductance = new_d;
                self.hyphae[j][i].conductance = new_d;
                self.hyphae[i][j].age_secs += dt;
                self.hyphae[j][i].age_secs += dt;
            }
        }

        for i in 0..self.n {
            for j in (i + 1)..self.n {
                if self.hyphae[i][j].conductance < self.prune_threshold {
                    self.hyphae[i][j].active = false;
                    self.hyphae[j][i].active = false;
                }
            }
        }
    }

    pub fn anastomose(&mut self, i: usize, j: usize, k: usize) {
        if self.hyphae[i][j].active && self.hyphae[i][k].active && !self.hyphae[j][k].active {
            let new_d = self.hyphae[i][j]
                .conductance
                .max(self.hyphae[i][k].conductance);
            let state = HyphaState {
                conductance: new_d,
                flow: 0.0,
                length: (self.hyphae[i][j].length + self.hyphae[i][k].length) / 2.0,
                active: true,
                age_secs: 0.0,
            };
            self.hyphae[j][k] = state.clone();
            self.hyphae[k][j] = state;
        }
    }

    pub fn best_route(&self, from: usize, to: usize) -> Option<Vec<usize>> {
        let mut dist = vec![f64::NEG_INFINITY; self.n];
        let mut prev = vec![usize::MAX; self.n];
        let mut visited = vec![false; self.n];
        dist[from] = 0.0;

        for _ in 0..self.n {
            let mut u = usize::MAX;
            let mut best = f64::NEG_INFINITY;
            for i in 0..self.n {
                if !visited[i] && dist[i] > best {
                    best = dist[i];
                    u = i;
                }
            }
            if u == usize::MAX {
                break;
            }
            visited[u] = true;
            for v in 0..self.n {
                if !self.hyphae[u][v].active || visited[v] {
                    continue;
                }
                let w = self.hyphae[u][v].conductance / self.hyphae[u][v].length.max(1e-9);
                let new_dist = dist[u] + w;
                if new_dist > dist[v] {
                    dist[v] = new_dist;
                    prev[v] = u;
                }
            }
        }

        if dist[to] == f64::NEG_INFINITY {
            return None;
        }
        let mut path = vec![to];
        let mut cur = to;
        while cur != from {
            cur = prev[cur];
            if cur == usize::MAX {
                return None;
            }
            path.push(cur);
        }
        path.reverse();
        Some(path)
    }

    pub fn phase(&self) -> MyceliumPhase {
        let active_count: usize = (0..self.n)
            .flat_map(|i| ((i + 1)..self.n).map(move |j| (i, j)))
            .filter(|&(i, j)| self.hyphae[i][j].active)
            .count();
        let total_possible = self.n * (self.n - 1) / 2;
        let density = active_count as f64 / total_possible.max(1) as f64;
        let avg_flow: f64 = (0..self.n)
            .flat_map(|i| ((i + 1)..self.n).map(move |j| (i, j)))
            .filter(|&(i, j)| self.hyphae[i][j].active)
            .map(|(i, j)| self.hyphae[i][j].flow.abs())
            .sum::<f64>()
            / active_count.max(1) as f64;

        if density < 0.2 {
            MyceliumPhase::Exploratory
        } else if avg_flow > 0.01 {
            MyceliumPhase::Transport
        } else {
            MyceliumPhase::Dormant
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_exists_on_complete_graph() {
        let net = PhysarumNetwork::new(4, 0.1, 0.001);
        let path = net.best_route(0, 3).expect("path");
        assert_eq!(path.first(), Some(&0));
        assert_eq!(path.last(), Some(&3));
    }
}
