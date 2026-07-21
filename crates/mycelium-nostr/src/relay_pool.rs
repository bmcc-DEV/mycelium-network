//! Pool de relays Nostr via WebSocket (tokio-tungstenite).

use crate::nip94::NostrEvent;
use crate::NostrError;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

/// Relays públicos estáveis (mailbox outbound).
/// `relay.nostr.band` omitido — timeouts frequentes (~6s/EVENT) sem ganho.
pub const PUBLIC_RELAYS: &[&str] = &[
    "wss://relay.damus.io",
    "wss://nos.lol",
    "wss://relay.snort.social",
    "wss://relay.primal.net",
];

/// Pool de relays.
pub struct RelayPool {
    relays: Vec<String>,
    timeout: Duration,
    min_relays: usize,
}

impl Default for RelayPool {
    fn default() -> Self {
        Self::default_public()
    }
}

impl RelayPool {
    pub fn new(relays: Vec<String>) -> Self {
        Self {
            relays,
            timeout: Duration::from_secs(4),
            min_relays: 1,
        }
    }

    pub fn default_public() -> Self {
        Self::new(PUBLIC_RELAYS.iter().map(|s| (*s).to_string()).collect())
            .with_timeout(Duration::from_secs(6))
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn with_min_relays(mut self, min: usize) -> Self {
        self.min_relays = min.max(1);
        self
    }

    pub fn relays(&self) -> &[String] {
        &self.relays
    }

    /// Publica em paralelo em todos os relays; exige `min_relays` sucessos.
    pub async fn publish(&self, event: &NostrEvent) -> Result<usize, NostrError> {
        let msg = json!(["EVENT", event]);
        let payload = Arc::new(msg.to_string());
        let timeout_dur = self.timeout;
        let min = self.min_relays;

        let mut handles = Vec::new();
        for url in &self.relays {
            let url = url.clone();
            let payload = Arc::clone(&payload);
            handles.push(tokio::spawn(async move {
                match Self::send_event_static(&url, &payload, timeout_dur).await {
                    Ok(()) => {
                        tracing::info!(relay = %url, "EVENT publicado");
                        true
                    }
                    Err(e) => {
                        tracing::warn!(relay = %url, error = %e, "relay falhou");
                        false
                    }
                }
            }));
        }

        let mut ok = 0usize;
        for h in handles {
            if let Ok(true) = h.await {
                ok += 1;
            }
        }

        if ok < min {
            Err(NostrError::AllRelaysFailed)
        } else {
            Ok(ok)
        }
    }

    async fn send_event_static(
        url: &str,
        payload: &str,
        overall: Duration,
    ) -> Result<(), NostrError> {
        let fut = async {
            let (mut ws, _) = connect_async(url)
                .await
                .map_err(|e| NostrError::WebSocket(e.to_string()))?;
            ws.send(Message::Text(payload.to_string().into()))
                .await
                .map_err(|e| NostrError::WebSocket(e.to_string()))?;

            // Espera OK breve (não bloqueia 3s)
            let _ = timeout(Duration::from_millis(800), async {
                while let Some(msg) = ws.next().await {
                    let msg = msg.map_err(|e| NostrError::WebSocket(e.to_string()))?;
                    if let Message::Text(t) = msg {
                        if t.contains("\"OK\"") || t.contains("\"NOTICE\"") {
                            break;
                        }
                    }
                }
                Ok::<(), NostrError>(())
            })
            .await;

            let _ = ws.close(None).await;
            Ok(())
        };
        timeout(overall, fut)
            .await
            .map_err(|_| NostrError::Timeout)?
    }

    /// Subscreve em paralelo nos relays.
    pub async fn subscribe(&self, filter: Value) -> Result<Vec<NostrEvent>, NostrError> {
        let mut handles = Vec::new();
        for url in &self.relays {
            let url = url.clone();
            let filter = filter.clone();
            let t = self.timeout;
            handles.push(tokio::spawn(async move {
                Self::subscribe_one_static(&url, filter, t).await
            }));
        }
        let mut collected = Vec::new();
        for h in handles {
            if let Ok(Ok(mut evs)) = h.await {
                collected.append(&mut evs);
            }
        }
        Ok(collected)
    }

    async fn subscribe_one_static(
        url: &str,
        filter: Value,
        overall: Duration,
    ) -> Result<Vec<NostrEvent>, NostrError> {
        let sub_id = format!("mycelium-{}", &hex::encode(rand_id())[..8]);
        let req = json!(["REQ", sub_id, filter]);
        let fut = async {
            let (mut ws, _) = connect_async(url)
                .await
                .map_err(|e| NostrError::WebSocket(e.to_string()))?;
            ws.send(Message::Text(req.to_string().into()))
                .await
                .map_err(|e| NostrError::WebSocket(e.to_string()))?;

            let mut events = Vec::new();
            let collect_deadline = Duration::from_secs(5);
            let _ = timeout(collect_deadline, async {
                while let Some(msg) = ws.next().await {
                    let msg = msg.map_err(|e| NostrError::WebSocket(e.to_string()))?;
                    if let Message::Text(t) = msg {
                        if let Ok(Value::Array(arr)) = serde_json::from_str::<Value>(&t) {
                            if arr.first().and_then(|v| v.as_str()) == Some("EVENT")
                                && arr.len() >= 3
                            {
                                if let Ok(ev) = serde_json::from_value::<NostrEvent>(arr[2].clone())
                                {
                                    events.push(ev);
                                }
                            }
                            if arr.first().and_then(|v| v.as_str()) == Some("EOSE") {
                                break;
                            }
                        }
                    }
                }
                Ok::<(), NostrError>(())
            })
            .await;

            let close = json!(["CLOSE", sub_id]);
            let _ = ws.send(Message::Text(close.to_string().into())).await;
            let _ = ws.close(None).await;
            Ok(events)
        };
        timeout(overall, fut)
            .await
            .map_err(|_| NostrError::Timeout)?
    }
}

fn rand_id() -> [u8; 8] {
    use rand::RngCore;
    let mut b = [0u8; 8];
    rand::rngs::OsRng.fill_bytes(&mut b);
    b
}
