use crate::spore::{QemuConfig, SoftwareSpore, TargetPlatform};
use std::path::Path;
use tracing::info;

pub struct QemuBuilder;

impl QemuBuilder {
    /// Constrói o comando de inicialização do QEMU com argumentos adequados
    pub fn build_command(
        spore: &SoftwareSpore,
        config: &QemuConfig,
        game_path: &Path,
    ) -> (String, Vec<String>) {
        let binary_name = format!("qemu-system-{}", config.arch);
        let mut args: Vec<String> = Vec::new();

        // Memória RAM
        args.push("-m".to_string());
        args.push(config.memory_mb.to_string());

        // Placa mãe/máquina específica se configurada
        if let Some(ref m) = config.machine {
            args.push("-M".to_string());
            args.push(m.clone());
        }

        // CPU emulada
        if let Some(ref cpu) = config.cpu {
            args.push("-cpu".to_string());
            args.push(cpu.clone());
        }

        // Placa de vídeo
        if let Some(ref vga) = config.vga_card {
            args.push("-vga".to_string());
            args.push(vga.clone());
        }

        // Placa de som (sintaxe QEMU 8+): audiodev + device
        if let Some(ref sound) = config.sound_card {
            args.push("-audiodev".to_string());
            args.push("id=audio0,driver=none".to_string());
            args.push("-device".to_string());
            args.push(format!("{},audiodev=audio0", sound));
        }

        // Configuração de Drive de Armazenamento / Imagem de Disco / ROM / CDROM
        let file_ext = game_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        if file_ext == "iso" || file_ext == "cue" || file_ext == "img" {
            args.push("-cdrom".to_string());
            args.push(game_path.to_string_lossy().to_string());
            args.push("-boot".to_string());
            args.push("d".to_string());
        } else if file_ext == "qcow2" || file_ext == "vmdk" || file_ext == "raw" {
            args.push("-drive".to_string());
            args.push(format!(
                "file={},format={},if=ide",
                game_path.display(),
                if file_ext == "qcow2" { "qcow2" } else { "raw" }
            ));
            args.push("-boot".to_string());
            args.push(config.boot_device.clone());
        } else {
            // Outras mídias ou imagem genérica
            args.push("-hda".to_string());
            args.push(game_path.to_string_lossy().to_string());
        }

        // Argumentos extras para MS-DOS, Win95/98 ou PowerPC
        match spore.platform {
            TargetPlatform::MSDOS => {
                info!("[QEMU Builder] Otimizando máquina virtual para MS-DOS");
            }
            TargetPlatform::PowerPCMac => {
                info!("[QEMU Builder] Otimizando máquina virtual para Mac OS 9/X (PowerPC)");
                if config.machine.is_none() {
                    args.push("-M".to_string());
                    args.push("mac99".to_string());
                }
            }
            TargetPlatform::SunOSSPARC => {
                info!("[QEMU Builder] Otimizando máquina virtual para SunOS SPARCstation");
                if config.machine.is_none() {
                    args.push("-M".to_string());
                    args.push("SS-5".to_string());
                }
            }
            _ => {}
        }

        // Adiciona argumentos de software extras
        for arg in &spore.extra_args {
            args.push(arg.clone());
        }

        (binary_name, args)
    }
}
