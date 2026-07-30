# Status do Substrato

## Build
```bash
cargo build               → 0 warnings, 0 erros
cargo test                → 112 passed (default features)
cargo test --all-features → 115 passed (com pqc-transport)
```

## Legendas
- ✅ **Feito e testado**
- 🟡 **Feito parcial / wire presente mas incompleto**
- ❌ **Não iniciado**

## Camadas implementadas

### Core
| Módulo | Status | O que faz |
|--------|--------|-----------|
| `mycelium-core` | ✅ | NodeId, ContentId, Membrane, Resources, FruitingBody trait |
| `mycelium-pheromones` | ✅ | Gland (ed25519), Trail, Scent, Decay, Alarm |
| `mycelium-ghostid` | ✅ | Identidade efémera secp256k1 (Nostr anônimo) |
| `mycelium-pqc` | ✅ | ML-KEM-1024 keygen/encapsulate/decapsulate |

### Rede (Hyphae)
| Módulo | Status | O que faz |
|--------|--------|-----------|
| TCP/Noise/Yamux | ✅ | Transporte base |
| QUIC | ✅ | Transporte alternativo |
| mDNS | ✅ | Descoberta LAN |
| Kademlia DHT | ✅ | Bootstrap + record store |
| Gossipsub | ✅ | Pheromones + Lattice + RelayMesh |
| Circuit relay v2 | ✅ | Server + client |
| Identify | ✅ | Troca de endereços |
| WebRTC-direct | 🟡 | Feature-gated (`webrtc`), opcional |
| Nostr transport | ✅ | libp2p sobre WSS (auto folha/floresta) |
| **PQC transport** | 🟡 | Módulo de handshake híbrido existe; **Transport trait completo não implementado** |
| DNS (Cloudflare) | ✅ | Resolução de seeds |
| Seed book | ✅ | HTTP + DNS TXT + arquivo local |
| DuckDNS | ✅ | Publicação de TXT para esporocarps |

### Armazenamento
| Módulo | Status | O que faz |
|--------|--------|-----------|
| SporeBank | ✅ | Plots content-addressed em disco |
| LayerStore | ✅ | Layers Vacuum content-addressed |
| BlockStore (IPFS) | ✅ | Blockstore local (Hybrid Theory) |
| NodeStore | ✅ | Gland, ledger, resources, organismo, nucleus |

### Computação Distribuída
| Módulo | Status | O que faz |
|--------|--------|-----------|
| Giggs (Plot/Mesh) | ✅ | Versionamento mesh content-addressed |
| TheField (Signal) | ✅ | Sinalização com quorum |
| Inertia (Flywheel) | ✅ | Build/Test/Deploy local + remoto |
| Vacuum (Chamber) | ✅ | Runtime OCI-lite com layers |
| Plasma (Ion/Cloud) | ✅ | Orquestração local de Ions |
| **Plasma Ion migration** | ✅ | **IonOffer/IonAccept/IonMigrate/IonReady via gossip. `mycelium ion-migrate`** |
| Singularity (Horizon) | ✅ | Proxy HTTP reverso + rate-limit |

### Estado Distribuído
| Módulo | Status | O que faz |
|--------|--------|-----------|
| Isotope (Nucleus) | ✅ | LWW register ring (4 shards) com Decay protocol |
| **Entropy (Shades)** | 🟡 | **SSS sobre GF(256) funciona. Vault no organismo + CLI pronto. Falta gossip: distribuir/coletar shades via hifas** |

### Economia
| Módulo | Status | O que faz |
|--------|--------|-----------|
| Nutrients (Ledger) | ✅ | Ledger local (ATP, Enzymes, Mycelia, Spores, Resilience) |
| **Nutrient ledger distribuído** | ✅ | **CRDT LWW via `BalanceSync` gossip a cada 60s. `mycelium balance` mostra local + peers** |

### Travessia de Barreiras
| Módulo | Status | O que faz |
|--------|--------|-----------|
| QEL (fragmentação) | ✅ | K-of-N threshold + TransportHint |
| Nostr mailbox | ✅ | RelayPool, NIP-94, shards QEL via Nostr |
| Nostr transport | ✅ | libp2p sobre WSS |
| CandidateRelay | ✅ | Kind 39401/39406 CGNAT↔CGNAT |
| DistanceBridge | ✅ | Seleção inteligente de transporte |

### Observabilidade
| Módulo | Status | O que faz |
|--------|--------|-----------|
| **Prometheus /metrics** | ✅ | **Endpoint `GET /metrics` no Event Horizon + tick 30s** |
| Console HTML | ✅ | `/console` lista ions |
| Health check | ✅ | `/health` |
| Status report | ✅ | Socket de controle + CLI `mycelium status` |

### Infra
| Módulo | Status | O que faz |
|--------|--------|-----------|
| **Sporocarp CDN** | ✅ | **`GET /plots/{id}` + `GET /layers/{id}` no Event Horizon** |
| **Growth Zones** | ✅ | **`ZoneAnnounce` gossip + `mycelium zones`. Prefixo derivado do NodeId** |
| Deploy one-shot | ✅ | `mycelium deploy` |
| Scripts de demo | ✅ | e2e, horizon, seedbook, hybrid, isotope, lattice-remote, nostr-transport |
| Script voluntário | ✅ | volunteer-pipeline, probe/verify-sporocarp, run-folha/public-seed |
| CLI completa | ✅ | 20+ comandos |

## Roadmap

### Concluído nesta sessão
1. **Entropy gossip** — distribuir/coletar shades via Lattice Envelope ✅
2. **PQC híbrido** — handshake pós-Noise com `blake3(noise_secret || pqc_secret)` ✅
3. **Nutrient ledger CRDT** — `BalanceSync` gossip a cada 60s ✅
4. **Plasma Ion migration** — `IonOffer`/`IonAccept`/`IonMigrate`/`IonReady` ✅
5. **Sporocarp CDN** — `GET /plots/{id}` + `GET /layers/{id}` no Horizon ✅
6. **Growth Zones** — `ZoneAnnounce` gossip + `mycelium zones` ✅
7. **Prometheus /metrics** — endpoint + tick 30s ✅

### Próximos passos sugeridos
- **PQC transport real** (Transport trait TCP → KEM → yamux)
- **Plasma reactive scaling** (réplicas automáticas conforme carga)
- **Prometheus alerts** integrados
- **Nutrient ledger com consenso** (Raft/PBFT leve entre esporocarps)
- **Growth Zones** com DHT overlay (distance XOR routing)
