# Plasma Ion Migration

## Problema

Hoje `Ion` tem campo `host: NodeId` mas o `Cloud` (Plasma) é estritamente
local — não há como migrar um Ion de um nó para outro.

## Solução proposta: gossip de oferta + handshake

### Fluxo

```
Nó A (sobrecarregado)              Nó B (ocioso)
  │                                      │
  ├─ IonOffer(webapp, carga=+) ──►       │
  │                                      │
  │                              ◄───────┤ IonAccept(webapp)
  │                                      │
  ├─ IonMigrate(webapp, layers…) ──►     │
  │                                      ├─ Chamber::suck(void)
  │                                      ├─ Horizon::expose(orbit)
  │                                      │
  │                              ◄───────┤ IonReady(webapp, upstream)
  │                                      │
  ├─ Horizon::expose(replica)            │
  ├─ DNS update (fallback)               │
  │                                      │
```

### Envelope

```rust
enum Envelope {
    // ... existentes ...
    IonOffer {
        ion: String,
        host: NodeId,
        charge: Charge,
        desired_replicas: u32,
        layers: Vec<ContentId>,
    },
    IonAccept {
        ion: String,
        acceptor: NodeId,
    },
    IonMigrate {
        ion: String,
        void: Void,
        layers: Vec<(ContentId, Vec<u8>)>,  // conteúdo das layers
    },
    IonReady {
        ion: String,
        node: NodeId,
        upstream: String,
    },
}
```

### Implementação

1. **IonOffer publish** — quando `cloud.hungry()` retorna ions com carga positiva, publica `IonOffer` no gossip
2. **IonAccept** — nó ocioso (`resources.cpu_cores > 0 && flywheel.pending() == 0`) aceita
3. **Migrate** — nó origem envia `Void` + layers via DHT/gossip direto
4. **IonReady** — destino frutifica Chamber + expõe no Horizon + avisa origem
5. **Redirect** — origem adiciona rota no Horizon para o novo upstream (gravity routing)

### CLI

```bash
mycelium ion migrate --ion webapp --to <peer_id>
mycelium ion list           # mostra ions locais + réplicas remotas
```

## Próximos passos

1. Criar `Envelope` variants para os 4 tipos de mensagem
2. Adicionar tick no organismo que publica `IonOffer` se `cloud.hungry()` non-empty
3. Implementar `handle_envelope` para `IonAccept`, `IonMigrate`, `IonReady`
4. Implementar `fruit_ion` remoto (já existe `fruit_ion` local, adaptar)
5. Teste de integração: dois nós, um overloaded, migra Ion
