//! # Mycelium Store — Decentralized Retro Software & Hardware Launcher Store
//!
//! A Mycelium Store é uma App Store / Steam descentralizada para software antigo, jogos retro e
//! plataformas de hardware legado, construída sobre o ecossistema P2P Mycelium Network.
//!
//! Recursos:
//! - Catálogo descentralizado com SporeBank (IPFS / DHT / Nostr)
//! - Motor de emulação dinâmica: RetroArch (Libretro), MAME, QEMU (x86, PPC, SPARC)
//! - WebAssembly embutido (RetroArch/EmulatorJS) para execução em navegadores
//! - Cloud Gaming P2P e sandboxing de segurança com Vacuum (bwrap)

pub mod catalog;
pub mod process;
pub mod qemu_builder;
pub mod runner;
pub mod spore;

pub use catalog::StoreCatalog;
pub use process::ProcessManager;
pub use qemu_builder::QemuBuilder;
pub use runner::{EmulatorRunner, SystemCapabilities};
pub use spore::{ExecutionEngineType, ExecutionMatrix, QemuConfig, SoftwareSpore, SporeLicense, TargetPlatform};

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use axum::extract::Path as AxumPath;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct StoreAppState {
    pub catalog: Arc<Mutex<StoreCatalog>>,
    pub processes: Arc<Mutex<ProcessManager>>,
    pub home: PathBuf,
}

/// Cria o router HTTP Axum para a Mycelium Store integrar ao Singularity Event Horizon
pub fn create_store_router(home: impl AsRef<Path>, catalog: Arc<Mutex<StoreCatalog>>) -> Router {
    let state = StoreAppState {
        catalog,
        processes: Arc::new(Mutex::new(ProcessManager::new())),
        home: home.as_ref().to_path_buf(),
    };

    Router::new()
        .route("/api/store/capabilities", get(get_capabilities_handler))
        .route("/api/store/spores", get(list_spores_handler))
        .route("/api/store/spores/{id}", get(get_spore_handler))
        .route("/api/store/spores/{id}/launch", post(launch_spore_handler))
        .route("/api/store/covers/{id}", get(get_cover_handler))
        .route("/api/store/processes", get(list_processes_handler))
        .route("/api/store/processes/{id}/output", get(process_output_handler))
        .route("/api/store/processes/{id}/input", post(process_input_handler))
        .route("/api/store/processes/{id}/stop", post(process_stop_handler))
        .with_state(state)
}

/// Cria o router que serve a UI estática da store (index.html e game.html)
pub fn create_store_ui_router(home: impl AsRef<Path>) -> Router {
    let home = home.as_ref().to_path_buf();
    let home_for_ui = home.clone();
    Router::new()
        .route("/", get(move || serve_store_ui(home.clone())))
        .route("/web/{id}", get(move |AxumPath(id): AxumPath<String>| serve_game_page(home_for_ui.clone(), id)))
}

async fn serve_store_ui(home: PathBuf) -> impl IntoResponse {
    let default_ui = include_str!("../../../deploy/store-ui/index.html");
    let ui_path = home.join("store-ui").join("index.html");
    let html = std::fs::read_to_string(&ui_path).unwrap_or_else(|_| default_ui.to_string());
    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
        html,
    )
}

async fn serve_game_page(home: PathBuf, id: String) -> impl IntoResponse {
    let default_ui = include_str!("../../../deploy/store-ui/game.html");
    let ui_path = home.join("store-ui").join("game.html");
    let mut html = std::fs::read_to_string(&ui_path).unwrap_or_else(|_| default_ui.to_string());
    html = html.replace("__SPORE_ID__", &id);
    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
        html,
    )
}

async fn get_capabilities_handler() -> impl IntoResponse {
    let caps = EmulatorRunner::detect_capabilities();
    (StatusCode::OK, Json(caps))
}

/// Lista apenas spores com licença de distribuição pública.
/// Spores `proprietary` (BYOR) não aparecem na loja pública.
async fn list_spores_handler(State(state): State<StoreAppState>) -> impl IntoResponse {
    let catalog = state.catalog.lock().unwrap();
    let spores: Vec<SoftwareSpore> = catalog.list_public_spores().into_iter().cloned().collect();
    (StatusCode::OK, Json(spores))
}

async fn get_spore_handler(
    State(state): State<StoreAppState>,
    AxumPath(id): AxumPath<String>,
) -> impl IntoResponse {
    let catalog = state.catalog.lock().unwrap();
    if let Some(spore) = catalog.get_spore(&id) {
        (StatusCode::OK, Json(json!(spore)))
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Spore não encontrado"})),
        )
    }
}

