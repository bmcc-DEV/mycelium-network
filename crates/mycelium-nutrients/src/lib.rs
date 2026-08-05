//! # mycelium-nutrients
//!
//! Economia do Substrato: não há cobrança em dinheiro fiat. Quem alimenta
//! a rede é alimentado pela rede. Este ledger é local e sem consenso
//! distribuído nesta fase — cada nó contabiliza os nutrientes que produz
//! e consome; a liquidação via gossip é trabalho futuro.
//!
//! ## Micelial Value Layer (Fases 1–4 do lastro)
//!
//! Transações assinadas (GhostID/NIP-01 Schnorr) que movem nutrientes entre
//! carteiras por motivos económicos reais:
//! - **Consumption** — pagar-por-sessão/stream (Fase 1: software, música, jogos)
//! - **Seeding** — recompensa ao nó que hospeda/entrega o binário (CDN comunitária)
//! - **Compute / Relay** — Proof-of-Compute / Proof-of-Relay (Fase 2: hardware)
//! - **Equity / Royalty** — cotas de ativos físicos + royalties em transferência (Fase 3: RWA)
//! - **Revenue** — receita de empresas/cooperativas → pool de liquidez (Fase 4)

use mycelium_core::{NodeId, Nutrient, Resources};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Erros da economia bioquímica.
#[derive(Debug, thiserror::Error)]
pub enum NutrientError {
    #[error("saldo insuficiente de {nutrient}: tem {have}, precisa de {need}")]
    Starved {
        nutrient: Nutrient,
        have: u64,
        need: u64,
    },
    #[error("transferência: {0}")]
    Transfer(String),
}

/// Motivo económico de uma transferência (mapeia as fases do lastro).
#[derive(Clone, Debug, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TxKind {
    /// Pagamento por consumo: sessão de jogo, música ouvida, código executado (Fase 1).
    Consumption,
    /// Recompensa ao seeder que hospedou/entregou o binário (CDN comunitária, Fase 1).
    Seeding,
    /// Proof-of-Compute: pagamento por processamento (Fase 2).
    Compute,
    /// Proof-of-Relay: pagamento por tráfego roteado (Fase 2).
    Relay,
    /// Cota de utilização de ativo físico fracionado (RWA, Fase 3).
    Equity,
    /// Royalty automático sobre transferência secundária (Fase 3/4).
    Royalty,
    /// Receita de empresa/cooperativa a alimentar o pool (Fase 4).
    Revenue,
}

impl TxKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Consumption => "consumption",
            Self::Seeding => "seeding",
            Self::Compute => "compute",
            Self::Relay => "relay",
            Self::Equity => "equity",
            Self::Royalty => "royalty",
            Self::Revenue => "revenue",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "consumption" => Some(TxKind::Consumption),
            "seeding" => Some(TxKind::Seeding),
            "compute" => Some(TxKind::Compute),
            "relay" => Some(TxKind::Relay),
            "equity" => Some(TxKind::Equity),
            "royalty" => Some(TxKind::Royalty),
            "revenue" => Some(TxKind::Revenue),
            _ => None,
        }
    }
}

impl std::str::FromStr for TxKind {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_str(s).ok_or_else(|| format!("kind inválido: {}", s))
    }
}

/// Payload canónico (o que é assinado).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TransferPayload {
    pub kind: TxKind,
    /// GhostID pubkey x-only do pagador (32 bytes).
    pub from: [u8; 32],
    /// GhostID pubkey x-only do recebedor (32 bytes).
    pub to: [u8; 32],
    pub nutrient: Nutrient,
    pub amount: u64,
    pub memo: String,
    /// Spore/ativo/empresa envolvido (ex.: "doom-1993", "estudio-x", "coop-som").
    pub asset: Option<String>,
    /// Proteção contra replay.
    pub nonce: u64,
    /// Unix seconds.
    pub ts: u64,
}

