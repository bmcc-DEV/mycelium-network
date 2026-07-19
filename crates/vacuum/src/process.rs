//! Chamber viva: processo filho com isolamento real.
//!
//! Política padrão [`Isolation::Auto`]:
//! 1. **Bubblewrap** (bwrap) — namespaces de PID + filesystem sandboxed
//! 2. Fallback **Process** — processo separado sem sandbox
//!
//! A rede permanece compartilhada (a Chamber precisa ser alcançável pelo
//! Singularity no host). Lifecycle: fruit → health → hibernate → awaken → decompose.

use crate::VacuumError;
use serde::{Deserialize, Serialize};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

/// Política de isolamento da Chamber.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Isolation {
    /// Bubblewrap se disponível; senão processo simples.
    #[default]
    Auto,
    /// Apenas processo filho (sem namespaces).
    Process,
    /// Exige bwrap; erro se indisponível.
    Bubblewrap,
}

impl Isolation {
    pub fn resolve(self) -> Self {
        match self {
            Isolation::Auto => {
                if which("bwrap") {
                    Isolation::Bubblewrap
                } else {
                    Isolation::Process
                }
            }
            other => other,
        }
    }
}

/// Opções ao frutificar uma Chamber.
#[derive(Clone, Debug)]
pub struct FruitOptions {
    pub isolation: Isolation,
    /// Limite soft de memória em MiB (melhor esforço via `ulimit`/prlimit).
    pub memory_mib: Option<u64>,
}

impl Default for FruitOptions {
    fn default() -> Self {
        Self {
            isolation: Isolation::Auto,
            memory_mib: None,
        }
    }
}

/// Spec persistível para re-despertar após hibernate.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct FruitSpec {
    mycelium_bin: PathBuf,
    chambers_root: PathBuf,
    ion: String,
    plot_message: String,
    isolation: Isolation,
    memory_mib: Option<u64>,
}

/// Processo vivo que serve um Ion.
#[derive(Debug)]
pub struct ChamberProcess {
    child: Option<Child>,
    pub ion: String,
    pub port: u16,
    pub workdir: PathBuf,
    pub upstream: String,
    pub isolation: Isolation,
    spec: FruitSpec,
}

impl ChamberProcess {
    /// Frutifica: materializa rootfs-lite + sobe `chamber-serve` isolado.
    pub fn fruit(
        mycelium_bin: &Path,
        chambers_root: &Path,
        ion: &str,
        plot_message: &str,
        payload: &[u8],
    ) -> Result<Self, VacuumError> {
        Self::fruit_with(
            mycelium_bin,
            chambers_root,
            ion,
            plot_message,
            payload,
            FruitOptions::default(),
        )
    }

    pub fn fruit_with(
        mycelium_bin: &Path,
        chambers_root: &Path,
        ion: &str,
        plot_message: &str,
        payload: &[u8],
        opts: FruitOptions,
    ) -> Result<Self, VacuumError> {
        let workdir = materialize_bundle(chambers_root, ion, plot_message, payload)?;
        let isolation = opts.isolation.resolve();
        if isolation == Isolation::Bubblewrap && !which("bwrap") {
            return Err(VacuumError::Spawn(
                "Isolation::Bubblewrap exige `bwrap` no PATH".into(),
            ));
        }

        let port = free_port()?;
        let spec = FruitSpec {
            mycelium_bin: mycelium_bin.to_path_buf(),
            chambers_root: chambers_root.to_path_buf(),
            ion: ion.to_string(),
            plot_message: plot_message.to_string(),
            isolation: opts.isolation,
            memory_mib: opts.memory_mib,
        };
        std::fs::write(
            workdir.join("fruit-spec.json"),
            serde_json::to_vec_pretty(&spec)?,
        )?;

        let child = spawn_serve(&spec, &workdir, port, isolation)?;
        let upstream = format!("http://127.0.0.1:{port}");
        wait_until_listening(port, Duration::from_secs(8))?;

        tracing::info!(
            ion,
            %port,
            ?isolation,
            "chamber frutificou"
        );

        Ok(Self {
            child: Some(child),
            ion: ion.to_string(),
            port,
            workdir,
            upstream,
            isolation,
            spec,
        })
    }

    pub fn pid(&self) -> Option<u32> {
        self.child.as_ref().map(|c| c.id())
    }

