//! Store-and-forward mínimo via DHT (mailbox).
//!
//! Só funciona **após** o nó já ter entrada na DHT (bootstrap). Não resolve
//! o ovo/galinha de dois peers mutuamente inacessíveis sem seed.

use libp2p::PeerId;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

pub const MAILBOX_DHT_PREFIX: &[u8] = b"/mycelium/mailbox/";
pub const MAILBOX_ACK_PREFIX: &[u8] = b"/mycelium/mailbox-ack/";
pub const MAX_MAILBOX_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MailboxContentType {
    Generic,
    IsotopeAtom,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MailboxMessage {
    pub id_hex: String,
    pub from: String,
    pub to: String,
    pub content_type: MailboxContentType,
    pub payload: Vec<u8>,
    pub timestamp: u64,
    pub ttl_secs: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MailboxAck {
    pub message_id_hex: String,
    pub receiver: String,
    pub timestamp: u64,
}

pub fn mailbox_key(to: &PeerId, msg_id_hex: &str) -> Vec<u8> {
    let mut k = MAILBOX_DHT_PREFIX.to_vec();
    k.extend_from_slice(to.to_string().as_bytes());
    k.push(b'/');
    k.extend_from_slice(msg_id_hex.as_bytes());
    k
}

pub fn mailbox_prefix(to: &PeerId) -> Vec<u8> {
    let mut k = MAILBOX_DHT_PREFIX.to_vec();
    k.extend_from_slice(to.to_string().as_bytes());
    k.push(b'/');
    k
}

pub fn ack_key(msg_id_hex: &str) -> Vec<u8> {
    let mut k = MAILBOX_ACK_PREFIX.to_vec();
    k.extend_from_slice(msg_id_hex.as_bytes());
    k
}

pub fn make_message(
    from: &PeerId,
    to: &PeerId,
    payload: Vec<u8>,
    content_type: MailboxContentType,
) -> Result<MailboxMessage, String> {
    if payload.len() > MAX_MAILBOX_BYTES {
        return Err(format!(
            "mensagem excede {MAX_MAILBOX_BYTES} bytes ({})",
            payload.len()
        ));
    }
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let material = format!("{from}|{to}|{ts}|{}", payload.len());
    let id_hex = hex::encode(blake3::hash(material.as_bytes()).as_bytes());
    Ok(MailboxMessage {
        id_hex,
        from: from.to_string(),
        to: to.to_string(),
        content_type,
        payload,
        timestamp: ts,
        ttl_secs: 3600,
    })
}

pub fn make_ack(receiver: &PeerId, message_id_hex: &str) -> MailboxAck {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    MailboxAck {
        message_id_hex: message_id_hex.to_string(),
        receiver: receiver.to_string(),
        timestamp: ts,
    }
}

pub fn is_expired(msg: &MailboxMessage) -> bool {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    now.saturating_sub(msg.timestamp) > msg.ttl_secs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_oversized() {
        let from = PeerId::random();
        let to = PeerId::random();
        let big = vec![0u8; MAX_MAILBOX_BYTES + 1];
        assert!(make_message(&from, &to, big, MailboxContentType::Generic).is_err());
    }

    #[test]
    fn ttl_expiry() {
        let msg = MailboxMessage {
            id_hex: "ab".into(),
            from: "a".into(),
            to: "b".into(),
            content_type: MailboxContentType::Generic,
            payload: vec![],
            timestamp: 0,
            ttl_secs: 1,
        };
        assert!(is_expired(&msg));
    }
}