async fn launch_spore_handler(
    State(state): State<StoreAppState>,
    AxumPath(id): AxumPath<String>,
) -> impl IntoResponse {
    let spore = {
        let catalog = state.catalog.lock().unwrap();
        match catalog.get_spore(&id) {
            Some(s) => s.clone(),
            None => {
                return (
                    StatusCode::NOT_FOUND,
                    [(axum::http::header::CONTENT_TYPE, "application/json; charset=utf-8")],
                    json!({"error": format!("spore '{id}' não encontrado")}).to_string(),
                );
            }
        }
    };

    let caps = EmulatorRunner::detect_capabilities();
    let engine = EmulatorRunner::resolve_best_engine(&spore, &caps, None);

    let game_path = resolve_game_path(&state, &spore);
    let terminal = engine == ExecutionEngineType::QEMU;

    match EmulatorRunner::build_launch_command(&spore, game_path.as_deref(), &engine, false, terminal)
    {
        Ok((program, args, cwd)) => {
            let mut processes = state.processes.lock().unwrap();
            match processes.spawn(
                &program,
                &args,
                cwd,
                spore.id.clone(),
                format!("{engine:?}"),
                spore.title.clone(),
            ) {
                Ok(process_id) => {
                    let bin = game_path.map(|p| p.to_string_lossy().to_string());
                    let mode = if terminal {
                        "terminal-web"
                    } else {
                        "janela-host"
                    };
                    (
                        StatusCode::OK,
                        [(axum::http::header::CONTENT_TYPE, "application/json; charset=utf-8")],
                        json!({
                            "status": "launched",
                            "spore_id": spore.id,
                            "engine": format!("{engine:?}"),
                            "process_id": process_id,
                            "mode": mode,
                            "binary": bin,
                            "message": format!(
                                "{} iniciado via {} ({})",
                                spore.title,
                                program,
                                mode
                            ),
                        })
                        .to_string(),
                    )
                }
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    [(axum::http::header::CONTENT_TYPE, "application/json; charset=utf-8")],
                    json!({ "status": "error", "message": e }).to_string(),
                ),
            }
        }
        Err(e) => (
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "application/json; charset=utf-8")],
            json!({
                "status": "prepared",
                "engine": format!("{engine:?}"),
                "message": format!("{} não pôde ser lançado: {e}", spore.title),
            })
            .to_string(),
        ),
    }
}

fn resolve_game_path(state: &StoreAppState, spore: &SoftwareSpore) -> Option<PathBuf> {
    let local = state.home.join("store").join(&spore.main_binary_file);
    if local.exists() {
        return Some(local);
    }
    let catalog = state.catalog.lock().unwrap();
    let plot = catalog.sporebank().recall(&spore.content_id)?;
    let leaf = plot
        .leaves
        .iter()
        .find(|l| l.path == spore.main_binary_file)?;
    let ext = spore
        .main_binary_file
        .rsplit('.')
        .next()
        .unwrap_or("bin");
    let tmp = std::env::temp_dir().join(format!("mycelium-{}-{}", spore.id, ext));
    if std::fs::write(&tmp, &leaf.content).is_ok() {
        Some(tmp)
    } else {
        None
    }
}

async fn list_processes_handler(State(state): State<StoreAppState>) -> impl IntoResponse {
    let mut processes = state.processes.lock().unwrap();
    let snapshot = processes.snapshot();
    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/json; charset=utf-8")],
        serde_json::to_string(&json!({ "processes": snapshot })).unwrap_or_else(|_| "{}".into()),
    )
}

async fn process_output_handler(
    State(state): State<StoreAppState>,
    AxumPath(id): AxumPath<String>,
) -> impl IntoResponse {
    let id: u64 = match id.parse() {
        Ok(v) => v,
        Err(_) => return process_json_err(StatusCode::BAD_REQUEST, "id de processo inválido".into()),
    };
    let mut processes = state.processes.lock().unwrap();
    match processes.get_mut(id) {
        Some(p) => {
            let output = p.drain();
            let status = p.status.clone();
            let output_str = String::from_utf8_lossy(&output).to_string();
            (
                StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, "application/json; charset=utf-8")],
                json!({ "process_id": id, "status": status, "output": output_str }).to_string(),
            )
        }
        None => process_json_err(StatusCode::NOT_FOUND, format!("processo {id} não encontrado")),
    }
}

async fn process_input_handler(
    State(state): State<StoreAppState>,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let id: u64 = match id.parse() {
        Ok(v) => v,
        Err(_) => return process_json_err(StatusCode::BAD_REQUEST, "id de processo inválido".into()),
    };
    let input = body
        .get("input")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let mut processes = state.processes.lock().unwrap();
    match processes.get_mut(id) {
        Some(p) => match p.send_input(input.as_bytes()) {
            Ok(()) => (
                StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, "application/json; charset=utf-8")],
                json!({ "ok": true }).to_string(),
            ),
            Err(e) => process_json_err(StatusCode::INTERNAL_SERVER_ERROR, e),
        },
        None => process_json_err(StatusCode::NOT_FOUND, format!("processo {id} não encontrado")),
    }
}

