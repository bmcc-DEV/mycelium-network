# Protocolo Lattice (wire)

## Tópico gossip

`mycelium/lattice/v1` — Plots, Signals, Vectors, Atoms, Layers.

## Envelope versionado

Mensagens novas saem como:

```json
{"v":1,"msg":{"LayerNeed":{"id":"Qm…}}}
```

| Campo | Significado |
|-------|-------------|
| `v` | Versão do frame (`1` atual) |
| `msg` | Variante de `Envelope` (serde externally tagged) |

**Compatibilidade:** nós atuais ainda aceitam Envelope nu (sem `v`/`msg`) como legado v1.

**Versões futuras:** se `v > 1` e o binário não conhece, o decode falha e o nó **ignora** a mensagem (log `envelope inválido`) — sem panic.

## Variantes (`msg`)

| Variante | Uso |
|----------|-----|
| `SporePrint` | Plot do Giggs |
| `SignalBroadcast` | Signal do TheField |
| `Resonance` | Quórum / ressonância |
| `VectorOffer` | Oferta Inertia (Build/Test) |
| `MomentumReport` | Resultado Inertia |
| `AtomSync` | Estado Isotope LWW |
| `LayerOffer` / `LayerNeed` | Layers Vacuum |

## Deploy

Só o **origin** do `Signal` executa `Thrust::Deploy`. Remotos aceitam `VectorOffer` de Build/Test.