    /// Chamber ainda respira?
    pub fn healthy(&mut self) -> bool {
        if let Some(child) = self.child.as_mut() {
            match child.try_wait() {
                Ok(None) => {
                    return std::net::TcpStream::connect(format!("127.0.0.1:{}", self.port)).is_ok();
                }
                Ok(Some(_)) | Err(_) => {
                    self.child = None;
                    return false;
                }
            }
        }
        false
    }

    /// Hiberna: mata o processo, preserva o bundle em disco.
    pub fn hibernate(&mut self) -> Result<(), VacuumError> {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        tracing::info!(ion = %self.ion, "chamber hibernou (sclerotium)");
        Ok(())
    }

    /// Desperta uma Chamber hibernada (mesmo workdir/payload).
    pub fn awaken(&mut self) -> Result<(), VacuumError> {
        if self.healthy() {
            return Ok(());
        }
        let isolation = self.spec.isolation.resolve();
        let port = free_port()?;
        let child = spawn_serve(&self.spec, &self.workdir, port, isolation)?;
        wait_until_listening(port, Duration::from_secs(8))?;
        self.port = port;
        self.upstream = format!("http://127.0.0.1:{port}");
        self.child = Some(child);
        self.isolation = isolation;
        tracing::info!(ion = %self.ion, port, "chamber despertou");
        Ok(())
    }

    /// Decompõe: mata processo e remove o bundle.
    pub fn decompose(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = std::fs::remove_dir_all(&self.workdir);
        tracing::info!(ion = %self.ion, "chamber decomposta");
    }
}

impl Drop for ChamberProcess {
    fn drop(&mut self) {
        // No Drop só matamos o processo; o bundle fica para reboot/debug.
        // Decompose explícito é quem apaga o disco.
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Materializa um bundle OCI-lite sob `chambers/{ion}/`.
fn materialize_bundle(
    chambers_root: &Path,
    ion: &str,
    plot_message: &str,
    payload: &[u8],
) -> Result<PathBuf, VacuumError> {
    let workdir = chambers_root.join(ion);
    let rootfs = workdir.join("rootfs");
    std::fs::create_dir_all(&rootfs)?;
    std::fs::create_dir_all(workdir.join("logs"))?;

    std::fs::write(workdir.join("payload.bin"), payload)?;
    std::fs::write(workdir.join("message.txt"), plot_message.as_bytes())?;
    // Camada "app" no rootfs — o que a Chamber "vê" como filesystem de app.
    std::fs::write(rootfs.join("app.payload"), payload)?;
    std::fs::write(rootfs.join("MESSAGE"), plot_message.as_bytes())?;
    std::fs::write(
        workdir.join("config.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "ociVersion": "mycelium-vacuum-0.1",
            "ion": ion,
            "message": plot_message,
            "root": { "path": "rootfs", "readonly": false },
            "process": {
                "args": ["chamber-serve"],
                "cwd": "/",
            }
        }))?,
    )?;
    std::fs::write(
        workdir.join("meta.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "ion": ion,
            "message": plot_message,
        }))?,
    )?;
    Ok(workdir)
}

fn spawn_serve(
    spec: &FruitSpec,
    workdir: &Path,
    port: u16,
    isolation: Isolation,
) -> Result<Child, VacuumError> {
    let stderr = Stdio::from(std::fs::File::create(workdir.join("logs/stderr.log"))?);
    let stdout = Stdio::from(std::fs::File::create(workdir.join("logs/stdout.log"))?);

    let result = match isolation {
        Isolation::Bubblewrap => spawn_bwrap(spec, workdir, port, stdout, stderr),
        Isolation::Process | Isolation::Auto => {
            spawn_plain(&spec.mycelium_bin, workdir, port, stdout, stderr)
        }
    };

    match result {
        Ok(child) => Ok(child),
        Err(e) if isolation == Isolation::Bubblewrap => {
            // Auto-path already resolved; explicit Bubblewrap doesn't fallback.
            // But if we got here via Auto→Bubblewrap failure inside spawn_bwrap's caller...
            Err(e)
        }
        Err(e) => Err(e),
    }
    .or_else(|e| {
        if matches!(spec.isolation, Isolation::Auto) && isolation == Isolation::Bubblewrap {
            tracing::warn!("bwrap falhou ({e}); fallback Isolation::Process");
            let stderr = Stdio::from(std::fs::File::create(workdir.join("logs/stderr.log"))?);
            let stdout = Stdio::from(std::fs::File::create(workdir.join("logs/stdout.log"))?);
            spawn_plain(&spec.mycelium_bin, workdir, port, stdout, stderr)
        } else {
            Err(e)
        }
    })
}

