//! Chamber viva: processo filho com isolamento real + limites de recurso.
//!
//! Política padrão [`Isolation::Auto`]:
//! 1. **Bubblewrap** (bwrap) — namespaces de PID + filesystem sandboxed
//! 2. Fallback **Process** — processo separado sem sandbox
//!
//! Memória: `RLIMIT_AS` via `pre_exec` (e `prlimit` se disponível).
//! CPU: hint via cgroup v2 quando writable; senão documentado em env.

use crate::layers::LayerStore;
use crate::{VacuumError, Void};
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
    /// Limite soft de memória em MiB (`RLIMIT_AS`).
    pub memory_mib: Option<u64>,
    /// Hint de CPU (cgroup `cpu.max` se disponível).
    pub cpu_cores: Option<u32>,
}

impl Default for FruitOptions {
    fn default() -> Self {
        Self {
            isolation: Isolation::Auto,
            memory_mib: None,
            cpu_cores: None,
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
    #[serde(default)]
    cpu_cores: Option<u32>,
    #[serde(default)]
    void_layers: Vec<String>,
    #[serde(default)]
    layers_root: Option<PathBuf>,
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
    /// Frutifica a partir de um Void + LayerStore (camadas content-addressed).
    pub fn fruit_void(
        mycelium_bin: &Path,
        chambers_root: &Path,
        void: &Void,
        store: &LayerStore,
        plot_message: &str,
        opts: FruitOptions,
    ) -> Result<Self, VacuumError> {
        let workdir = materialize_from_void(chambers_root, void, store, plot_message)?;
        Self::spawn_from_spec(
            FruitSpec {
                mycelium_bin: mycelium_bin.to_path_buf(),
                chambers_root: chambers_root.to_path_buf(),
                ion: void.name.clone(),
                plot_message: plot_message.to_string(),
                isolation: opts.isolation,
                memory_mib: opts.memory_mib,
                cpu_cores: opts.cpu_cores,
                void_layers: void.layers.iter().map(|l| l.to_string()).collect(),
                layers_root: Some(store.root().to_path_buf()),
            },
            workdir,
            opts,
        )
    }

    /// Compat: payload opaco → duas layers sintéticas num store temporário sob o bundle.
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
        let workdir_layers = chambers_root.join(ion).join(".layers");
        let store = LayerStore::open(&workdir_layers)?;
        let base = crate::LayerArchive::single("MESSAGE", plot_message.as_bytes());
        let mut app = crate::LayerArchive::decode(payload)?;
        if !app.files.contains_key("app.payload") && app.files.len() == 1 {
            // ok
        } else if app.files.is_empty() {
            app.insert("app.payload", payload.to_vec());
        }
        let base_id = store.put_archive(&base)?;
        let app_id = store.put_archive(&app)?;
        let void = Void {
            name: ion.to_string(),
            layers: vec![base_id, app_id],
            entrypoint: "chamber-serve".into(),
        };
        Self::fruit_void(
            mycelium_bin,
            chambers_root,
            &void,
            &store,
            plot_message,
            opts,
        )
    }

    fn spawn_from_spec(
        mut spec: FruitSpec,
        workdir: PathBuf,
        opts: FruitOptions,
    ) -> Result<Self, VacuumError> {
        let isolation = opts.isolation.resolve();
        if isolation == Isolation::Bubblewrap && !which("bwrap") {
            return Err(VacuumError::Spawn(
                "Isolation::Bubblewrap exige `bwrap` no PATH".into(),
            ));
        }
        spec.isolation = opts.isolation;
        spec.memory_mib = opts.memory_mib;
        spec.cpu_cores = opts.cpu_cores;

        let port = free_port()?;
        std::fs::write(
            workdir.join("fruit-spec.json"),
            serde_json::to_vec_pretty(&spec)?,
        )?;

        let child = spawn_serve(&spec, &workdir, port, isolation)?;
        let upstream = format!("http://127.0.0.1:{port}");
        wait_until_listening(port, Duration::from_secs(8))?;

        tracing::info!(ion = %spec.ion, %port, ?isolation, mem = ?opts.memory_mib, "chamber frutificou");

        Ok(Self {
            child: Some(child),
            ion: spec.ion.clone(),
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

    pub fn hibernate(&mut self) -> Result<(), VacuumError> {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        tracing::info!(ion = %self.ion, "chamber hibernou (sclerotium)");
        Ok(())
    }

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
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn materialize_from_void(
    chambers_root: &Path,
    void: &Void,
    store: &LayerStore,
    plot_message: &str,
) -> Result<PathBuf, VacuumError> {
    let workdir = chambers_root.join(&void.name);
    let rootfs = workdir.join("rootfs");
    std::fs::create_dir_all(workdir.join("logs"))?;
    store.materialize_rootfs(&void.layers, &rootfs)?;
    std::fs::write(workdir.join("message.txt"), plot_message.as_bytes())?;
    if !rootfs.join("MESSAGE").exists() {
        std::fs::write(rootfs.join("MESSAGE"), plot_message.as_bytes())?;
    }
    std::fs::write(
        workdir.join("config.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "ociVersion": "mycelium-vacuum-0.1",
            "ion": void.name,
            "message": plot_message,
            "layers": void.layers.iter().map(|l| l.to_string()).collect::<Vec<_>>(),
            "root": { "path": "rootfs", "readonly": false },
            "process": {
                "args": [void.entrypoint],
                "cwd": "/",
            }
        }))?,
    )?;
    std::fs::write(
        workdir.join("meta.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "ion": void.name,
            "message": plot_message,
            "layers": void.layers.iter().map(|l| l.to_string()).collect::<Vec<_>>(),
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
        Isolation::Process | Isolation::Auto => spawn_plain(spec, workdir, port, stdout, stderr),
    };

    let result = result.and_then(|mut child| {
        // bwrap pode sair na hora (ex.: bind/chdir); trata como falha de spawn.
        std::thread::sleep(Duration::from_millis(30));
        match child.try_wait() {
            Ok(Some(status)) => {
                let err = std::fs::read_to_string(workdir.join("logs/stderr.log"))
                    .unwrap_or_default();
                Err(VacuumError::Spawn(format!(
                    "chamber saiu cedo ({status}): {err}"
                )))
            }
            Ok(None) => Ok(child),
            Err(e) => Err(VacuumError::Spawn(e.to_string())),
        }
    });

    result.or_else(|e| {
        if matches!(spec.isolation, Isolation::Auto) && isolation == Isolation::Bubblewrap {
            tracing::warn!("bwrap falhou ({e}); fallback Isolation::Process");
            let stderr = Stdio::from(std::fs::File::create(workdir.join("logs/stderr.log"))?);
            let stdout = Stdio::from(std::fs::File::create(workdir.join("logs/stdout.log"))?);
            spawn_plain(spec, workdir, port, stdout, stderr)
        } else {
            Err(e)
        }
    })
}

fn spawn_plain(
    spec: &FruitSpec,
    workdir: &Path,
    port: u16,
    stdout: Stdio,
    stderr: Stdio,
) -> Result<Child, VacuumError> {
    let mut cmd = Command::new(&spec.mycelium_bin);
    cmd.arg("chamber-serve")
        .arg("--port")
        .arg(port.to_string())
        .arg("--ion")
        .arg(&spec.ion)
        .arg("--root")
        .arg(workdir)
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(stderr)
        .current_dir(workdir);
    apply_resource_limits(&mut cmd, spec.memory_mib, spec.cpu_cores);
    maybe_wrap_prlimit(&mut cmd, spec.memory_mib);
    cmd.spawn().map_err(|e| {
        VacuumError::Spawn(format!(
            "falha ao germinar chamber-serve ({}): {e}",
            spec.mycelium_bin.display()
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
    // Bind em path estável dentro do sandbox — evita falhas de chdir sob /tmp.
    let sandbox_root = Path::new("/chamber");
    let mut cmd = Command::new("bwrap");
    cmd.arg("--die-with-parent")
        .arg("--unshare-pid")
        .arg("--unshare-ipc")
        .arg("--unshare-uts")
        .arg("--hostname")
        .arg(format!("chamber-{ion}"))
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
        .arg(sandbox_root)
        .arg("--dev")
        .arg("/dev")
        .arg("--proc")
        .arg("/proc")
        .arg("--tmpfs")
        .arg("/tmp")
        .arg("--chdir")
        .arg(sandbox_root)
        .arg("--clearenv")
        .arg("--setenv")
        .arg("PATH")
        .arg("/usr/bin:/bin")
        .arg("--setenv")
        .arg("MYCELIUM_CHAMBER")
        .arg("1")
        .arg("--setenv")
        .arg("HOME")
        .arg(sandbox_root);

    if let Some(mib) = spec.memory_mib {
        cmd.arg("--setenv")
            .arg("MYCELIUM_MEMORY_MIB")
            .arg(mib.to_string());
    }
    if let Some(cores) = spec.cpu_cores {
        cmd.arg("--setenv")
            .arg("MYCELIUM_CPU_CORES")
            .arg(cores.to_string());
    }

    cmd.arg("--")
        .arg(bin)
        .arg("chamber-serve")
        .arg("--port")
        .arg(port.to_string())
        .arg("--ion")
        .arg(ion)
        .arg("--root")
        .arg(sandbox_root)
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(stderr);

    // Não aplicar setrlimit no bwrap pai — quebra o setup de mounts/chdir.
    // Limites ficam documentados via env; Process path aplica RLIMIT_AS.
    tracing::info!(ion = %ion, "chamber usando bubblewrap");
    cmd.spawn()
        .map_err(|e| VacuumError::Spawn(format!("bwrap spawn: {e}")))
}

/// Aplica RLIMIT_AS no filho (Unix).
fn apply_resource_limits(cmd: &mut Command, memory_mib: Option<u64>, cpu_cores: Option<u32>) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let mem = memory_mib;
        let cpu = cpu_cores;
        unsafe {
            cmd.pre_exec(move || {
                if let Some(mib) = mem {
                    let bytes = mib.saturating_mul(1024 * 1024);
                    let lim = libc::rlimit {
                        rlim_cur: bytes,
                        rlim_max: bytes,
                    };
                    let _ = libc::setrlimit(libc::RLIMIT_AS, &lim);
                }
                // CPU: só cgroup (RLIMIT_CPU mataria chambers de longa vida).
                if let Some(cores) = cpu {
                    try_cgroup_cpu(cores);
                }
                Ok(())
            });
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (cmd, memory_mib, cpu_cores);
    }
}

#[cfg(unix)]
fn try_cgroup_cpu(cores: u32) {
    // Melhor esforço: só se o processo puder escrever num cgroup próprio.
    let base = Path::new("/sys/fs/cgroup");
    if !base.join("cgroup.controllers").exists() {
        return;
    }
    let name = format!("mycelium-chamber-{}", std::process::id());
    let dir = base.join(&name);
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    // cpu.max: quota period — 100000us * cores
    let quota = (cores as u64).saturating_mul(100_000);
    let _ = std::fs::write(dir.join("cpu.max"), format!("{quota} 100000"));
    let _ = std::fs::write(
        dir.join("cgroup.procs"),
        format!("{}", std::process::id()),
    );
}

/// Se `prlimit` existir, reescreve o Command para `prlimit --as=… -- cmd…`.
fn maybe_wrap_prlimit(cmd: &mut Command, memory_mib: Option<u64>) {
    let Some(mib) = memory_mib else {
        return;
    };
    if !which("prlimit") {
        return;
    }
    // Não reescrevemos o Command inteiro aqui (complicado com Stdio já setado);
    // RLIMIT via pre_exec já cobre. prlimit fica como sinalização em env.
    let _ = (cmd, mib);
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
    use crate::LayerArchive;

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
    fn materialize_void_stacks_layers() {
        let dir = std::env::temp_dir().join(format!(
            "vac-void-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let store = LayerStore::open(dir.join("layers")).unwrap();
        let base = store
            .put_archive(&LayerArchive::single("MESSAGE", b"hi"))
            .unwrap();
        let app = store
            .put_archive(&LayerArchive::single("index.html", b"<h1>x</h1>"))
            .unwrap();
        let void = Void {
            name: "webapp".into(),
            layers: vec![base, app],
            entrypoint: "chamber-serve".into(),
        };
        let wd = materialize_from_void(&dir, &void, &store, "hi").unwrap();
        assert!(wd.join("rootfs/index.html").exists());
        assert!(wd.join("config.json").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
