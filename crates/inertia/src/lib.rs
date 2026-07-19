//! # Inertia — CI/CD que viaja pela rede
//!
//! Um **Vector** é uma unidade de trabalho (build, teste, deploy) que
//! viaja pelas hifas até um nó com CPU ociosa, executa, e devolve o
//! momentum (resultado) ao emissor. Quem executa Vectors ganha ATP.
//!
//! Build/Test locais são reais: `build.sh` ou `cargo build` no workbench
//! materializado a partir das leaves do Plot.

use mycelium_core::{ContentId, NodeId};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::Path;
use std::process::Command;

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

    /// Retira o próximo Vector sem executar.
    pub fn take(&mut self) -> Option<Vector> {
        self.queue.pop_front()
    }

    /// Executa o próximo Vector no `work_dir` (build/test/deploy reais).
    pub fn spin(
        &mut self,
        executor: NodeId,
        work_dir: &Path,
    ) -> Result<(Vector, Momentum), InertiaError> {
        let vector = self.take().ok_or(InertiaError::QueueEmpty)?;
        let momentum = execute(&vector.thrust, executor, work_dir);
        Ok((vector, momentum))
    }
}

/// Materializa leaves do Plot em disco (workbench do Inertia).
pub fn materialize_leaves(
    work_dir: &Path,
    leaves: &[(String, Vec<u8>)],
) -> Result<(), std::io::Error> {
    std::fs::create_dir_all(work_dir)?;
    for (path, content) in leaves {
        let dest = work_dir.join(path);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(dest, content)?;
    }
    Ok(())
}

/// Executa um Thrust no workbench.
pub fn execute(thrust: &Thrust, executor: NodeId, work_dir: &Path) -> Momentum {
    match thrust {
        Thrust::Build => run_build(executor, work_dir),
        Thrust::Test => run_test(executor, work_dir),
        Thrust::Deploy { target_ion } => Momentum {
            success: true,
            log: format!(
                "[inertia] {} pronto para deploy no ion {target_ion}",
                executor.short()
            ),
            atp_earned: 8,
        },
    }
}

fn run_build(executor: NodeId, work_dir: &Path) -> Momentum {
    let build_sh = work_dir.join("build.sh");
    let cargo_toml = work_dir.join("Cargo.toml");

    let result = if build_sh.exists() {
        Command::new("sh")
            .arg("build.sh")
            .current_dir(work_dir)
            .output()
    } else if cargo_toml.exists() {
        Command::new("cargo")
            .args(["build", "--release"])
            .current_dir(work_dir)
            .output()
    } else {
        // Sem receita: gera artefato mínimo a partir das leaves.
        let _ = std::fs::create_dir_all(work_dir.join("dist"));
        let msg = std::fs::read_to_string(work_dir.join("MESSAGE"))
            .or_else(|_| std::fs::read_to_string(work_dir.join("message.txt")))
            .unwrap_or_else(|_| "mycelium".into());
        let html = format!(
            "<!doctype html><html><body><h1>built by inertia</h1><pre>{msg}</pre></body></html>"
        );
        let _ = std::fs::write(work_dir.join("dist/index.html"), html);
        return Momentum {
            success: true,
            log: format!(
                "[inertia] build sintético de {} em {}",
                work_dir.display(),
                executor.short()
            ),
            atp_earned: 5,
        };
    };

    match result {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            let log = format!(
                "[inertia] build em {} (exit {:?})\n{stdout}{stderr}",
                executor.short(),
                out.status.code()
            );
            if out.status.success() {
                Momentum {
                    success: true,
                    log,
                    atp_earned: 5,
                }
            } else {
                Momentum {
                    success: false,
                    log,
                    atp_earned: 0,
                }
            }
        }
        Err(e) => Momentum {
            success: false,
            log: format!("[inertia] falha ao spawnar build: {e}"),
            atp_earned: 0,
        },
    }
}

