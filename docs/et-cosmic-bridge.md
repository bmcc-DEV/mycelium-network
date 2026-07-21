# Ponte ET-COSMIC → Mycelium (fase tropical / PQC / DistanceBridge)

O que foi portado do stack ETΞRNET / VOID-COSMIC **para o Mycelium Network**,
sem Tampermonkey, SOV, Hardhat, WebGPU nem claim de hardware quântico.

## Mapa de camadas

| ET-COSMIC | Mycelium | Estado |
|-----------|----------|--------|
| Layer 0 GhostID | `mycelium-ghostid` + `decoherence` | ✅ + API descoerência |
| Layer 1 QEL | `mycelium-qel` + `topological` | ✅ + carga topológica |
| Layer 2 DistanceBridge | `mycelium-distancebridge` | ✅ fase 0 (seleção / fallback) |
| Layer 5 PQC | `mycelium-pqc` | ✅ ML-KEM-1024 nativo (sem WASM) |
| Nostr mailbox | `mycelium-nostr` | ✅ já existia |
| IPFS | `mycelium-ipfs` | ✅ blockstore local |
| Max-Plus / Physarum | `mycelium-tropical` | ✅ greenfield (tese unificada) |
| LUSUS / WebGPU / vHGPU | — | ❌ fora de âmbito |
| UTXO / Lightning / SOV | — | ❌ fora de âmbito |
| Injectors Tampermonkey | — | ❌ fora de âmbito |

## Novos crates

```
crates/mycelium-tropical/       # Max-Plus, Bellman, Physarum, CFL, Hilbert
crates/mycelium-pqc/            # ML-KEM-1024 (port void_core/pqc.rs)
crates/mycelium-distancebridge/ # select_transports / fallback_order / anderson_cage
```

## Honestidade

- **Não** alegamos vantagem quântica de hardware.
- ML-DSA completo fica para quando a crate estabilizar; neste sprint só KEM real.
- DistanceBridge fase 0 **não** fala BLE/LoRa — só escolhe `TransportHint`.
- Daemon **não** foi reescrito com loop RSA (isso é sprint seguinte).

## Testes

```bash
cargo test -p mycelium-tropical -p mycelium-pqc -p mycelium-distancebridge \
  -p mycelium-qel -p mycelium-ghostid
```

## Uso rápido

```rust
use mycelium_tropical::{BellmanOperator, PhysarumNetwork, Tropical, TropicalMatrix};
use mycelium_distancebridge::{select_transports, TransportContext};
use mycelium_pqc::{mlkem_keygen, mlkem_encapsulate, mlkem_decapsulate};
```

Ver também: [`nostr-qel.md`](nostr-qel.md) (Hybrid Theory A+B).