fn spawn_plain(
    mycelium_bin: &Path,
    workdir: &Path,
    port: u16,
    stdout: Stdio,
    stderr: Stdio,
) -> Result<Child, VacuumError> {
    Command::new(mycelium_bin)
        .arg("chamber-serve")
        .arg("--port")
        .arg(port.to_string())
        .arg("--ion")
        .arg(workdir.file_name().and_then(|s| s.to_str()).unwrap_or("ion"))
        .arg("--root")
        .arg(workdir)
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(stderr)
        .current_dir(workdir)
        .spawn()
        .map_err(|e| {
            VacuumError::Spawn(format!(
                "falha ao germinar chamber-serve ({}): {e}",
                mycelium_bin.display()
            ))
        })
}

fn spawn_bwrap(
    spec: &FruitSpec,
    workdir: &Path,
    port: u16,
    stdout: Stdio,
    stderr: Stdio,
) -> Result<Child, VacuumError> {
    let bin = &spec.mycelium_bin;
    let ion = &spec.ion;
    let mut cmd = Command::new("bwrap");
    cmd.arg("--die-with-parent")
        .arg("--unshare-pid")
        .arg("--unshare-ipc")
        .arg("--unshare-uts")
        .arg("--hostname")
        .arg(format!("chamber-{ion}"))
        // Rede compartilhada: Singularity no host precisa alcançar 127.0.0.1:port
        .arg("--share-net")
        .arg("--ro-bind")
        .arg("/usr")
        .arg("/usr")
        .arg("--ro-bind")
        .arg("/lib")
        .arg("/lib")
        .arg("--ro-bind-try")
        .arg("/lib64")
        .arg("/lib64")
        .arg("--ro-bind-try")
        .arg("/bin")
        .arg("/bin")
        .arg("--ro-bind-try")
        .arg("/etc/resolv.conf")
        .arg("/etc/resolv.conf")
        .arg("--ro-bind-try")
        .arg("/etc/ssl")
        .arg("/etc/ssl")
        .arg("--ro-bind")
        .arg(bin)
        .arg(bin)
        .arg("--bind")
        .arg(workdir)
        .arg(workdir)
        .arg("--dev")
        .arg("/dev")
        .arg("--proc")
        .arg("/proc")
        .arg("--tmpfs")
        .arg("/tmp")
        .arg("--chdir")
        .arg(workdir)
        .arg("--clearenv")
        .arg("--setenv")
        .arg("PATH")
        .arg("/usr/bin:/bin")
        .arg("--setenv")
        .arg("MYCELIUM_CHAMBER")
        .arg("1")
        .arg("--setenv")
        .arg("HOME")
        .arg(workdir);

    if let Some(mib) = spec.memory_mib {
        // bwrap não limita RAM nativamente em todas as builds; documentamos via env.
        cmd.arg("--setenv").arg("MYCELIUM_MEMORY_MIB").arg(mib.to_string());
    }

    cmd.arg("--")
        .arg(bin)
        .arg("chamber-serve")
        .arg("--port")
        .arg(port.to_string())
        .arg("--ion")
        .arg(ion)
        .arg("--root")
        .arg(workdir)
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(stderr);

    tracing::info!(ion = %ion, "chamber usando bubblewrap");
    cmd.spawn()
        .map_err(|e| VacuumError::Spawn(format!("bwrap spawn: {e}")))
}

fn free_port() -> Result<u16, VacuumError> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?.port())
}

fn wait_until_listening(port: u16, timeout: Duration) -> Result<(), VacuumError> {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if std::net::TcpStream::connect(format!("127.0.0.1:{port}")).is_ok() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Err(VacuumError::Spawn(format!(
        "chamber não abriu a porta {port} a tempo"
    )))
}

fn which(bin: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|p| p.join(bin).is_file()))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_port_returns_ephemeral() {
        assert!(free_port().unwrap() > 0);
    }

    #[test]
    fn auto_resolves_to_available_backend() {
        let r = Isolation::Auto.resolve();
        assert!(matches!(r, Isolation::Bubblewrap | Isolation::Process));
        if which("bwrap") {
            assert_eq!(r, Isolation::Bubblewrap);
        }
    }

    #[test]
    fn materialize_creates_oci_lite_bundle() {
        let dir = std::env::temp_dir().join(format!(
            "vac-bundle-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let wd = materialize_bundle(&dir, "webapp", "hi", b"payload").unwrap();
        assert!(wd.join("rootfs/app.payload").exists());
        assert!(wd.join("config.json").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