fn run_test(executor: NodeId, work_dir: &Path) -> Momentum {
    let test_sh = work_dir.join("test.sh");
    let cargo_toml = work_dir.join("Cargo.toml");

    let result = if test_sh.exists() {
        Some(
            Command::new("sh")
                .arg("test.sh")
                .current_dir(work_dir)
                .output(),
        )
    } else if cargo_toml.exists() {
        Some(
            Command::new("cargo")
                .arg("test")
                .current_dir(work_dir)
                .output(),
        )
    } else if work_dir.join("dist").exists() || work_dir.join("dist/index.html").exists() {
        return Momentum {
            success: true,
            log: format!("[inertia] testes smoke ok (dist presente) em {}", executor.short()),
            atp_earned: 3,
        };
    } else {
        None
    };

    match result {
        Some(Ok(out)) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            let log = format!(
                "[inertia] test em {} (exit {:?})\n{stdout}{stderr}",
                executor.short(),
                out.status.code()
            );
            Momentum {
                success: out.status.success(),
                log,
                atp_earned: if out.status.success() { 3 } else { 0 },
            }
        }
        Some(Err(e)) => Momentum {
            success: false,
            log: format!("[inertia] falha ao spawnar test: {e}"),
            atp_earned: 0,
        },
        None => Momentum {
            success: true,
            log: format!("[inertia] sem suite de testes — skip em {}", executor.short()),
            atp_earned: 3,
        },
    }
}

/// Recolhe o artefato do build para empacotar como layer do Vacuum.
/// Preferência: `dist/` (arquivos), senão `index.html`, senão binário release.
pub fn collect_artifact(work_dir: &Path) -> Option<Vec<(String, Vec<u8>)>> {
    let dist = work_dir.join("dist");
    if dist.is_dir() {
        let mut files = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&dist) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Ok(bytes) = std::fs::read(&path) {
                        let name = path
                            .file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or("artifact")
                            .to_string();
                        files.push((name, bytes));
                    }
                }
            }
        }
        if !files.is_empty() {
            return Some(files);
        }
    }
    let index = work_dir.join("index.html");
    if index.is_file() {
        if let Ok(bytes) = std::fs::read(&index) {
            return Some(vec![("index.html".into(), bytes)]);
        }
    }
    let release = work_dir.join("target/release");
    if release.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&release) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file()
                    && path.extension().is_none()
                    && !path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .map(|n| n.starts_with('.'))
                        .unwrap_or(true)
                {
                    if let Ok(bytes) = std::fs::read(&path) {
                        let name = path
                            .file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or("app")
                            .to_string();
                        return Some(vec![(name, bytes)]);
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vectors_spin_in_fifo_order() {
        let dir = std::env::temp_dir().join(format!(
            "inertia-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("MESSAGE"), b"hi").unwrap();

        let mut wheel = Flywheel::new();
        let plot = ContentId::of(b"code");
        let emitter = NodeId::derive(b"dev");
        wheel.inject(Vector {
            plot,
            thrust: Thrust::Build,
            emitter,
        });
        wheel.inject(Vector {
            plot,
            thrust: Thrust::Test,
            emitter,
        });

        let executor = NodeId::derive(b"worker");
        let (v1, m1) = wheel.spin(executor, &dir).unwrap();
        assert_eq!(v1.thrust, Thrust::Build);
        assert!(m1.success);
        assert_eq!(m1.atp_earned, 5);

        let (v2, m2) = wheel.spin(executor, &dir).unwrap();
        assert_eq!(v2.thrust, Thrust::Test);
        assert!(m2.success);

        assert!(matches!(
            wheel.spin(executor, &dir),
            Err(InertiaError::QueueEmpty)
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn build_sh_produces_dist_artifact() {
        let dir = std::env::temp_dir().join(format!(
            "inertia-sh-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("build.sh"),
            "#!/bin/sh\nmkdir -p dist\necho built > dist/index.html\n",
        )
        .unwrap();
        let m = execute(&Thrust::Build, NodeId::derive(b"w"), &dir);
        assert!(m.success, "{}", m.log);
        let art = collect_artifact(&dir).unwrap();
        assert_eq!(art[0].0, "index.html");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
