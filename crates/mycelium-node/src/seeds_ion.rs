//! # Seeds Ion — Catálogo de seeds públicas/privadas via HTTP
//!
//! Expõe o catálogo de seeds (`{home}/seeds/catalog.json`) como ion `seeds`
//! no Singularity Event Horizon:
//! - `GET /`           → UI (seeds.html)
//! - `GET /api`        → catálogo completo (JSON)
//! - `GET /api/public` → só seeds públicas (endpoint de bootstrap soberano)
//! - `GET /api/private`→ só seeds privadas
//! - `POST /api/add`   → adiciona seed (JSON SeedEntry)
//! - `POST /api/remove/{id}` → remove seed

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use mycelium_hyphae::{SeedCatalog, SeedEntry, SeedVisibility};
use serde_json::json;
use std::path::{Path as FsPath, PathBuf};

#[derive(Clone)]
pub struct SeedsState {
    pub home: PathBuf,
}

pub fn create_seeds_router(home: impl AsRef<FsPath>) -> Router {
    Router::new()
        .route("/", get(ui))
        .route("/api", get(catalog_all))
        .route("/api/public", get(catalog_public))
        .route("/api/private", get(catalog_private))
        .route("/api/add", post(add))
        .route("/api/remove/{id}", post(remove))
        .with_state(SeedsState {
            home: home.as_ref().to_path_buf(),
        })
}

async fn ui() -> impl IntoResponse {
    let default_ui = include_str!("../../../deploy/store-ui/seeds.html");
    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
        default_ui.to_string(),
    )
}

async fn catalog_all(State(state): State<SeedsState>) -> impl IntoResponse {
    match SeedCatalog::open(&state.home) {
        Ok(catalog) => (
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "application/json; charset=utf-8")],
            serde_json::to_string_pretty(&catalog).unwrap_or_else(|_| "{}".into()),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            [(axum::http::header::CONTENT_TYPE, "application/json; charset=utf-8")],
            json!({ "error": e }).to_string(),
        ),
    }
}

async fn catalog_visibility(State(state): State<SeedsState>, visibility: SeedVisibility) -> impl IntoResponse {
    match SeedCatalog::open(&state.home) {
        Ok(catalog) => {
            let entries: Vec<&SeedEntry> = catalog.list(Some(visibility));
            let body = serde_json::to_vec_pretty(&entries).unwrap_or_else(|_| b"[]".to_vec());
            (
                StatusCode::OK,
                [
                    (axum::http::header::CONTENT_TYPE, "application/json; charset=utf-8"),
                    (axum::http::header::CACHE_CONTROL, "public, max-age=60"),
                ],
                body,
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            [
                (axum::http::header::CONTENT_TYPE, "application/json; charset=utf-8"),
                (axum::http::header::CACHE_CONTROL, "no-store"),
            ],
            json!({ "error": e }).to_string().into_bytes(),
        ),
    }
}

async fn catalog_public(State(state): State<SeedsState>) -> impl IntoResponse {
    catalog_visibility(State(state), SeedVisibility::Public).await
}

async fn catalog_private(State(state): State<SeedsState>) -> impl IntoResponse {
    catalog_visibility(State(state), SeedVisibility::Private).await
}

async fn add(State(state): State<SeedsState>, Json(entry): Json<SeedEntry>) -> impl IntoResponse {
    let mut catalog = match SeedCatalog::open(&state.home) {
        Ok(c) => c,
        Err(e) => return json_err(StatusCode::INTERNAL_SERVER_ERROR, e),
    };
    match catalog.add(entry) {
        Ok(()) => match catalog.save(&state.home) {
            Ok(()) => (
                StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, "application/json; charset=utf-8")],
                json!({ "ok": true, "seeds": catalog.seeds.len() }).to_string(),
            ),
            Err(e) => json_err(StatusCode::INTERNAL_SERVER_ERROR, e),
        },
        Err(e) => json_err(StatusCode::BAD_REQUEST, e),
    }
}

async fn remove(State(state): State<SeedsState>, Path(id): Path<String>) -> impl IntoResponse {
    let mut catalog = match SeedCatalog::open(&state.home) {
        Ok(c) => c,
        Err(e) => return json_err(StatusCode::INTERNAL_SERVER_ERROR, e),
    };
    if catalog.remove(&id) {
        match catalog.save(&state.home) {
            Ok(()) => (
                StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, "application/json; charset=utf-8")],
                json!({ "ok": true, "removed": id }).to_string(),
            ),
            Err(e) => json_err(StatusCode::INTERNAL_SERVER_ERROR, e),
        }
    } else {
        json_err(StatusCode::NOT_FOUND, format!("seed '{id}' não encontrada"))
    }
}

fn json_err(status: StatusCode, message: String) -> (StatusCode, [(axum::http::header::HeaderName, &'static str); 1], String) {
    (
        status,
        [(axum::http::header::CONTENT_TYPE, "application/json; charset=utf-8")],
        json!({ "error": message }).to_string(),
    )
}