async fn process_stop_handler(
    State(state): State<StoreAppState>,
    AxumPath(id): AxumPath<String>,
) -> impl IntoResponse {
    let id: u64 = match id.parse() {
        Ok(v) => v,
        Err(_) => return process_json_err(StatusCode::BAD_REQUEST, "id de processo inválido".into()),
    };
    let mut processes = state.processes.lock().unwrap();
    match processes.stop(id) {
        Some(p) => (
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "application/json; charset=utf-8")],
            json!({ "ok": true, "stopped": p.spore_id }).to_string(),
        ),
        None => process_json_err(StatusCode::NOT_FOUND, format!("processo {id} não encontrado")),
    }
}

fn process_json_err(
    status: StatusCode,
    message: String,
) -> (StatusCode, [(axum::http::header::HeaderName, &'static str); 1], String) {
    (
        status,
        [(axum::http::header::CONTENT_TYPE, "application/json; charset=utf-8")],
        json!({ "error": message }).to_string(),
    )
}

async fn get_cover_handler(
    State(state): State<StoreAppState>,
    AxumPath(id): AxumPath<String>,
) -> impl IntoResponse {
    let id = id.strip_suffix(".svg").unwrap_or(&id).to_string();
    let catalog = state.catalog.lock().unwrap();
    let (title, platform) = match catalog.get_spore(&id) {
        Some(spore) => (spore.title.clone(), spore.platform.clone()),
        None => (format!("Spore não encontrado: {id}"), spore::TargetPlatform::NativeSystem),
    };
    let svg = cover_svg(&title, &platform);
    (
        StatusCode::OK,
        [
            (axum::http::header::CONTENT_TYPE, "image/svg+xml; charset=utf-8"),
            (axum::http::header::CACHE_CONTROL, "public, max-age=86400"),
        ],
        svg,
    )
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn platform_gradient(platform: &spore::TargetPlatform) -> (&'static str, &'static str) {
    use spore::TargetPlatform::*;
    match platform {
        MSDOS | Windows95 | Windows98 | WindowsXP => ("#0f2a3d", "#0e7c86"),
        SNES | NES | N64 | GameBoyAdvance => ("#2a0f3d", "#7c2ae8"),
        ArcadeMame | NeoGeo | MegaDrive => ("#0f3d17", "#00b359"),
        PowerPCMac | Amiga | SunOSSPARC => ("#3d240f", "#ff9100"),
        SegaSaturn | Dreamcast | PlayStation1 | PlayStation2 => ("#3d0f24", "#e8478f"),
        NativeSystem => ("#1a2333", "#2d3748"),
    }
}

fn cover_svg(title: &str, platform: &spore::TargetPlatform) -> String {
    let (c1, c2) = platform_gradient(platform);
    let safe = escape_xml(title);
    let line1 = safe.chars().take(22).collect::<String>();
    let line2: String = safe.chars().skip(22).take(22).collect();
    let mut lines = String::new();
    if !line1.is_empty() {
        lines.push_str(&format!(
            r#"<text x="300" y="520" font-family="monospace" font-size="30" font-weight="bold" text-anchor="middle" fill="rgba(255,255,255,0.92)" letter-spacing="2">{line1}</text>"#
        ));
    }
    if !line2.is_empty() {
        lines.push_str(&format!(
            r#"<text x="300" y="560" font-family="monospace" font-size="30" font-weight="bold" text-anchor="middle" fill="rgba(255,255,255,0.92)" letter-spacing="2">{line2}</text>"#
        ));
    }
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="600" height="800" viewBox="0 0 600 800">
<defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="{c1}"/><stop offset="1" stop-color="{c2}"/></linearGradient></defs>
<rect width="600" height="800" fill="url(#g)"/>
<rect x="24" y="24" width="552" height="752" fill="none" stroke="rgba(255,255,255,0.28)" stroke-width="6" rx="20"/>
<text x="300" y="400" font-size="230" text-anchor="middle">🎮</text>
{lines}
<text x="300" y="640" font-family="monospace" font-size="26" text-anchor="middle" fill="rgba(255,255,255,0.55)" letter-spacing="3">MYCELIUM RETRO STORE</text>
</svg>"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir() -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("mycelium-store-test-{}", nanos));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_store_catalog_creation() {
        let home = tmp_dir();
        let catalog = StoreCatalog::open(&home).unwrap();
        assert!(!catalog.list_spores().is_empty());
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn test_catalog_seed_only_has_redistributable_spores() {
        let home = tmp_dir();
        let catalog = StoreCatalog::open(&home).unwrap();

        // O seed público deve conter apenas conteúdo com licença redistribuível
        // (Shareware/Freeware/OpenSource/PublicDomain). Nenhum spore copyright (BYOR).
        let all = catalog.list_spores();
        assert!(!all.is_empty(), "catálogo seed não pode estar vazio");
        for spore in &all {
            assert!(
                spore.license.is_publicly_redistributable(),
                "spore '{}' ({}) é copyright e não pode estar no seed público",
                spore.id,
                spore.title
            );
        }

        let public = catalog.list_public_spores();
        assert_eq!(public.len(), all.len(), "seed deve ser 100% redistribuível");

        std::fs::remove_dir_all(&home).ok();
    }
}
