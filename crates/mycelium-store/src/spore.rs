use serde::{Deserialize, Serialize};
use mycelium_core::ContentId;

/// Plataformas de hardware legado, computadores clássicos e consoles suportados.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TargetPlatform {
    /// Consoles Nintendo (NES, SNES, N64, GB, GBA, etc)
    SNES,
    NES,
    N64,
    GameBoyAdvance,
    /// Consoles Sega (Master System, Mega Drive / Genesis, Saturn, Dreamcast)
    MegaDrive,
    SegaSaturn,
    Dreamcast,
    /// Consoles Sony (PlayStation 1, PS2, PSP)
    PlayStation1,
    PlayStation2,
    /// Arcades & Placas de Hardware Legado (MAME / NeoGeo / CPS2)
    ArcadeMame,
    NeoGeo,
    /// Computadores PC Legados & Sistemas Operacionais de Época
    MSDOS,
    Windows95,
    Windows98,
    WindowsXP,
    /// Computadores Não-x86 (PowerPC Mac OS 9/X, SunOS SPARC, Commodore Amiga)
    PowerPCMac,
    Amiga,
    SunOSSPARC,
    /// Execução Nativa Host / PC Atual
    NativeSystem,
}

impl TargetPlatform {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::SNES => "Super Nintendo (SNES)",
            Self::NES => "Nintendo Entertainment System (NES)",
            Self::N64 => "Nintendo 64",
            Self::GameBoyAdvance => "Game Boy Advance",
            Self::MegaDrive => "Sega Genesis / Mega Drive",
            Self::SegaSaturn => "Sega Saturn",
            Self::Dreamcast => "Sega Dreamcast",
            Self::PlayStation1 => "Sony PlayStation 1",
            Self::PlayStation2 => "Sony PlayStation 2",
            Self::ArcadeMame => "MAME Arcade Machine",
            Self::NeoGeo => "SNK NeoGeo",
            Self::MSDOS => "MS-DOS PC",
            Self::Windows95 => "Windows 95 PC",
            Self::Windows98 => "Windows 98 PC",
            Self::WindowsXP => "Windows XP PC",
            Self::PowerPCMac => "Apple Macintosh PowerPC (Mac OS 9/X)",
            Self::Amiga => "Commodore Amiga 500/1200",
            Self::SunOSSPARC => "Sun Microsystems SPARCstation",
            Self::NativeSystem => "Sistema Nativo Host",
        }
    }
}

/// Licença de distribuição do software/ROM.
///
/// Determina se o spore pode ser distribuído publicamente pela rede P2P
/// (Shareware/Freeware/OpenSource/DomínioPúblico) ou se é apenas metadado
/// local do usuário (BYOR — Bring Your Own ROM).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "snake_case")]
pub enum SporeLicense {
    /// Distribuição autorizada oficialmente pelo detentor (ex.: DOOM 1993 Shareware)
    Shareware,
    /// Software gratuito autorizado pelo autor (freeware)
    Freeware,
    /// Licença de código aberto (GPL, MIT, etc.)
    OpenSource,
    /// Software em domínio público / licença de preservação (PD, CC0)
    PublicDomain,
    /// Copyright ativo — NÃO pode ser distribuído pela rede. Só local (BYOR).
    #[default]
    Proprietary,
}

impl SporeLicense {
    /// Se o spore pode ser distribuído publicamente pela rede P2P.
    pub fn is_publicly_redistributable(&self) -> bool {
        !matches!(self, Self::Proprietary)
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Shareware => "Shareware",
            Self::Freeware => "Freeware",
            Self::OpenSource => "Open Source",
            Self::PublicDomain => "Domínio Público",
            Self::Proprietary => "Copyright (BYOR)",
        }
    }
}

/// Motor de execução resolvedor para o software/ROM
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ExecutionEngineType {
    Native,
    RetroArchLibretro,
    MAME,
    QEMU,
    WebAssembly,
    P2PCloudStream,
}

/// Detalhes de configuração para execução em QEMU
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QemuConfig {
    pub arch: String,           // ex: "i386", "x86_64", "ppc", "sparc"
    pub machine: Option<String>,// ex: "mac99", "pc"
    pub memory_mb: u32,         // ex: 64, 256, 512
    pub cpu: Option<String>,    // ex: "pentium3", "G4"
    pub sound_card: Option<String>, // ex: "sb16", "es1370"
    pub vga_card: Option<String>,   // ex: "cirrus", "std"
    pub boot_device: String,   // ex: "c", "d", "a"
}

