//! Event Horizon HTTP — reverse proxy por gravidade.

use crate::{HorizonTable, SingularityError};
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::any;
use axum::Router;
use std::net::SocketAddr;
use tokio::sync::oneshot;

/// Handle para encerrar o horizon.
pub struct HorizonHandle {
    pub bind: SocketAddr,
    shutdown: Option<oneshot::Sender<()>>,
}

impl HorizonHandle {
    pub fn shutdown(mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
    }
}

impl Drop for HorizonHandle {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
    }
}

/// Sobe o Event Horizon em `bind` (ex.: `127.0.0.1:7474`).
pub async fn serve_horizon(
    bind: SocketAddr,
    table: HorizonTable,
) -> Result<HorizonHandle, String> {
    let listener = tokio::net::TcpListener::bind(bind)
        .await
        .map_err(|e| e.to_string())?;
    let local = listener.local_addr().map_err(|e| e.to_string())?;

    let app = Router::new()
        .route("/", any(root))
        .route("/health", any(health))
        .route("/{*path}", any(proxy))
        .with_state(table);

    let (tx, rx) = oneshot::channel::<()>();
    tokio::spawn(async move {
        let server = axum::serve(listener, app).with_graceful_shutdown(async {
            let _ = rx.await;
        });
        if let Err(e) = server.await {
            tracing::error!("event horizon: {e}");
        }
    });

    tracing::info!(%local, "event horizon aberto");
    Ok(HorizonHandle {
        bind: local,
        shutdown: Some(tx),
    })
}

async fn health() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

async fn root(State(table): State<HorizonTable>) -> impl IntoResponse {
    let (ions, hosts) = {
        let t = table.read().unwrap();
        (
            t.ion_upstreams(),
            t.hosts().cloned().collect::<Vec<_>>(),
        )
    };
    let body = serde_json::json!({
        "service": "mycelium-singularity",
        "role": "event-horizon",
        "hosts": hosts,
        "ions": ions.iter().map(|(n, u)| serde_json::json!({"ion": n, "upstream": u})).collect::<Vec<_>>(),
        "hint": "GET /{ion}/  — rizomorfo proxya até a Chamber",
    });
    (StatusCode::OK, [(header::CONTENT_TYPE, "application/json")], body.to_string())
}

async fn proxy(State(table): State<HorizonTable>, req: Request) -> Response {
    let path = req.uri().path().to_string();
    let ion = path
        .trim_start_matches('/')
        .split('/')
        .next()
        .unwrap_or("")
        .to_string();

    if ion.is_empty() || ion == "health" {
        return StatusCode::NOT_FOUND.into_response();
    }

    let upstream = {
        let t = table.read().unwrap();
        match t.route_ion(&ion) {
            Ok(orbit) if !orbit.upstream.is_empty() => orbit.upstream.clone(),
            Ok(_) => {
                return (
                    StatusCode::BAD_GATEWAY,
                    format!("ion `{ion}` sem upstream (chamber morta?)"),
                )
                    .into_response();
            }
            Err(SingularityError::NoOrbit(_)) => {
                return (StatusCode::NOT_FOUND, format!("nenhum ion `{ion}` no horizonte"))
                    .into_response();
            }
            Err(e) => {
                return (StatusCode::BAD_GATEWAY, e.to_string()).into_response();
            }
        }
    };

    // Reescreve /{ion}/foo → /foo no upstream.
    let rest = {
        let stripped = path
            .strip_prefix(&format!("/{ion}"))
            .unwrap_or(&path);
        if stripped.is_empty() {
            "/".to_string()
        } else {
            stripped.to_string()
        }
    };
    let query = req
        .uri()
        .query()
        .map(|q| format!("?{q}"))
        .unwrap_or_default();
    let target = format!("{upstream}{rest}{query}");

    let client = reqwest::Client::new();
    let method = reqwest::Method::from_bytes(req.method().as_str().as_bytes())
        .unwrap_or(reqwest::Method::GET);

    let (parts, body) = req.into_parts();
    let body_bytes = match axum::body::to_bytes(body, 2 * 1024 * 1024).await {
        Ok(b) => b,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, format!("body: {e}")).into_response();
        }
    };

    let mut builder = client.request(method, &target);
    for (name, value) in parts.headers.iter() {
        if name == header::HOST || name == header::CONNECTION {
            continue;
        }
        if let Ok(v) = value.to_str() {
            builder = builder.header(name.as_str(), v);
        }
    }

    match builder.body(body_bytes).send().await {
        Ok(upstream_resp) => {
            let status =
                StatusCode::from_u16(upstream_resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
            let mut response = Response::builder().status(status);
            for (name, value) in upstream_resp.headers().iter() {
                if name == header::TRANSFER_ENCODING || name == header::CONNECTION {
                    continue;
                }
                response = response.header(name.as_str(), value.as_bytes());
            }
            let bytes = upstream_resp.bytes().await.unwrap_or_default();
            response
                .body(Body::from(bytes))
                .unwrap_or_else(|_| StatusCode::BAD_GATEWAY.into_response())
        }
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            format!("rizomorfo falhou ao alcançar {target}: {e}"),
        )
            .into_response(),
    }
}

#[allow(dead_code)]
fn _uri_ok(u: &str) -> bool {
    u.parse::<Uri>().is_ok()
}
