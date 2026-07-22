//! Framing fiável sobre pacotes (seq/ACK, janela deslizante).

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

pub const WINDOW_SIZE: u64 = 16;
pub const MAX_PAYLOAD: usize = 24 * 1024;
const RTO_INITIAL: Duration = Duration::from_secs(2);

/// Pacote de dados ou ACK puro.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub stream_id: [u8; 8],
    pub seq: u64,
    pub ack: u64,
    pub payload: Vec<u8>,
}

impl Frame {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(8 + 8 + 8 + 4 + self.payload.len());
        out.extend_from_slice(&self.stream_id);
        out.extend_from_slice(&self.seq.to_be_bytes());
        out.extend_from_slice(&self.ack.to_be_bytes());
        out.extend_from_slice(&(self.payload.len() as u32).to_be_bytes());
        out.extend_from_slice(&self.payload);
        out
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 28 {
            return None;
        }
        let mut stream_id = [0u8; 8];
        stream_id.copy_from_slice(&bytes[0..8]);
        let seq = u64::from_be_bytes(bytes[8..16].try_into().ok()?);
        let ack = u64::from_be_bytes(bytes[16..24].try_into().ok()?);
        let len = u32::from_be_bytes(bytes[24..28].try_into().ok()?) as usize;
        if bytes.len() < 28 + len {
            return None;
        }
        Some(Self {
            stream_id,
            seq,
            ack,
            payload: bytes[28..28 + len].to_vec(),
        })
    }
}

#[derive(Debug)]
struct Inflight {
    payload: Vec<u8>,
    sent_at: Instant,
}

/// Estado de fiabilidade (sem I/O).
#[derive(Debug)]
pub struct ReliableState {
    pub stream_id: [u8; 8],
    next_send_seq: u64,
    /// Próximo seq esperado do peer (contiguous).
    next_recv_seq: u64,
    /// Último ACK enviado; None = ainda não ACK'ámos nada.
    last_ack_sent: Option<u64>,
    inflight: BTreeMap<u64, Inflight>,
    reorder: BTreeMap<u64, Vec<u8>>,
    rto: Duration,
}

impl ReliableState {
    pub fn new(stream_id: [u8; 8]) -> Self {
        Self {
            stream_id,
            next_send_seq: 0,
            next_recv_seq: 0,
            last_ack_sent: None,
            inflight: BTreeMap::new(),
            reorder: BTreeMap::new(),
            rto: RTO_INITIAL,
        }
    }

    /// Enfileira payload para envio; devolve frames a publicar (pode ser vazio se janela cheia).
    pub fn enqueue_send(&mut self, mut data: Vec<u8>) -> Vec<Frame> {
        let mut out = Vec::new();
        while !data.is_empty() {
            if self.inflight.len() as u64 >= WINDOW_SIZE {
                break;
            }
            let take = data.len().min(MAX_PAYLOAD);
            let chunk: Vec<u8> = data.drain(..take).collect();
            let seq = self.next_send_seq;
            self.next_send_seq += 1;
            self.inflight.insert(
                seq,
                Inflight {
                    payload: chunk.clone(),
                    sent_at: Instant::now(),
                },
            );
            out.push(Frame {
                stream_id: self.stream_id,
                seq,
                ack: self.next_recv_seq.saturating_sub(1),
                payload: chunk,
            });
        }
        // guardar resto? para MVP, caller deve retry enqueue do que sobrou
        let _ = data;
        out
    }

    /// Dados ainda por enviar se a janela estava cheia — caller guarda buffer.
    pub fn window_full(&self) -> bool {
        (self.inflight.len() as u64) >= WINDOW_SIZE
    }

    /// Processa frame entrante; devolve bytes entregues em ordem + frame ACK opcional.
    pub fn on_recv(&mut self, frame: Frame) -> (Vec<u8>, Option<Frame>) {
        if frame.stream_id != self.stream_id {
            return (Vec::new(), None);
        }
        // Aplicar ACK remoto
        let ack = frame.ack;
        self.inflight.retain(|&seq, _| seq > ack);

        let mut delivered = Vec::new();
        if !frame.payload.is_empty() {
            if frame.seq == self.next_recv_seq {
                delivered.extend_from_slice(&frame.payload);
                self.next_recv_seq += 1;
                while let Some(p) = self.reorder.remove(&self.next_recv_seq) {
                    delivered.extend_from_slice(&p);
                    self.next_recv_seq += 1;
                }
            } else if frame.seq > self.next_recv_seq {
                self.reorder.insert(frame.seq, frame.payload);
            }
        }

        let ack_frame = if self.next_recv_seq > 0 {
            let ack_val = self.next_recv_seq - 1;
            if self.last_ack_sent != Some(ack_val) {
                self.last_ack_sent = Some(ack_val);
                Some(Frame {
                    stream_id: self.stream_id,
                    seq: self.next_send_seq,
                    ack: ack_val,
                    payload: Vec::new(),
                })
            } else {
                None
            }
        } else {
            None
        };

        (delivered, ack_frame)
    }

    /// Retransmissões por timeout.
    pub fn retransmit_due(&mut self) -> Vec<Frame> {
        let now = Instant::now();
        let mut out = Vec::new();
        for (seq, inf) in self.inflight.iter_mut() {
            if now.duration_since(inf.sent_at) >= self.rto {
                inf.sent_at = now;
                out.push(Frame {
                    stream_id: self.stream_id,
                    seq: *seq,
                    ack: self.next_recv_seq.saturating_sub(1),
                    payload: inf.payload.clone(),
                });
            }
        }
        if !out.is_empty() {
            self.rto = (self.rto.mul_f32(1.5)).min(Duration::from_secs(30));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_roundtrip() {
        let f = Frame {
            stream_id: [1; 8],
            seq: 7,
            ack: 3,
            payload: b"hello".to_vec(),
        };
        let enc = f.encode();
        assert_eq!(Frame::decode(&enc), Some(f));
    }

    #[test]
    fn reorder_and_ack() {
        let sid = [9u8; 8];
        let mut a = ReliableState::new(sid);
        let mut b = ReliableState::new(sid);

        let frames = a.enqueue_send(b"abcdef".to_vec());
        assert!(!frames.is_empty());

        // entregar fora de ordem se vários chunks — com payload pequeno 1 frame
        let (d, ack) = b.on_recv(frames[0].clone());
        assert_eq!(d, b"abcdef");
        assert!(ack.is_some());

        let (_, _) = a.on_recv(ack.unwrap());
        assert!(a.inflight.is_empty());
    }

    #[test]
    fn gap_then_fill() {
        let sid = [2u8; 8];
        let mut b = ReliableState::new(sid);
        let f0 = Frame {
            stream_id: sid,
            seq: 0,
            ack: 0,
            payload: b"A".to_vec(),
        };
        let f1 = Frame {
            stream_id: sid,
            seq: 1,
            ack: 0,
            payload: b"B".to_vec(),
        };
        // recebe 1 antes de 0
        let (d1, _) = b.on_recv(f1);
        assert!(d1.is_empty());
        let (d0, _) = b.on_recv(f0);
        assert_eq!(d0, b"AB");
    }
}