impl Default for QemuConfig {
    fn default() -> Self {
        Self {
            arch: "i386".to_string(),
            machine: None,
            memory_mb: 128,
            cpu: None,
            sound_card: Some("sb16".to_string()),
            vga_card: Some("std".to_string()),
            boot_device: "c".to_string(),
        }
    }
}

/// Matrix indicando quais motores conseguem executar este Spore
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionMatrix {
    pub recommended: ExecutionEngineType,
    pub supports_native: bool,
    pub libretro_core: Option<String>,  // ex: "snes9x_libretro.so", "pcsx_rearmed"
    pub mame_driver: Option<String>,    // ex: "cps2", "neogeo"
    pub qemu_config: Option<QemuConfig>,
    pub supports_wasm: bool,
    pub supports_p2p_stream: bool,
}

/// Requisitos mínimos de hardware para o host
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareRequirements {
    pub min_ram_mb: u32,
    pub min_cpu_cores: u32,
    pub storage_bytes: u64,
    pub needs_kvm_acceleration: bool,
}

impl Default for HardwareRequirements {
    fn default() -> Self {
        Self {
            min_ram_mb: 512,
            min_cpu_cores: 1,
            storage_bytes: 50 * 1024 * 1024,
            needs_kvm_acceleration: false,
        }
    }
}

/// Manifesto do Spore de Software / Jogo Legado
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoftwareSpore {
    pub id: String,
    pub title: String,
    pub description: String,
    pub developer_or_publisher: String,
    pub release_year: u16,
    pub platform: TargetPlatform,
    pub category: String, // "jogo", "sistema_operacional", "utilitario", "demoscene"
    pub tags: Vec<String>,
    pub license: SporeLicense,
    pub main_binary_file: String,
    pub content_id: ContentId,
    pub execution_matrix: ExecutionMatrix,
    pub requirements: HardwareRequirements,
    pub extra_args: Vec<String>,
    pub cover_image_url: Option<String>,
}

impl SoftwareSpore {
    pub fn create_sample_snes(id: &str, title: &str, rom_file: &str, content_id: ContentId) -> Self {
        Self {
            id: id.to_string(),
            title: title.to_string(),
            description: "Jogo clássico de Super Nintendo.".to_string(),
            developer_or_publisher: "Classics Vault".to_string(),
            release_year: 1992,
            platform: TargetPlatform::SNES,
            category: "jogo".to_string(),
            tags: vec!["retro".to_string(), "16bit".to_string(), "snes".to_string()],
            license: SporeLicense::Proprietary,
            main_binary_file: rom_file.to_string(),
            content_id,
            execution_matrix: ExecutionMatrix {
                recommended: ExecutionEngineType::RetroArchLibretro,
                supports_native: false,
                libretro_core: Some("snes9x".to_string()),
                mame_driver: None,
                qemu_config: None,
                supports_wasm: true,
                supports_p2p_stream: true,
            },
            requirements: HardwareRequirements {
                min_ram_mb: 256,
                min_cpu_cores: 1,
                storage_bytes: 4 * 1024 * 1024,
                needs_kvm_acceleration: false,
            },
            extra_args: vec![],
            cover_image_url: None,
        }
    }

    pub fn create_sample_win98(id: &str, title: &str, iso_file: &str, content_id: ContentId) -> Self {
        Self {
            id: id.to_string(),
            title: title.to_string(),
            description: "Software ou jogo legado para Windows 98 / PC MS-DOS.".to_string(),
            developer_or_publisher: "RetroSoft".to_string(),
            release_year: 1998,
            platform: TargetPlatform::Windows98,
            category: "sistema_operacional".to_string(),
            tags: vec!["win98".to_string(), "x86".to_string(), "qemu".to_string()],
            license: SporeLicense::Proprietary,
            main_binary_file: iso_file.to_string(),
            content_id,
            execution_matrix: ExecutionMatrix {
                recommended: ExecutionEngineType::QEMU,
                supports_native: false,
                libretro_core: None,
                mame_driver: None,
                qemu_config: Some(QemuConfig {
                    arch: "i386".to_string(),
                    machine: Some("pc".to_string()),
                    memory_mb: 256,
                    cpu: Some("pentium3".to_string()),
                    sound_card: Some("sb16".to_string()),
                    vga_card: Some("cirrus".to_string()),
                    boot_device: "c".to_string(),
                }),
                supports_wasm: false,
                supports_p2p_stream: true,
            },
            requirements: HardwareRequirements {
                min_ram_mb: 1024,
                min_cpu_cores: 2,
                storage_bytes: 2000 * 1024 * 1024,
                needs_kvm_acceleration: false,
            },
            extra_args: vec![],
            cover_image_url: None,
        }
    }
}