/// Transferência assinada (Schnorr NIP-01, GhostID).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SignedTransfer {
    #[serde(flatten)]
    pub payload: TransferPayload,
    /// Assinatura Schnorr de 64 bytes em hex (128 chars).
    pub sig: String,
}

impl SignedTransfer {
    /// Bytes canónicos para assinar/verificar.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(&self.payload).unwrap_or_default()
    }

    pub fn short(&self) -> String {
        if self.sig.len() >= 12 {
            self.sig[..12].to_string()
        } else {
            self.sig.clone()
        }
    }
}

/// Um lançamento no ledger.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Exchange {
    pub counterparty: Option<NodeId>,
    pub nutrient: Nutrient,
    /// Positivo = ganho; negativo = gasto.
    pub delta: i64,
    pub memo: String,
    pub kind: Option<TxKind>,
}

/// Ledger local de nutrientes de um nó.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Ledger {
    pub balances: HashMap<Nutrient, u64>,
    pub history: Vec<Exchange>,
    /// nonces já vistos por pagador (replay protection).
    seen_nonces: HashMap<[u8; 32], u64>,
    /// Transferências recentes (para a UI).
    pub transfers: Vec<SignedTransfer>,
}

impl Ledger {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn balance(&self, nutrient: Nutrient) -> u64 {
        self.balances.get(&nutrient).copied().unwrap_or(0)
    }

    pub fn history(&self) -> &[Exchange] {
        &self.history
    }

    /// Credita nutrientes ganhos por uma contribuição.
    pub fn feed(&mut self, nutrient: Nutrient, amount: u64, memo: impl Into<String>) {
        self.feed_kind(nutrient, amount, memo, None);
    }

    pub fn feed_kind(
        &mut self,
        nutrient: Nutrient,
        amount: u64,
        memo: impl Into<String>,
        kind: Option<TxKind>,
    ) {
        *self.balances.entry(nutrient).or_default() += amount;
        self.history.push(Exchange {
            counterparty: None,
            nutrient,
            delta: amount as i64,
            memo: memo.into(),
            kind,
        });
    }

    /// Consome nutrientes para trocar por recursos de outro nó.
    pub fn metabolize(
        &mut self,
        nutrient: Nutrient,
        amount: u64,
        counterparty: Option<NodeId>,
        memo: impl Into<String>,
    ) -> Result<(), NutrientError> {
        let have = self.balance(nutrient);
        if have < amount {
            return Err(NutrientError::Starved {
                nutrient,
                have,
                need: amount,
            });
        }
        *self.balances.entry(nutrient).or_default() -= amount;
        self.history.push(Exchange {
            counterparty,
            nutrient,
            delta: -(amount as i64),
            memo: memo.into(),
            kind: None,
        });
        Ok(())
    }

    /// Credita a recompensa inicial pela contribuição declarada de recursos,
    /// segundo a tabela do manifesto:
    /// CPU→ATP, RAM→Enzymes, Storage→Mycelia, Bandwidth→Spores.
    pub fn pledge(&mut self, resources: &Resources) {
        if resources.cpu_cores > 0 {
            self.feed(
                Nutrient::Atp,
                resources.cpu_cores as u64 * 10,
                "pledge: cpu",
            );
        }
        if resources.ram_mib > 0 {
            self.feed(Nutrient::Enzymes, resources.ram_mib / 512, "pledge: ram");
        }
        if resources.storage_gib > 0 {
            self.feed(Nutrient::Mycelia, resources.storage_gib, "pledge: storage");
        }
        if resources.bandwidth_mbps > 0 {
            self.feed(
                Nutrient::Spores,
                resources.bandwidth_mbps,
                "pledge: bandwidth",
            );
        }
    }

    /// Recompensa contínua por uptime (chamada periodicamente).
    pub fn heartbeat(&mut self, hours: u64) {
        self.feed(Nutrient::Resilience, hours, "uptime heartbeat");
    }

    // ── Rewards (Fase 2: Proof-of-Compute / Proof-of-Relay) ──
    pub fn compute_reward(&mut self, ops: u64) {
        self.feed_kind(
            Nutrient::Atp,
            ops,
            "proof-of-compute",
            Some(TxKind::Compute),
        );
    }

