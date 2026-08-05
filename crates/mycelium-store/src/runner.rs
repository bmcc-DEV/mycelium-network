use crate::qemu_builder::QemuBuilder;
use crate::spore::{ExecutionEngineType, SoftwareSpore};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use thiserror::Error;
use tracing::info;

#[derive(Debug, Error)]
pub enum RunnerError {
    #[error("binário do emulador '{0}' não foi encontrado no PATH do sistema")]
    EmulatorBinaryNotFound(String),
    #[error("core libretro '{0}' não foi encontrado no sistema")]
    LibretroCoreNotFound(String),
    #[error("falha ao lançar o processo do emulador: {0}")]
    ProcessSpawn(#[from] std::io::Error),
    #[error("plataforma ou motor de execução não suportado: {0}")]
    UnsupportedEngine(String),
    #[error("erro ao preparar a imagem do jogo: {0}")]
    GamePathError(String),
}

/// Verifica se um binário existe em qualquer diretório do PATH do sistema
pub fn is_binary_in_path(name: &str) -> bool {
    if let Some(path_var) = std::env::var_os("PATH") {
        for path in std::env::split_paths(&path_var) {
            let bin_path = path.join(name);
            if bin_path.is_file() {
                return true;
            }
        }
    }
    false
}

/// Status de verificação do sistema local para saber quais emuladores estão instalados
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SystemCapabilities {
    pub has_qemu: HashMap<String, bool>, // "i386", "x86_64", "ppc", "sparc"
    pub has_retroarch: bool,
    pub available_libretro_cores: Vec<String>,
    pub has_mame: bool,
    pub has_bwrap_sandbox: bool,
}

pub struct EmulatorRunner;

impl EmulatorRunner {
    /// Detecta quais emuladores e recursos o host atual possui
    pub fn detect_capabilities() -> SystemCapabilities {
        let mut has_qemu = HashMap::new();
        for arch in &["i386", "x86_64", "ppc", "sparc", "m68k", "arm", "aarch64"] {
            let bin = format!("qemu-system-{}", arch);
            has_qemu.insert(arch.to_string(), is_binary_in_path(&bin));
        }

        let has_retroarch = is_binary_in_path("retroarch");
        let has_mame = is_binary_in_path("mame");
        let has_bwrap_sandbox = is_binary_in_path("bwrap");

        let mut available_libretro_cores = Vec::new();
        let common_core_paths = vec![
            PathBuf::from("/usr/lib/libretro"),
            PathBuf::from("/usr/lib/x86_64-linux-gnu/libretro"),
            PathBuf::from("/usr/local/lib/libretro"),
            PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config/retroarch/cores"),
        ];

        for dir in common_core_paths {
            if dir.exists() && dir.is_dir() {
                if let Ok(entries) = std::fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        if let Some(file_name) = entry.file_name().to_str() {
                            if file_name.ends_with("_libretro.so") || file_name.ends_with("_libretro.dylib") || file_name.ends_with("_libretro.dll") {
                                available_libretro_cores.push(file_name.to_string());
                            }
                        }
                    }
                }
            }
        }

