# Nutrient Ledger Distribuído

## Problema

Hoje cada nó tem seu `Ledger` local (ATP, Enzymes, Mycelia, Spores, Resilience)
sem sincronização com a rede. Não há como um nó A pagar um nó B por trabalho.

## Solução proposta: CRDT sobre gossip

Usar o mesmo padrão do Isotope: **LWW (Last-Writer-Wins) register** replicado
via gossip no tópico `mycelium/nutrients/v1`.

### Design

```
Envelope::BalanceSync {
    node_id: NodeId,
    balances: HashMap<Nutrient, u64>,
    clock: u64,            // timestamp UNIX
    delta: i64,            // último delta (para verificação)
    signature: [u8; 64],   // assinatura ed25519 do node_id
}
```

### Regras

1. Cada nó publica seu próprio balance no gossip a cada 60s
2. Ao receber um `BalanceSync`:
   - Verificar assinatura
   - Se `clock > local_clock` para aquele `node_id`: aceitar
   - Se `clock == local_clock`: usar o maior balance (LWW tiebreak)
3. Ao executar trabalho remoto (`MomentumReport`):
   - Emissor debita ATP da contraparte
   - Executor credita ATP na contraparte
   - Ambos publicam `BalanceSync` atualizado

### Envelope

```rust
enum Envelope {
    // ... existentes ...
    BalanceSync {
        node_id: NodeId,
        balances: HashMap<Nutrient, u64>,
        clock: u64,
    },
}
```

### Implementação

1. **Ledger remoto** — `RemoteLedger` struct que mantém `HashMap<NodeId, (HashMap<Nutrient, u64>, u64)>`
2. **Tick de publish** — a cada 60s, publica `BalanceSync` no gossip
3. **Handle** — no `handle_envelope`, processa `BalanceSync` entrante
4. **MomentumReport** — ao receber report de trabalho remoto, ajusta balance local + publica sync

### CLI

```bash
mycelium balance          # mostra balance local + remotos conhecidos
mycelium balance --peer   # mostra balance de um peer específico
```

## Riscos

- Sem consenso BFT: um nó malicioso pode mentir seu balance
- Solução futura: zk-proof de recursos ou consenso leve (Raft entre esporocarps)
- Para MVP, confiança baseada em reputação (Spores + Scent)