    pub fn relay_reward(&mut self, mb: u64) {
        self.feed_kind(
            Nutrient::Spores,
            mb,
            "proof-of-relay",
            Some(TxKind::Relay),
        );
    }

    pub fn seed_reward(&mut self, bytes_mb: u64) {
        self.feed_kind(
            Nutrient::Mycelia,
            bytes_mb,
            "seeding: cdn comunitária",
            Some(TxKind::Seeding),
        );
    }

    // ── Micelial Value Layer ──
    /// Aplica uma transferência assinada ao ledger local.
    ///
    /// - `my_identity`: GhostID pubkey x-only do nó atual.
    /// - Se `from == eu`: debita (após verificar saldo e assinatura).
    /// - Se `to == eu`: credita e guarda a transferência.
    /// - Royalties (10%) são retidos automaticamente para o autor original.
    ///
    /// Nota: a verificação da assinatura Schnorr é feita pela camada de
    /// identidade (GhostID) antes de chamar este método — aqui validamos
    /// nonce e saldo.
    pub fn apply_transfer(
        &mut self,
        tx: &SignedTransfer,
        my_identity: &[u8; 32],
    ) -> Result<(), NutrientError> {
        let p = &tx.payload;
        let prev = self.seen_nonces.get(&p.from).copied().unwrap_or(0);
        if p.nonce <= prev {
            return Err(NutrientError::Transfer(format!(
                "replay/nonce {} <= {}",
                p.nonce, prev
            )));
        }
        if p.amount == 0 {
            return Err(NutrientError::Transfer("montante zero".into()));
        }
        self.seen_nonces.insert(p.from, p.nonce);

        // Não é para nós nem de nós → apenas regista (testemunha) se não vimos.
        let mine_from = p.from == *my_identity;
        let mine_to = p.to == *my_identity;

        if mine_from {
            let have = self.balance(p.nutrient);
            if have < p.amount {
                self.seen_nonces.insert(p.from, prev);
                return Err(NutrientError::Starved {
                    nutrient: p.nutrient,
                    have,
                    need: p.amount,
                });
            }
            *self.balances.entry(p.nutrient).or_default() -= p.amount;
            self.history.push(Exchange {
                counterparty: None,
                nutrient: p.nutrient,
                delta: -(p.amount as i64),
                memo: format!("→ {} | {}", hex::encode(&p.to[..4]), p.memo),
                kind: Some(p.kind),
            });
        }

        if mine_to {
            *self.balances.entry(p.nutrient).or_default() += p.amount;
            self.history.push(Exchange {
                counterparty: None,
                nutrient: p.nutrient,
                delta: p.amount as i64,
                memo: format!("← {} | {}", hex::encode(&p.from[..4]), p.memo),
                kind: Some(p.kind),
            });
        }

        // Royalty: 10% de cada Equity/Consumption secundário retido ao autor.
        if mine_to && matches!(p.kind, TxKind::Equity | TxKind::Royalty) {
            let royalty = p.amount / 10;
            if royalty > 0 {
                // Reserva local "royalty pool" para o autor original.
                *self.balances.entry(Nutrient::Resilience).or_default() += royalty;
                self.history.push(Exchange {
                    counterparty: None,
                    nutrient: Nutrient::Resilience,
                    delta: royalty as i64,
                    memo: format!("royalty 10% sobre {:?} ({})", p.kind, p.asset.as_deref().unwrap_or("-")),
                    kind: Some(TxKind::Royalty),
                });
            }
        }

        if self.transfers.len() > 500 {
            self.transfers.drain(..100);
        }
        self.transfers.push(tx.clone());
        Ok(())
    }

