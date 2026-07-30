# Entropy — Segredos com meia-vida

## Conceito

Um segredo nunca existe inteiro em um só lugar. É fatiado em **Shades**
(sombras) via **Shamir Secret Sharing** sobre GF(256). Só quando M de N
Shades são coletadas pelas hifas o segredo materializa — e só existe
por uma meia-vida curta antes de evaporar da memória.

## Implementação

- **Crate:** `entropy` em `crates/entropy/`
- **Shamir real sobre GF(256)** com polinômio irredutível `x^8 + x^4 + x^3 + x + 1` (0x11b)
- **Aritmética:** gf_add (XOR), gf_mul (shift-and-add), gf_inv (a^254 via exp squaring)
- **Lagrange interpolation** em x=0 para reconstrução

### Tipos

```rust
pub struct Shade {
    pub index: u8,        // x ∈ [1, 255] no polinômio
    pub shares: Vec<u8>,  // y = f(x) para cada byte do segredo
}

pub struct ChaosKey {
    bytes: Vec<u8>,        // segredo materializado
    born: Instant,         // momento da materialização
    half_life: Duration,   // default 30s
}

pub struct Vault {
    custody: Vec<(NodeId, Shade)>,  // shades custodidas
}
```

### Funcionalidades existentes (✅)

- `Vault::shatter(secret, m, n)` — divide segredo em N shades (M necessárias)
- `ChaosKey::materialize(shades, threshold)` — reconstrói o segredo
- `ChaosKey::reveal()` — lê o segredo se não evaporou
- `ChaosKey::evaporate()` — zera o buffer imediatamente
- `Vault::hold(custodian, shade)` — custodia uma shade
- `Vault::gather()` — coleta shades custodiadas
- CLI: `mycelium entropy shatter|reconstruct|status`
- Organismo: `Vault` integrado, comandos no controle socket

### O que falta (🟡)

- **Distribuição via gossip:** enviar shades para peers usando Envelope
- **Coleta remota:** pedir shades por DecayQuery/DecayReply-like protocol
- **Persistência do Vault** no NodeStore

## CLI

```bash
# Fragmenta um segredo em 5 shades (precisa de 3 pra reconstruir)
mycelium entropy shatter --secret "minha-chave-super-secreta" -k 3 -n 5

# Reconstrói o segredo
mycelium entropy reconstruct -k 3

# Mostra shades em custódia
mycelium entropy status
```

## Testes

```
cargo test -p entropy
```

Testes cobrem: roundtrip, threshold insuficiente, half-life evaporação,
bad threshold, vault hold+gather.

## Design da integração com hifas (planejado)

```rust
// Envelope (protocol.rs)
enum Envelope {
    // ... existentes ...
    ShadeOffer {
        shard_index: u8,
        total: u8,
        shade_bytes: Vec<u8>,
        custodian: NodeId,
    },
    ShadeRequest {
        threshold: u8,
        requester: NodeId,
    },
}
```

1. **Shatter local** → distribui shades via `ShadeOffer` para peers aleatórios
2. **Peer recebe** → `hold()` a shade no vault local + persiste
3. **Reconstruct** → envia `ShadeRequest` → peers respondem com `ShadeOffer` →
   `gather()` + `materialize()`
