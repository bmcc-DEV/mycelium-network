//! Plano de controle local: Unix socket + JSON linha-a-linha.
//!
//! Se `MYCELIUM_CONTROL_TOKEN` estiver definido no daemon, cada pedido
//! deve incluir `"auth": "<token>"` no JSON.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream, UnixListener, UnixStream};
use tokio::sync::{mpsc, oneshot};

/// Porta TCP de controlo (fallback Android / sem Unix socket).
fn tcp_port_path(sock_path: &Path) -> PathBuf {
    sock_path.with_extension("tcp")
}

/// Pedidos da CLI ao daemon.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Request {
    Status,
    Sow {
        message: String,
        path: String,
        content: String,
        /// QEL k,n (ex. "3,7"). Requer feature `nostr`.
        #[serde(default)]
        qel: Option<String>,
        #[serde(default)]
        nostr: bool,
        #[serde(default)]
        ghost: bool,
        #[serde(default)]
        recipient: Option<String>,
    },
    Signal {
        plot: String,
        quorum: usize,
        ion: String,
        name: String,
    },
    Resonate {
        signal: String,
    },
    Recall {
        plot: String,
        #[serde(default)]
        qel: bool,
        #[serde(default)]
        nostr: bool,
        #[serde(default)]
        qel_threshold: Option<u8>,
    },
    Bootstrap {
        addr: String,
    },
    /// Escreve um átomo no Isotope e propaga via hifas.
    IsotopePut {
        key: String,
        value: String,
        #[serde(default)]
        clock: Option<u64>,
    },
    /// Lê um átomo do Nucleus Isotope local.
    IsotopeGet {
        key: String,
    },
    Shutdown,
}

/// Respostas do daemon.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Response {
    Ok { message: String },
    Status(Box<StatusReport>),
    Err { message: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StatusReport {
    pub node_id: String,
    pub peer_id: String,
    pub listen_addrs: Vec<String>,
    pub neighbors: usize,
    pub plots: usize,
    pub signals: usize,
    pub ions: Vec<String>,
    pub atp: u64,
    pub enzymes: u64,
    pub mycelia: u64,
    pub spores: u64,
    pub resilience: u64,
    pub anastomoses: u64,
    pub atrophies: u64,
    pub messages_in: u64,
    pub messages_out: u64,
    pub home: String,
    /// URL do Event Horizon HTTP (Singularity).
    #[serde(default)]
    pub event_horizon: String,
    /// Endpoints vivos das Chambers (Vacuum).
    #[serde(default)]
    pub ion_endpoints: Vec<String>,
    /// Átomos no Nucleus Isotope local.
    #[serde(default)]
    pub isotope_atoms: usize,
    /// Índice do shard Isotope deste nó.
    #[serde(default)]
    pub isotope_shard: u32,
    /// Tamanho do anel Isotope.
    #[serde(default)]
    pub isotope_ring: u32,
    /// Membrana fisiológica: floresta | raiz | folha | esporocarp.
    #[serde(default)]
    pub membrane: String,
    /// Volunteer Sporocarp (relay comunitário).
    #[serde(default)]
    pub sporocarp: bool,
    /// Nome DNS TXT do Spore Bank em uso (se configurado).
    #[serde(default)]
    pub dns_seed: Option<String>,
    /// Inbound WAN declarado alcançável (`MYCELIUM_REACHABLE` / `--assume-reachable`).
    #[serde(default)]
    pub wan_reachable: bool,
    /// Este nó opera como circuit relay (esporocarp).
    #[serde(default)]
    pub is_relay: bool,
    /// PeerId do relay activo (folha em circuito), se houver.
    #[serde(default)]
    pub active_relay: Option<String>,
    /// Saúde do catálogo de relays mesh: healthy|degraded|none|self.
    #[serde(default)]
    pub relay_health: String,
    /// Fase Physarum (exploratory|transport|dormant).
    #[serde(default)]
    pub physarum_phase: String,
}

/// Mensagem interna: pedido + canal de resposta.
pub struct ControlMsg {
    pub request: Request,
    pub reply: oneshot::Sender<Response>,
}

/// Serve o plano de controlo (Unix socket, ou TCP 127.0.0.1 se Unix for bloqueado — Android shell).
pub async fn serve(
    sock_path: impl AsRef<Path>,
    tx: mpsc::Sender<ControlMsg>,
    required_token: Option<String>,
) -> Result<(), String> {
    let path = sock_path.as_ref();
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(tcp_port_path(path));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    match UnixListener::bind(path) {
        Ok(listener) => {
            if required_token.is_some() {
                tracing::info!(path = %path.display(), "control socket listening (auth obrigatória)");
            } else {
                tracing::info!(path = %path.display(), "control socket listening");
            }
            loop {
                let (stream, _) = listener.accept().await.map_err(|e| e.to_string())?;
                let tx = tx.clone();
                let token = required_token.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_client_unix(stream, tx, token).await {
                        tracing::warn!("control client error: {e}");
                    }
                });
            }
        }
        Err(e) => {
            tracing::warn!(
                path = %path.display(),
                err = %e,
                "Unix control socket indisponível — fallback TCP 127.0.0.1"
            );
            let listener = TcpListener::bind("127.0.0.1:0")
                .await
                .map_err(|e| e.to_string())?;
            let port = listener.local_addr().map_err(|e| e.to_string())?.port();
            let port_file = tcp_port_path(path);
            std::fs::write(&port_file, format!("{port}\n")).map_err(|e| e.to_string())?;
            if required_token.is_some() {
                tracing::info!(%port, "control TCP listening (auth obrigatória)");
            } else {
                tracing::info!(%port, "control TCP listening");
            }
            loop {
                let (stream, _) = listener.accept().await.map_err(|e| e.to_string())?;
                let tx = tx.clone();
                let token = required_token.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_client_tcp(stream, tx, token).await {
                        tracing::warn!("control client error: {e}");
                    }
                });
            }
        }
    }
}

