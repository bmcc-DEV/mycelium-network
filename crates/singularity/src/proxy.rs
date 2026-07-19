//! Event Horizon HTTP — reverse proxy por gravidade + rate-limit básico.

use crate::{HorizonTable, SingularityError};
use axum::body::Body;
use axum::extract::{ConnectInfo, Request, State};
use axum::http::{header, StatusCode, Uri};
use axum::middleware::{from_fn, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::any;
use axum::Router;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
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

/// Janela e teto do rate-limit por IP (requests).
const RATE_WINDOW: Duration = Duration::from_secs(60);
const RATE_MAX: u32 = 120;

fn rate_table() -> &'static Mutex<HashMap<IpAddr, (Instant, u32)>> {
    static TABLE: OnceLock<Mutex<HashMap<IpAddr, (Instant, u32)>>> = OnceLock::new();
    TABLE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn allow_ip(ip: IpAddr) -> bool {
    let mut guard = match rate_table().lock() {
        Ok(g) => g,
        Err(_) => return true,
    };
    let now = Instant::now();
    let entry = guard.entry(ip).or_insert((now, 0));
    if now.duration_since(entry.0) > RATE_WINDOW {
        *entry = (now, 1);
        return true;
    }
    if entry.1 >= RATE_MAX {
        return false;
    }
    entry.1 += 1;
    true
}

async fn rate_gate(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: Request,
    next: Next,
) -> Response {
    if !allow_ip(addr.ip()) {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            "rate limit: tente de novo em até 60s",
        )
            .into_response();
    }
    next.run(req).await
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
        .route("/console", any(console))
        .route("/health", any(health))
        .route("/{*path}", any(proxy))
        .layer(from_fn(rate_gate))
        .with_state(table);

    let (tx, rx) = oneshot::channel::<()>();
    tokio::spawn(async move {
        let server = axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(async {
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
        "console": "/console",
    });
    (StatusCode::OK, [(header::CONTENT_TYPE, "application/json")], body.to_string())
}

/// UI mínima do Event Horizon — lista ions e links.
async fn console(State(table): State<HorizonTable>) -> impl IntoResponse {
    let ions = {
        let t = table.read().unwrap();
        t.ion_upstreams()
    };
    let mut items = String::new();
    if ions.is_empty() {
        items.push_str("<li><em>nenhum ion em órbita</em></li>");
    } else {
        for (name, upstream) in &ions {
            items.push_str(&format!(
                "<li><a href=\"/{name}/\">{name}</a> \
                 <span style=\"opacity:.6\">→ {upstream}</span> \
                 · <a href=\"/{name}/index.html\">html</a></li>"
            ));
        }
    }
    let html = format!(
        r#"<!doctype html>
<html lang="pt-BR">
<head>
<meta charset="utf-8"/>
<meta name="viewport" content="width=device-width,initial-scale=1"/>
<title>Mycelium — Event Horizon</title>
<style>
  :root {{ --bg:#0d1f17; --fg:#d7f5e3; --accent:#3d8f6a; --muted:#7aa892; }}
  body {{ margin:0; min-height:100vh; font-family:"IBM Plex Sans",Segoe UI,sans-serif;
         background:radial-gradient(1200px 600px at 10% -10%,#1a3d2e,var(--bg));
         color:var(--fg); padding:2.5rem clamp(1rem,4vw,3rem); }}
  h1 {{ font-family:"IBM Plex Serif",Georgia,serif; font-weight:500; letter-spacing:-.02em;
       font-size:clamp(1.8rem,4vw,2.6rem); margin:0 0 .4rem; }}
  p {{ color:var(--muted); max-width:36rem; line-height:1.5; }}
  ul {{ list-style:none; padding:0; margin:2rem 0; }}
  li {{ padding:.85rem 0; border-bottom:1px solid rgba(125,180,150,.2); }}
  a {{ color:var(--accent); text-decoration:none; font-weight:600; }}
  a:hover {{ text-decoration:underline; }}
  .meta {{ font-size:.85rem; color:var(--muted); margin-top:2rem; }}
</style>
</head>
<body>
  <h1>Event Horizon</h1>
  <p>Console do Singularity — ions em órbita neste nó. Cada link passa pelo rizomorfo até a Vacuum Chamber.</p>
  <ul>{items}</ul>
  <p class="meta"><a href="/">JSON</a> · <a href="/health">health</a></p>
</body>
</html>"#
    );
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        html,
    )
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
