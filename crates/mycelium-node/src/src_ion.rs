//! # Src Ion — Browser de código soberano (repos do SporeBank via HTTP)
//!
//! Serve as árvores de código publicadas como Plots multi-leaf no SporeBank,
//! expostas como ion `src` no Singularity Event Horizon — um "GitHub" próprio,
//! content-addressed (blake3) e servido pelo próprio nó.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use giggs::Plot;
use serde_json::Value;
use std::path::{Path as FsPath, PathBuf};

#[derive(Clone)]
pub struct SrcState {
    pub home: PathBuf,
}

pub fn create_src_router(home: impl AsRef<FsPath>) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/{cid}", get(list))
        .route("/{cid}/{*path}", get(file))
        .with_state(SrcState {
            home: home.as_ref().to_path_buf(),
        })
}

fn plots_dir(home: &FsPath) -> PathBuf {
    home.join("sporebank").join("plots")
}

fn read_plot(home: &FsPath, cid: &str) -> Option<Plot> {
    let path = plots_dir(home).join(format!("{cid}.json"));
    let bytes = std::fs::read(&path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn cids(home: &FsPath) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(plots_dir(home))
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    name.strip_suffix(".json").map(|s| s.to_string())
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

async fn index(State(state): State<SrcState>) -> impl IntoResponse {
    let entries: Vec<Value> = cids(&state.home)
        .iter()
        .filter_map(|cid| {
            let plot = read_plot(&state.home, cid)?;
            Some(serde_json::json!({
                "cid": cid,
                "message": plot.message,
                "leaves": plot.leaves.len(),
                "author": plot.author.to_string(),
                "url": format!("/src/{cid}"),
            }))
        })
        .collect();
    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/json; charset=utf-8")],
        serde_json::json!({ "ion": "src", "substrate": "mycelium", "repos": entries }).to_string(),
    )
}

async fn list(State(state): State<SrcState>, Path(cid): Path<String>) -> impl IntoResponse {
    match read_plot(&state.home, &cid) {
        Some(plot) => {
            let files: Vec<Value> = plot
                .leaves
                .iter()
                .map(|leaf| {
                    serde_json::json!({
                        "path": leaf.path,
                        "size": leaf.content.len(),
                        "url": format!("/src/{cid}/{}", leaf.path),
                    })
                })
                .collect();
            (
                StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, "application/json; charset=utf-8")],
                serde_json::json!({
                    "cid": cid,
                    "message": plot.message,
                    "files": files,
                })
                .to_string(),
            )
        }
        None => (
            StatusCode::NOT_FOUND,
            [(axum::http::header::CONTENT_TYPE, "application/json; charset=utf-8")],
            serde_json::json!({ "error": format!("repo {cid} não encontrado no SporeBank local") })
                .to_string(),
        ),
    }
}

async fn file(
    State(state): State<SrcState>,
    Path((cid, path)): Path<(String, String)>,
) -> impl IntoResponse {
    match read_plot(&state.home, &cid) {
        Some(plot) => {
            match plot
                .leaves
                .iter()
                .find(|leaf| leaf.path == path)
            {
                Some(leaf) => {
                    let mime = mime_for(&path);
                    (
                        StatusCode::OK,
                        [
                            (axum::http::header::CONTENT_TYPE, mime),
                            (axum::http::header::CACHE_CONTROL, "public, max-age=31536000"),
                        ],
                        leaf.content.clone(),
                    )
                }
                None => (
                    StatusCode::NOT_FOUND,
                    [
                        (axum::http::header::CONTENT_TYPE, "text/plain; charset=utf-8"),
                        (axum::http::header::CACHE_CONTROL, "no-store"),
                    ],
                    format!("arquivo não encontrado no repo: {path}").into_bytes(),
                ),
            }
        }
        None => (
            StatusCode::NOT_FOUND,
            [
                (axum::http::header::CONTENT_TYPE, "text/plain; charset=utf-8"),
                (axum::http::header::CACHE_CONTROL, "no-store"),
            ],
            format!("repo {cid} não encontrado no SporeBank local").into_bytes(),
        ),
    }
}

fn mime_for(path: &str) -> &'static str {
    let lower = path.to_lowercase();
    if lower.ends_with(".html") {
        "text/html; charset=utf-8"
    } else if lower.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if lower.ends_with(".js") || lower.ends_with(".mjs") {
        "application/javascript; charset=utf-8"
    } else if lower.ends_with(".json") {
        "application/json; charset=utf-8"
    } else if lower.ends_with(".rs") || lower.ends_with(".toml") || lower.ends_with(".md")
        || lower.ends_with(".sh") || lower.ends_with(".svg") || lower.ends_with(".txt")
        || lower.ends_with(".yaml") || lower.ends_with(".yml") {
        "text/plain; charset=utf-8"
    } else {
        "application/octet-stream"
    }
}
