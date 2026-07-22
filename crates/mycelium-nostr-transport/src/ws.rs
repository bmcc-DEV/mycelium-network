//! Sessão WebSocket persistente a um relay Nostr.

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::{broadcast, mpsc, Mutex};
use tokio::time::{timeout, Duration};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use mycelium_nostr::NostrEvent;

#[derive(Debug, Error)]
pub enum WsError {
    #[error("websocket: {0}")]
    Ws(String),
    #[error("timeout")]
    Timeout,
    #[error("fechado")]
    Closed,
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

type SharedSubs = Arc<Mutex<HashMap<String, mpsc::UnboundedSender<NostrEvent>>>>;

/// Ligação WSS contínua: publish + múltiplas subscrições.
#[derive(Clone)]
pub struct PersistentRelay {
    out_tx: mpsc::UnboundedSender<String>,
    events: broadcast::Sender<NostrEvent>,
    subs: SharedSubs,
    url: String,
}

impl PersistentRelay {
    pub async fn connect(url: &str) -> Result<Self, WsError> {
        let (ws, _) = connect_async(url)
            .await
            .map_err(|e| WsError::Ws(e.to_string()))?;
        let (mut sink, mut stream) = ws.split();
        let (out_tx, mut out_rx) = mpsc::unbounded_channel::<String>();
        let (events, _) = broadcast::channel(256);
        let subs: SharedSubs = Arc::new(Mutex::new(HashMap::new()));

        let events_w = events.clone();
        let subs_w = Arc::clone(&subs);
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    msg = out_rx.recv() => {
                        match msg {
                            Some(payload) => {
                                if sink.send(Message::Text(payload.into())).await.is_err() {
                                    break;
                                }
                            }
                            None => break,
                        }
                    }
                    msg = stream.next() => {
                        match msg {
                            Some(Ok(Message::Text(t))) => {
                                if let Ok(Value::Array(arr)) = serde_json::from_str::<Value>(&t) {
                                    if arr.first().and_then(|v| v.as_str()) == Some("EVENT")
                                        && arr.len() >= 3
                                    {
                                        if let Ok(ev) =
                                            serde_json::from_value::<NostrEvent>(arr[2].clone())
                                        {
                                            let _ = events_w.send(ev.clone());
                                            let sub_id = arr.get(1).and_then(|v| v.as_str()).unwrap_or("");
                                            let guard = subs_w.lock().await;
                                            if let Some(tx) = guard.get(sub_id) {
                                                let _ = tx.send(ev);
                                            }
                                        }
                                    }
                                }
                            }
                            Some(Ok(Message::Ping(p))) => {
                                let _ = sink.send(Message::Pong(p)).await;
                            }
                            Some(Ok(_)) => {}
                            Some(Err(_)) | None => break,
                        }
                    }
                }
            }
        });

        Ok(Self {
            out_tx,
            events,
            subs,
            url: url.to_string(),
        })
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn publish(&self, event: &NostrEvent) -> Result<(), WsError> {
        let msg = json!(["EVENT", event]).to_string();
        self.out_tx.send(msg).map_err(|_| WsError::Closed)
    }

    /// Subscreve com filtro; devolve receiver de eventos + sub_id.
    pub async fn subscribe(
        &self,
        filter: Value,
    ) -> Result<(String, mpsc::UnboundedReceiver<NostrEvent>), WsError> {
        let sub_id = format!("nt-{}", &hex::encode(rand_bytes())[..10]);
        let (tx, rx) = mpsc::unbounded_channel();
        self.subs.lock().await.insert(sub_id.clone(), tx);
        let req = json!(["REQ", sub_id, filter]).to_string();
        self.out_tx.send(req).map_err(|_| WsError::Closed)?;
        Ok((sub_id, rx))
    }

    pub fn subscribe_broadcast(&self) -> broadcast::Receiver<NostrEvent> {
        self.events.subscribe()
    }

    /// Espera um evento que satisfaz o predicado (timeout).
    pub async fn wait_event<F>(
        &self,
        mut pred: F,
        timeout_dur: Duration,
    ) -> Result<NostrEvent, WsError>
    where
        F: FnMut(&NostrEvent) -> bool,
    {
        let mut rx = self.events.subscribe();
        timeout(timeout_dur, async {
            loop {
                match rx.recv().await {
                    Ok(ev) if pred(&ev) => return Ok(ev),
                    Ok(_) => continue,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(_) => return Err(WsError::Closed),
                }
            }
        })
        .await
        .map_err(|_| WsError::Timeout)?
    }
}

fn rand_bytes() -> [u8; 8] {
    use rand::RngCore;
    let mut b = [0u8; 8];
    rand::rngs::OsRng.fill_bytes(&mut b);
    b
}
