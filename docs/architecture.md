# Arquitetura do Micélio

## Organismo (daemon)

O crate `mycelium-node` mantém o nó vivo:

1. **Despertar** — carrega `gland.seed`, ledger, Spore Bank, Field e métricas de hifas
2. **Hifas** — libp2p (QUIC + TCP + DNS/`dnsaddr`), mDNS local, seed book (HTTP + arquivo), Kademlia bootstrap, gossip (`pheromones` + `lattice`)
3. **Spore Bank** — Plots em disco; anúncio/recuperação via Kademlia (`spore/<ContentId>`)
4. **Vacuum Chamber** — processo filho (`mycelium chamber-serve`) com bundle OCI-lite; isolamento `Auto` → bubblewrap se disponível, senão processo simples
5. **Singularity Event Horizon** — reverse proxy HTTP (`:7474`) que roteia `/{ion}/` à Chamber
6. **Controle** — Unix socket `mycelium.sock` (JSON linha-a-linha) para a CLI
7. **Hibernar** — persiste estado; PeerId e ledger sobrevivem ao reboot

## Fluxo Lattice (ponta a ponta)

```
sow(Plot) ──gossip/DHT──► Spore Bank dos vizinhos
     │
     ▼
signal(Pipeline) ──gossip──► TheField (quórum)
     │ resonate…
     ▼ Fired
Inertia Vectors (Build → Test → Deploy)
     ▼
Vacuum Chamber → Plasma Ion → Singularity Event Horizon
```

## Camadas

```
                    ┌─────────────────────────────────────┐
                    │         Mycelium Network            │
                    │        (O Substrato Vivo)           │
                    └─────────────────────────────────────┘
                                      │
           ┌──────────────────────────┼──────────────────────────┐
           ▼                          ▼                          ▼
    ┌─────────────┐           ┌─────────────┐           ┌─────────────┐
    │   Cortex    │   Hifas   │   Medulla   │   Hifas   │   Cortex    │
    │  (Borda)    │◄─────────►│  (Núcleo)   │◄─────────►│  (Borda)    │
    │  nós leves  │  (links   │ nós pesados │  (links   │  nós leves  │
    └─────────────┘   P2P)    └──────┬──────┘   P2P)    └─────────────┘
                                     ▼
                            ┌──────────────┐
                            │  Spore Bank  │
                            │ (DHT/estado) │
                            └──────────────┘
```

## Substrato (crates `mycelium-*`)

| Crate | Papel |
|---|---|
| `mycelium-core` | Tipos compartilhados: `NodeId`, `Resource`, `Spore`, trait `FruitingBody` |
| `mycelium-hyphae` | Rede P2P real: libp2p com QUIC, Kademlia DHT, gossipsub e mDNS. Anastomose = fusão de rotas quando duas hifas se encontram |
| `mycelium-pheromones` | Identidade ed25519: `scent` (reputação), `trail` (histórico assinado), `decay` (TTL), `alarm` (onda de perigo) |
| `mycelium-nutrients` | Ledger local da economia bioquímica: ATP, Enzymes, Mycelia, Spores, Resilience |

## Fluxos de dados no micélio

| Cenário | Fluxo |
|---|---|
| Developer pusha código | `giggs::Plot` → hifa → `thefield` → gossip → nós vizinhos replicam |
| CI/CD dispara | `thefield::Signal` → ressonância atinge quórum → `inertia::Vector` injetado em `plasma::Ion` |
| App containerizada sobe | `plasma::Ion` solicita Void → `vacuum` suga dependências via hifas → `Chamber` nasce em nó com recursos |
| Consulta a banco | App → `isotope::Decay` → propaga por hifas → Nuclei vizinhos respondem → fusão eventual |
| Segredo é acessado | App → `entropy` Chaos Key → M de N `Shade`s coletadas por hifas → segredo existe por meia-vida |
| Requisição externa chega | Internet → `singularity` Event Horizon → roteada por rizomorfos → chega ao Ion correto |

## Economia (Nutrient Cycling)

| Recurso contribuído | Recompensa |
|---|---|
| CPU cycles (Vectors do Inertia) | **ATP** — tokens de energia |
| RAM (Chambers do Vacuum) | **Enzymes** — prioridade de acesso |
| Storage (Nuclei do Isotope, Voids do Vacuum) | **Mycelia** — direitos de armazenamento futuro |
| Bandwidth (hifas, Singularity) | **Spores** — reputação e governança |
| Uptime (Shades do Entropy) | **Resilience** — imunidade a banimento |

Não há cobrança em dinheiro fiat: quem alimenta a rede é alimentado pela rede.

## Estado da implementação

Protótipo funcional: hifas com mDNS (opt-out `--no-mdns`) + seed book + DHT + circuit relay v2 (`--relay` / client `/p2p-circuit`); Vacuum com layers em disco/DHT (`layer/`) + gossip `LayerNeed`/`LayerOffer`; Inertia local e remoto (`VectorOffer` → `MomentumReport`); Deploy só no origin do Signal; Isotope com `nucleus.json` + `AtomSync`; Singularity HTTP + `/console` + rate-limit; Envelope `v:1` (`docs/protocol.md`); control socket com `MYCELIUM_CONTROL_TOKEN` (obrigatório em `--relay`); seed com `--announce-ip` + `scripts/install-seed.sh`. Catálogo: `seeds/mainnet.txt`. CI: unitários + demos de integração.