    pub fn recent_transfers(&self) -> &[SignedTransfer] {
        &self.transfers
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pledge_credits_all_currencies() {
        let mut ledger = Ledger::new();
        let r: Resources = "2cpu,4gb,100gb,50mbps".parse().unwrap();
        ledger.pledge(&r);
        assert_eq!(ledger.balance(Nutrient::Atp), 20);
        assert_eq!(ledger.balance(Nutrient::Enzymes), 8);
        assert_eq!(ledger.balance(Nutrient::Mycelia), 100);
        assert_eq!(ledger.balance(Nutrient::Spores), 50);
        assert_eq!(ledger.balance(Nutrient::Resilience), 0);
    }

    #[test]
    fn cannot_metabolize_more_than_balance() {
        let mut ledger = Ledger::new();
        ledger.feed(Nutrient::Atp, 5, "test");
        let err = ledger
            .metabolize(Nutrient::Atp, 10, None, "deploy")
            .unwrap_err();
        assert!(matches!(
            err,
            NutrientError::Starved {
                have: 5,
                need: 10,
                ..
            }
        ));
        assert!(ledger.metabolize(Nutrient::Atp, 5, None, "deploy").is_ok());
        assert_eq!(ledger.balance(Nutrient::Atp), 0);
    }

    #[test]
    fn history_records_exchanges() {
        let mut ledger = Ledger::new();
        ledger.feed(Nutrient::Spores, 3, "relay");
        ledger.heartbeat(1);
        assert_eq!(ledger.history().len(), 2);
    }

    #[test]
    fn relay_and_compute_rewards() {
        let mut ledger = Ledger::new();
        ledger.relay_reward(100);
        ledger.compute_reward(50);
        assert_eq!(ledger.balance(Nutrient::Spores), 100);
        assert_eq!(ledger.balance(Nutrient::Atp), 50);
    }

    fn mk_tx(from: [u8; 32], to: [u8; 32], amount: u64, nonce: u64) -> SignedTransfer {
        SignedTransfer {
            payload: TransferPayload {
                kind: TxKind::Consumption,
                from,
                to,
                nutrient: Nutrient::Atp,
                amount,
                memo: "teste".into(),
                asset: Some("doom-1993".into()),
                nonce,
                ts: 1,
            },
            sig: hex::encode([0u8; 64]),
        }
    }

    #[test]
    fn transfer_debits_sender_credits_receiver() {
        let alice = [1u8; 32];
        let bob = [2u8; 32];
        let mut a = Ledger::new();
        a.feed(Nutrient::Atp, 100, "start");

        let tx = mk_tx(alice, bob, 30, 1);
        // Alice paga (é o `from`)
        a.apply_transfer(&tx, &alice).unwrap();
        assert_eq!(a.balance(Nutrient::Atp), 70);

        // Bob recebe (é o `to`)
        let mut b = Ledger::new();
        b.apply_transfer(&tx, &bob).unwrap();
        assert_eq!(b.balance(Nutrient::Atp), 30);
    }

    #[test]
    fn replay_is_rejected() {
        let alice = [1u8; 32];
        let bob = [2u8; 32];
        let mut a = Ledger::new();
        a.feed(Nutrient::Atp, 100, "start");
        let tx = mk_tx(alice, bob, 30, 1);
        a.apply_transfer(&tx, &alice).unwrap();
        assert!(a.apply_transfer(&tx, &alice).is_err());
    }

    #[test]
    fn starved_sender_is_rejected() {
        let alice = [1u8; 32];
        let bob = [2u8; 32];
        let mut a = Ledger::new();
        let tx = mk_tx(alice, bob, 30, 1);
        assert!(a.apply_transfer(&tx, &alice).is_err());
    }

    #[test]
    fn equity_retains_royalty() {
        let alice = [1u8; 32];
        let bob = [2u8; 32];
        let mut b = Ledger::new();
        let mut tx = mk_tx(alice, bob, 100, 1);
        tx.payload.kind = TxKind::Equity;
        b.apply_transfer(&tx, &bob).unwrap();
        // 100 crédito + 10 de royalty retido (Resilience)
        assert_eq!(b.balance(Nutrient::Atp), 100);
        assert_eq!(b.balance(Nutrient::Resilience), 10);
    }
}
