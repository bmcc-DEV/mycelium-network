//! # Process Manager — execução supervisionada de emuladores
//!
//! Gerencia processos de emuladores (RetroArch, MAME, QEMU) lançados pelo
//! launcher: captura stdout/stderr em buffer, aceita input no stdin (terminal
//! web para QEMU `-nographic`) e permite parar o processo.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{error, info, warn};

/// Um processo de emulador supervisionado.
pub struct ManagedProcess {
    pub id: u64,
    pub pid: u32,
    pub spore_id: String,
    pub engine: String,
    pub label: String,
    pub status: String,
    pub started_at: u64,
    pub exit_code: Option<i32>,
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    buffer: Arc<Mutex<Vec<u8>>>,
    consumed: usize,
}

impl ManagedProcess {
    /// Bytes novos de output desde a última consulta.
    pub fn drain(&mut self) -> Vec<u8> {
        let buf = self.buffer.lock().unwrap();
        if self.consumed > buf.len() {
            self.consumed = 0;
        }
        let out = buf[self.consumed..].to_vec();
        self.consumed = buf.len();
        out
    }

    /// Envia bytes para o stdin do processo (terminal).
    pub fn send_input(&mut self, bytes: &[u8]) -> Result<(), String> {
        let stdin = self.stdin.as_mut().ok_or("stdin indisponível".to_string())?;
        stdin
            .write_all(bytes)
            .map_err(|e| format!("escrever no stdin: {e}"))
    }

    pub fn is_running(&mut self) -> bool {
        match self.child.as_mut() {
            Some(child) => match child.try_wait() {
                Ok(Some(status)) => {
                    self.exit_code = status.code();
                    self.status = if self.exit_code == Some(0) {
                        "exited".to_string()
                    } else {
                        "errored".to_string()
                    };
                    self.stdin.take();
                    false
                }
                Ok(None) => true,
                Err(_) => true,
            },
            None => false,
        }
    }
}

/// Gestor global de processos do launcher.
#[derive(Default)]
pub struct ProcessManager {
    pub processes: HashMap<u64, ManagedProcess>,
    next_id: u64,
}

impl ProcessManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Lança um emulador supervisionado e devolve o id.
    pub fn spawn(
        &mut self,
        program: &str,
        args: &[String],
        cwd: Option<PathBuf>,
        spore_id: String,
        engine: String,
        label: String,
    ) -> Result<u64, String> {
        let mut cmd = Command::new(program);
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }
        let mut child = cmd.spawn().map_err(|e| format!("spawn {program}: {e}"))?;

        let buffer = Arc::new(Mutex::new(Vec::new()));
        let pid = child.id();

        let stdin = child.stdin.take();
        let stdout = child.stdout.take().ok_or("sem stdout")?;
        let stderr = child.stderr.take().ok_or("sem stderr")?;
        let buf_out = buffer.clone();
        std::thread::spawn(move || pump(stdout, buf_out, "stdout"));
        let buf_err = buffer.clone();
        std::thread::spawn(move || pump(stderr, buf_err, "stderr"));

        let started_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let id = self.next_id;
        self.next_id += 1;

        self.processes.insert(
            id,
            ManagedProcess {
                id,
                pid,
                spore_id,
                engine: engine.clone(),
                label,
                status: "running".to_string(),
                started_at,
                exit_code: None,
                child: Some(child),
                stdin,
                buffer,
                consumed: 0,
            },
        );
        info!(id, pid, engine = %engine, "launcher: processo iniciado");
        Ok(id)
    }

    pub fn get_mut(&mut self, id: u64) -> Option<&mut ManagedProcess> {
        self.processes.get_mut(&id)
    }

    /// Mata o processo (SIGTERM → SIGKILL) e remove-o da tabela.
    pub fn stop(&mut self, id: u64) -> Option<ManagedProcess> {
        let mut p = self.processes.remove(&id)?;
        p.stdin.take();
        if let Some(mut child) = p.child.take() {
            let _ = child.kill();
            let _ = child.wait();
            p.exit_code = Some(0);
            p.status = "stopped".to_string();
        }
        info!(id, "launcher: processo parado");
        Some(p)
    }

    /// Recolhe processos que já saíram (prune) e devolve snapshot para a UI.
    pub fn snapshot(&mut self) -> Vec<serde_json::Value> {
        let mut out = Vec::new();
        let mut dead = Vec::new();
        for (id, p) in self.processes.iter_mut() {
            p.is_running();
            out.push(serde_json::json!({
                "id": p.id,
                "pid": p.pid,
                "spore_id": p.spore_id,
                "engine": p.engine,
                "label": p.label,
                "status": p.status,
                "started_at": p.started_at,
                "exit_code": p.exit_code,
            }));
            if p.status != "running" {
                dead.push(*id);
            }
        }
        for id in dead {
            if let Some(p) = self.processes.remove(&id) {
                warn!(id, status = p.status, "launcher: processo removido");
            }
        }
        out
    }
}

/// Lê um pipe até EOF e acumula no buffer partilhado.
fn pump<R: Read>(mut reader: R, buffer: Arc<Mutex<Vec<u8>>>, tag: &'static str) {
    let mut chunk = [0u8; 8192];
    loop {
        match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                if let Ok(mut buf) = buffer.lock() {
                    buf.extend_from_slice(&chunk[..n]);
                }
            }
            Err(e) => {
                error!(tag, "launcher: erro a ler pipe: {e}");
                break;
            }
        }
    }
}

// NOTA: `ManagedProcess.stdin` é preenchido no `spawn` com o `ChildStdin` do child.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_capture_and_drain() {
        let mut mgr = ProcessManager::new();
        let id = mgr
            .spawn(
                "sh",
                &["-c".to_string(), "echo hello-from-launcher; sleep 0.2; echo done".to_string()],
                None,
                "test".into(),
                "shell".into(),
                "teste".into(),
            )
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(600));
        let mut p = mgr.get_mut(id).unwrap();
        let out = String::from_utf8_lossy(&p.drain()).to_string();
        assert!(out.contains("hello-from-launcher"), "output: {out}");
        assert!(out.contains("done"), "output: {out}");
        assert!(!p.is_running());
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    #[test]
    fn stop_kills_process() {
        let mut mgr = ProcessManager::new();
        let id = mgr
            .spawn(
                "sh",
                &["-c".to_string(), "sleep 30".to_string()],
                None,
                "test".into(),
                "shell".into(),
                "teste".into(),
            )
            .unwrap();
        assert!(mgr.stop(id).is_some());
        assert!(!mgr.processes.contains_key(&id));
    }
}