        SystemCapabilities {
            has_qemu,
            has_retroarch,
            available_libretro_cores,
            has_mame,
            has_bwrap_sandbox,
        }
    }

    /// Resolve dinamicamente qual motor usar para um Spore no host atual
    pub fn resolve_best_engine(
        spore: &SoftwareSpore,
        caps: &SystemCapabilities,
        user_preference: Option<ExecutionEngineType>,
    ) -> ExecutionEngineType {
        if let Some(pref) = user_preference {
            return pref;
        }

        let matrix = &spore.execution_matrix;

        // Tenta o recomendado primeiro
        match matrix.recommended {
            ExecutionEngineType::RetroArchLibretro => {
                if caps.has_retroarch {
                    return ExecutionEngineType::RetroArchLibretro;
                }
            }
            ExecutionEngineType::QEMU => {
                if let Some(ref qemu_cfg) = matrix.qemu_config {
                    if *caps.has_qemu.get(&qemu_cfg.arch).unwrap_or(&false) {
                        return ExecutionEngineType::QEMU;
                    }
                }
            }
            ExecutionEngineType::MAME => {
                if caps.has_mame {
                    return ExecutionEngineType::MAME;
                }
            }
            ExecutionEngineType::Native => {
                if matrix.supports_native {
                    return ExecutionEngineType::Native;
                }
            }
            ExecutionEngineType::WebAssembly => {
                if matrix.supports_wasm {
                    return ExecutionEngineType::WebAssembly;
                }
            }
            ExecutionEngineType::P2PCloudStream => {
                return ExecutionEngineType::P2PCloudStream;
            }
        }

        // Fallbacks
        if matrix.supports_wasm {
            return ExecutionEngineType::WebAssembly;
        }

        if matrix.supports_p2p_stream {
            return ExecutionEngineType::P2PCloudStream;
        }

        matrix.recommended.clone()
    }

    /// Executa o Spore no motor resolvido
    pub fn launch(
        spore: &SoftwareSpore,
        game_path: &Path,
        engine: ExecutionEngineType,
        sandbox: bool,
    ) -> Result<Child, RunnerError> {
        info!(
            "[Mycelium Store Runner] Lançando '{}' [{:?}] usando motor {:?}",
            spore.title, spore.platform, engine
        );

        match engine {
            ExecutionEngineType::Native => {
                info!("[Runner] Executando binário nativo no host");
                let mut cmd = if sandbox && is_binary_in_path("bwrap") {
                    let mut bwrap = Command::new("bwrap");
                    bwrap.arg("--ro-bind").arg("/").arg("/")
                        .arg("--dev").arg("/dev")
                        .arg("--proc").arg("/proc")
                        .arg(game_path);
                    bwrap
                } else {
                    Command::new(game_path)
                };

                for arg in &spore.extra_args {
                    cmd.arg(arg);
                }

                Ok(cmd.spawn()?)
            }

            ExecutionEngineType::RetroArchLibretro => {
                if !is_binary_in_path("retroarch") {
                    return Err(RunnerError::EmulatorBinaryNotFound("retroarch".to_string()));
                }

                let core_name = spore
                    .execution_matrix
                    .libretro_core
                    .as_deref()
                    .unwrap_or("snes9x");

                let mut cmd = Command::new("retroarch");
                cmd.arg("-L").arg(core_name).arg(game_path);

                for arg in &spore.extra_args {
                    cmd.arg(arg);
                }

                Ok(cmd.spawn()?)
            }

            ExecutionEngineType::MAME => {
                if !is_binary_in_path("mame") {
                    return Err(RunnerError::EmulatorBinaryNotFound("mame".to_string()));
                }

                let driver = spore
                    .execution_matrix
                    .mame_driver
                    .as_deref()
                    .unwrap_or("arcade");

                let parent_dir = game_path
                    .parent()
                    .unwrap_or(Path::new("."))
                    .to_string_lossy();

                let mut cmd = Command::new("mame");
                cmd.arg("-rompath").arg(parent_dir.as_ref());
                
                if let Some(rom_name) = game_path.file_stem().and_then(|s| s.to_str()) {
                    cmd.arg(rom_name);
                } else {
                    cmd.arg(driver);
                }

                for arg in &spore.extra_args {
                    cmd.arg(arg);
                }

                Ok(cmd.spawn()?)
            }

            ExecutionEngineType::QEMU => {
                let qemu_cfg = spore
                    .execution_matrix
                    .qemu_config
                    .clone()
                    .unwrap_or_default();

                let (bin_name, args) = QemuBuilder::build_command(spore, &qemu_cfg, game_path);

                if !is_binary_in_path(&bin_name) {
                    return Err(RunnerError::EmulatorBinaryNotFound(bin_name));
                }

                let mut cmd = Command::new(&bin_name);
                cmd.args(&args);

                Ok(cmd.spawn()?)
            }

            ExecutionEngineType::WebAssembly => {
                info!("[Runner] Motor WebAssembly selecionado — disponível na interface web / Horizon");
                Err(RunnerError::UnsupportedEngine(
                    "Motor WebAssembly deve ser executado no navegador web / Event Horizon UI".to_string(),
                ))
            }

            ExecutionEngineType::P2PCloudStream => {
                info!("[Runner] Motor P2P Cloud Stream — instanciando Vacuum Chamber para stream");
                Err(RunnerError::UnsupportedEngine(
                    "P2P Cloud Stream iniciado via daemon de streaming Mycelium Vacuum".to_string(),
                ))
            }
        }
    }

    /// Constrói o comando (binário + args) para lançar um Spore num motor.
    /// `game_path` opcional: QEMU sem binário sobe para a BIOS (terminal útil);
    /// RetroArch/MAME sem binário reportam a falta da ROM.
    /// `terminal=true` adiciona `-nographic`/`-serial mon:stdio` (terminal web QEMU).
    pub fn build_launch_command(
        spore: &SoftwareSpore,
        game_path: Option<&Path>,
        engine: &ExecutionEngineType,
        sandbox: bool,
        terminal: bool,
    ) -> Result<(String, Vec<String>, Option<PathBuf>), RunnerError> {
        let _ = sandbox;
        let cwd: Option<PathBuf> = None;
        let mut args: Vec<String> = Vec::new();

        match engine {
            ExecutionEngineType::RetroArchLibretro => {
                if !is_binary_in_path("retroarch") {
                    return Err(RunnerError::EmulatorBinaryNotFound("retroarch".to_string()));
                }
                let core = spore
                    .execution_matrix
                    .libretro_core
                    .clone()
                    .unwrap_or_else(|| "snes9x".to_string());
                args.push("-L".to_string());
                args.push(core);
                if let Some(p) = game_path {
                    args.push(p.to_string_lossy().to_string());
                }
                for a in &spore.extra_args {
                    args.push(a.clone());
                }
                Ok(("retroarch".to_string(), args, cwd))
            }

            ExecutionEngineType::MAME => {
                if !is_binary_in_path("mame") {
                    return Err(RunnerError::EmulatorBinaryNotFound("mame".to_string()));
                }
                let driver = spore
                    .execution_matrix
                    .mame_driver
                    .clone()
                    .unwrap_or_else(|| "arcade".to_string());
                if let Some(p) = game_path {
                    if let Some(parent) = p.parent() {
                        args.push("-rompath".to_string());
                        args.push(parent.to_string_lossy().to_string());
                    }
                }
                if let Some(name) = game_path.and_then(|p| p.file_stem()).and_then(|s| s.to_str()) {
                    args.push(name.to_string());
                } else {
                    args.push(driver);
                }
                for a in &spore.extra_args {
                    args.push(a.clone());
                }
                Ok(("mame".to_string(), args, cwd))
            }

            ExecutionEngineType::QEMU => {
                let qemu_cfg = spore.execution_matrix.qemu_config.clone().unwrap_or_default();
                let bin = format!("qemu-system-{}", qemu_cfg.arch);
                if !is_binary_in_path(&bin) {
                    return Err(RunnerError::EmulatorBinaryNotFound(bin));
                }
                args.push("-m".to_string());
                args.push(qemu_cfg.memory_mb.to_string());
                if let Some(m) = &qemu_cfg.machine {
                    args.push("-M".to_string());
                    args.push(m.clone());
                }
                if let Some(cpu) = &qemu_cfg.cpu {
                    args.push("-cpu".to_string());
                    args.push(cpu.clone());
                }
                if let Some(vga) = &qemu_cfg.vga_card {
                    args.push("-vga".to_string());
                    args.push(vga.clone());
                }
                if let Some(sound) = &qemu_cfg.sound_card {
                    args.push("-audiodev".to_string());
                    args.push("id=audio0,driver=none".to_string());
                    args.push("-device".to_string());
                    args.push(format!("{},audiodev=audio0", sound));
                }
                if let Some(p) = game_path {
                    let (_, qargs) = QemuBuilder::build_command(spore, &qemu_cfg, p);
                    args.extend(qargs);
                }
                if terminal {
                    args.push("-nographic".to_string());
                    args.push("-serial".to_string());
                    args.push("mon:stdio".to_string());
                }
                for a in &spore.extra_args {
                    args.push(a.clone());
                }
                Ok((format!("qemu-system-{}", qemu_cfg.arch), args, cwd))
            }

            ExecutionEngineType::Native => {
                let Some(p) = game_path else {
                    return Err(RunnerError::GamePathError(
                        "execução nativa precisa do binário do spore".into(),
                    ));
                };
                args.push(p.to_string_lossy().to_string());
                for a in &spore.extra_args {
                    args.push(a.clone());
                }
                Ok(("sh".to_string(), args, cwd))
            }

            ExecutionEngineType::WebAssembly => Err(RunnerError::UnsupportedEngine(
                "WebAssembly roda no navegador, não no launcher host".to_string(),
            )),
            ExecutionEngineType::P2PCloudStream => Err(RunnerError::UnsupportedEngine(
                "P2P Cloud Stream exige a malha de streaming".to_string(),
            )),
        }
    }

    /// Tenta recuperar o binário do spore do SporeBank, escrever em um
    /// arquivo temporário e lançar o emulador.
    pub fn launch_spore(
        bank: &mycelium_sporebank::SporeBank,
        spore: &SoftwareSpore,
        engine: &ExecutionEngineType,
    ) -> Result<String, RunnerError> {
        let plot = bank
            .recall(&spore.content_id)
            .ok_or_else(|| RunnerError::GamePathError("spore ausente no SporeBank".into()))?;

        let leaf = plot
            .leaves
            .iter()
            .find(|l| l.path == spore.main_binary_file)
            .ok_or_else(|| {
                RunnerError::GamePathError(format!(
                    "arquivo '{}' não encontrado no spore",
                    spore.main_binary_file
                ))
            })?;

        let ext = spore
            .main_binary_file
            .rsplit('.')
            .next()
            .unwrap_or("bin");
        let tmp = std::env::temp_dir().join(format!("mycelium-{}-{}", spore.id, ext));
        std::fs::write(&tmp, &leaf.content).map_err(|e| RunnerError::GamePathError(e.to_string()))?;

        Self::launch(spore, &tmp, engine.clone(), true)?;
        Ok(format!(
            "Emulador lançado com sucesso para '{}'",
            spore.title
        ))
    }
}
