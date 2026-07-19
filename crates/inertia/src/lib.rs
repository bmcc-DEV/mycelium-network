//! # Inertia — CI/CD que viaja pela rede
//!
//! Um **Vector** é uma unidade de trabalho (build, teste, deploy) que
//! viaja pelas hifas até um nó com CPU ociosa, executa, e devolve o
//! momentum (resultado) ao emissor. Quem executa Vectors ganha ATP.
//!
//! Stub coeso: o "executor" local simula execução; despacho remoto real
//! fica para a próxima fase.

use mycelium_core::{ContentId, NodeId};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Debug, thiserror::Error)]
pub enum InertiaError {
    #[error("nenhum vector na fila de momentum")]
    QueueEmpty,
}

/// Fase do pipeline que o Vector carrega.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Thrust {
    Build,
    Test,
    Deploy { target_ion: String },
}

/// Unidade de trabalho que viaja pela rede.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Vector {
    /// Plot do Giggs que este Vector processa.
    pub plot: ContentId,
    pub thrust: Thrust,
    /// Nó que emitiu o Vector (para devolver o momentum).
    pub emitter: NodeId,
}

/// Resultado da execução de um Vector.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Momentum {
    pub success: bool,
    pub log: String,
    /// ATP ganho pelo executor.
    pub atp_earned: u64,
}

/// Fila local de Vectors aguardando um nó com CPU.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Flywheel {
    queue: VecDeque<Vector>,
}

impl Flywheel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Injeta um Vector na fila (vindo de um Signal do TheField).
    pub fn inject(&mut self, vector: Vector) {
        self.queue.push_back(vector);
    }

    pub fn pending(&self) -> usize {
        self.queue.len()
    }

    /// Executa o próximo Vector localmente (simulação do protótipo).
    pub fn spin(&mut self, executor: NodeId) -> Result<(Vector, Momentum), InertiaError> {
        let vector = self.queue.pop_front().ok_or(InertiaError::QueueEmpty)?;
        let (log, atp) = match &vector.thrust {
            Thrust::Build => (
                format!("[inertia] build de {} ok em {}", vector.plot.short(), executor.short()),
                5,
            ),
            Thrust::Test => (
                format!("[inertia] testes de {} passaram", vector.plot.short()),
                3,
            ),
            Thrust::Deploy { target_ion } => (
                format!("[inertia] {} implantado no ion {target_ion}", vector.plot.short()),
                8,
            ),
        };
        Ok((
            vector,
            Momentum {
                success: true,
                log,
                atp_earned: atp,
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vectors_spin_in_fifo_order() {
        let mut wheel = Flywheel::new();
        let plot = ContentId::of(b"code");
        let emitter = NodeId::derive(b"dev");
        wheel.inject(Vector { plot, thrust: Thrust::Build, emitter });
        wheel.inject(Vector { plot, thrust: Thrust::Test, emitter });

        let executor = NodeId::derive(b"worker");
        let (v1, m1) = wheel.spin(executor).unwrap();
        assert_eq!(v1.thrust, Thrust::Build);
        assert_eq!(m1.atp_earned, 5);

        let (v2, m2) = wheel.spin(executor).unwrap();
        assert_eq!(v2.thrust, Thrust::Test);
        assert!(m2.success);

        assert!(matches!(wheel.spin(executor), Err(InertiaError::QueueEmpty)));
    }
}