fn parse_request_line(line: &str, required: Option<&str>) -> Result<Request, String> {
    let mut value: serde_json::Value =
        serde_json::from_str(line).map_err(|e| format!("pedido inválido: {e}"))?;
    if let Some(exp) = required {
        let got = value
            .get("auth")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if got != exp {
            return Err("auth inválida ou ausente (defina MYCELIUM_CONTROL_TOKEN)".into());
        }
    }
    if let Some(obj) = value.as_object_mut() {
        obj.remove("auth");
    }
    serde_json::from_value(value).map_err(|e| format!("pedido inválido: {e}"))
}

async fn write_response<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    resp: &Response,
) -> Result<(), String> {
    let mut line = serde_json::to_string(resp).map_err(|e| e.to_string())?;
    line.push('\n');
    writer
        .write_all(line.as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

async fn handle_client_unix(
    stream: UnixStream,
    tx: mpsc::Sender<ControlMsg>,
    required_token: Option<String>,
) -> Result<(), String> {
    let (reader, mut writer) = stream.into_split();
    handle_client_lines(reader, &mut writer, tx, required_token).await
}

async fn handle_client_tcp(
    stream: TcpStream,
    tx: mpsc::Sender<ControlMsg>,
    required_token: Option<String>,
) -> Result<(), String> {
    let (reader, mut writer) = stream.into_split();
    handle_client_lines(reader, &mut writer, tx, required_token).await
}

async fn handle_client_lines<R, W>(
    reader: R,
    writer: &mut W,
    tx: mpsc::Sender<ControlMsg>,
    required_token: Option<String>,
) -> Result<(), String>
where
    R: tokio::io::AsyncRead + Unpin,
    W: AsyncWriteExt + Unpin,
{
    let mut lines = BufReader::new(reader).lines();
    while let Some(line) = lines.next_line().await.map_err(|e| e.to_string())? {
        if line.trim().is_empty() {
            continue;
        }
        let request = match parse_request_line(&line, required_token.as_deref()) {
            Ok(r) => r,
            Err(message) => {
                write_response(writer, &Response::Err { message }).await?;
                continue;
            }
        };
        let (reply_tx, reply_rx) = oneshot::channel();
        if tx
            .send(ControlMsg {
                request,
                reply: reply_tx,
            })
            .await
            .is_err()
        {
            let resp = Response::Err {
                message: "daemon encerrado".into(),
            };
            write_response(writer, &resp).await?;
            break;
        }
        let resp = reply_rx.await.unwrap_or(Response::Err {
            message: "sem resposta do organismo".into(),
        });
        write_response(writer, &resp).await?;
    }
    Ok(())
}

/// Resolve o token de controlo: env `MYCELIUM_CONTROL_TOKEN`, senão `{home}/control.token`.
fn resolve_control_token(sock_path: &Path) -> Option<String> {
    if let Ok(token) = std::env::var("MYCELIUM_CONTROL_TOKEN") {
        let t = token.trim();
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }
    let token_file = sock_path
        .parent()
        .unwrap_or(sock_path)
        .join("control.token");
    std::fs::read_to_string(&token_file)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Cliente: envia um pedido ao daemon e devolve a resposta.
pub async fn call(sock_path: impl AsRef<Path>, request: Request) -> Result<Response, String> {
    let path = sock_path.as_ref();
    let mut value = serde_json::to_value(&request).map_err(|e| e.to_string())?;
    if let Some(token) = resolve_control_token(path) {
        if let Some(obj) = value.as_object_mut() {
            obj.insert("auth".into(), serde_json::Value::String(token));
        }
    }
    let mut line = serde_json::to_string(&value).map_err(|e| e.to_string())?;
    line.push('\n');

    if let Ok(stream) = UnixStream::connect(path).await {
        return exchange_line(stream, &line).await;
    }

    let port_file = tcp_port_path(path);
    let port: u16 = std::fs::read_to_string(&port_file)
        .map_err(|e| {
            format!(
                "daemon não está rodando ({} / {}): {e}",
                path.display(),
                port_file.display()
            )
        })?
        .trim()
        .parse()
        .map_err(|e| format!("porta de controlo inválida: {e}"))?;
    let stream = TcpStream::connect(("127.0.0.1", port))
        .await
        .map_err(|e| format!("daemon não está rodando (127.0.0.1:{port}): {e}"))?;
    exchange_line(stream, &line).await
}

async fn exchange_line<S>(stream: S, line: &str) -> Result<Response, String>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let (reader, mut writer) = tokio::io::split(stream);
    writer
        .write_all(line.as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    let mut lines = BufReader::new(reader).lines();
    let resp_line = lines
        .next_line()
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "daemon fechou a conexão sem responder".to_string())?;
    serde_json::from_str(&resp_line).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_required_rejects_missing() {
        let err = parse_request_line(r#"{"cmd":"status"}"#, Some("secret")).unwrap_err();
        assert!(err.contains("auth"));
    }

    #[test]
    fn auth_ok_strips_field() {
        let req = parse_request_line(r#"{"auth":"secret","cmd":"status"}"#, Some("secret")).unwrap();
        assert!(matches!(req, Request::Status));
    }
}
