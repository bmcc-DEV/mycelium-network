//! Plano de controle local: Unix socket + JSON linha-a-linha.
//!
//! Se `MYCELIUM_CONTROL_TOKEN` estiver definido no daemon, cada pedido
//! deve incluir `"auth": "<token>"` no JSON.

use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{mpsc, oneshot};

/// Pedidos da CLI ao daemon.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Request {
    Status,
    Sow {
        message: String,
        path: String,
        content: String,
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
}

/// Mensagem interna: pedido + canal de resposta.
pub struct ControlMsg {
    pub request: Request,
    pub reply: oneshot::Sender<Response>,
}

/// Serve o socket Unix; encaminha pedidos pelo canal.
pub async fn serve(
    sock_path: impl AsRef<Path>,
    tx: mpsc::Sender<ControlMsg>,
    required_token: Option<String>,
) -> Result<(), String> {
    let path = sock_path.as_ref();
    let _ = std::fs::remove_file(path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let listener = UnixListener::bind(path).map_err(|e| e.to_string())?;
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
            if let Err(e) = handle_client(stream, tx, token).await {
                tracing::warn!("control client error: {e}");
            }
        });
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

async fn handle_client(
    stream: UnixStream,
    tx: mpsc::Sender<ControlMsg>,
    required_token: Option<String>,
) -> Result<(), String> {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();
    while let Some(line) = lines.next_line().await.map_err(|e| e.to_string())? {
        if line.trim().is_empty() {
            continue;
        }
        let request = match parse_request_line(&line, required_token.as_deref()) {
            Ok(r) => r,
            Err(message) => {
                write_response(&mut writer, &Response::Err { message }).await?;
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
            write_response(&mut writer, &resp).await?;
            break;
        }
        let resp = reply_rx.await.unwrap_or(Response::Err {
            message: "sem resposta do organismo".into(),
        });
        write_response(&mut writer, &resp).await?;
    }
    Ok(())
}

async fn write_response(
    writer: &mut tokio::net::unix::OwnedWriteHalf,
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

/// Cliente: envia um pedido ao daemon e devolve a resposta.
pub async fn call(sock_path: impl AsRef<Path>, request: Request) -> Result<Response, String> {
    let stream = UnixStream::connect(sock_path.as_ref())
        .await
        .map_err(|e| {
            format!(
                "daemon não está rodando ({}): {e}",
                sock_path.as_ref().display()
            )
        })?;
    let (reader, mut writer) = stream.into_split();
    let mut value = serde_json::to_value(&request).map_err(|e| e.to_string())?;
    if let Ok(token) = std::env::var("MYCELIUM_CONTROL_TOKEN") {
        if !token.is_empty() {
            if let Some(obj) = value.as_object_mut() {
                obj.insert("auth".into(), serde_json::Value::String(token));
            }
        }
    }
    let mut line = serde_json::to_string(&value).map_err(|e| e.to_string())?;
    line.push('\n');
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
