//! # mycelium-nutrients
//!
//! Economia do Substrato: não há cobrança em dinheiro fiat. Quem alimenta
//! a rede é alimentado pela rede. Este ledger é local e sem consenso
//! distribuído nesta fase — cada nó contabiliza os nutrientes que produz
//! e consome; a liquidação via gossip é trabalho futuro.

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
}

/// Um lançamento no ledger.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Exchange {
    pub counterparty: Option<NodeId>,
    pub nutrient: Nutrient,
    /// Positivo = ganho; negativo = gasto.
    pub delta: i64,
    pub memo: String,
}

/// Ledger local de nutrientes de um nó.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Ledger {
    balances: HashMap<Nutrient, u64>,
    history: Vec<Exchange>,
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
        *self.balances.entry(nutrient).or_default() += amount;
        self.history.push(Exchange {
            counterparty: None,
            nutrient,
            delta: amount as i64,
            memo: memo.into(),
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
}
