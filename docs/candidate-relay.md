# CandidateRelay — terceiro estado (kind 39401)

Quebra o ponto fixo CGNAT (`peers = 0`) **sem** violar o invariante de membrana.

```text
/esporocarp      ⇔ MYCELIUM_REACHABLE   (permanente, proof.json)
/candidate-relay ⇔ Nostr outbound       (temporário, TTL ~5 min, sem proof)
/folha           ⇔ nada                 (0 peers libp2p)
```

## Problema

CGNAT ⇒ inbound = ∅ ⇒ sem `/esporocarp` ⇒ sem circuit relay ⇒ 0 peers.
O operador Bellman `(Bφ)_i = sup_j(b_ij + φ_j)` fica em −∞. Ponto fixo estável.

Mailbox (`sow`/`recall --hybrid`, kind 31234) já funciona. **Mesh live** entre duas folhas CGNAT precisa de rendezvous — CandidateRelay.

## Algoritmo (P0)

```text
Cada 30–300s (jitter):
  1. GhostID efémero (secp256k1 / Schnorr)
  2. Publicar kind 39401 (expires=now+300, backchannel=relay)
  3. Subscrever kinds=[39401] recentes
  4. Verificar sig + TTL; handshake com tag #p
  5. Derivar shared secret de sessão (binding P0)
  6. Prune expirados
```

Nenhuma referência à identidade permanente (`gland.seed` / PeerId).

## CLI

```bash
# Descoberta (kind 39401)
mycelium candidate
mycelium candidate --loop

# Sessão estável para mensagens (grava {home}/candidate.session)
mycelium candidate whoami

# Terminal A — escuta + re-anuncia
MYCELIUM_HOME=/tmp/cand-a mycelium candidate listen --loop

# Terminal B — envia (usa o ghost impresso pelo whoami/listen de A)
MYCELIUM_HOME=/tmp/cand-b mycelium candidate send --to <ghost_A> -m "ola da folha B"

# Novo GhostID
mycelium candidate reset
```

**Importante:** duas sessões no mesmo PC precisam de `MYCELIUM_HOME` diferentes (cada uma tem o seu `candidate.session`).

Métrica: `candidate_peers > 0` **não** promove a membrana a esporocarp.

## Kinds QEL (reserva)

| Kind | Nome |
|------|------|
| 39400 | QEL_PRESENCE |
| **39401** | **QEL_CANDIDATE_RELAY** (descoberta) |
| **39406** | **QEL_BACKCHANNEL** (NIP-44, tag `#p`) |
| 31234 | QEL_SHARD mailbox (já em produção) |

## Roadmap

| Prioridade | Item | Estado |
|------------|------|--------|
| P0 | Announce + discover + handshake | ✅ `mycelium candidate` |
| P0 | Backchannel cifrado bidirecional | ✅ `listen` / `send` (NIP-44) |
| P0 | Transporte libp2p sobre Nostr | ✅ `mycelium-nostr-transport` + `--nostr-transport` |
| P1 | Hole punching UDP (Ford 2005) | pendente |
| P2 | Keep-alive NAT | pendente |
| P3 | LoRa / SMS / DTN DistanceBridge | pendente |

Doc do transporte: [`nostr-transport.md`](nostr-transport.md).


## Riscos

1. **CGNAT simétrico UDP** — hole punching pode falhar; fallback = Nostr WS (~50 Kbps ok para texto/tx).
2. **Bootstrapping frio** — primeira folha sozinha mantém ponto fixo; precisa de ≥2 folhas ou “esporocarpo verde” (IP público a publicar 39401 **sem** fingir `/esporocarp`).
3. **Latência relay** — 200–400 ms Europa; mitigar com hole punch ou relays LATAM.

## Relação com esporocarpo voluntário

| | CandidateRelay | Esporocarpo |
|--|----------------|-------------|
| Proof inbound | Não | Sim (`proof.json`) |
| Membrana | permanece folha | `/esporocarp` |
| Uso | descoberta + backchannel Nostr | circuit relay libp2p |
| CGNAT Vivo | sim | não (folha) |

Ver [`invariante_membrana.md`](invariante_membrana.md), [`nostr-qel.md`](nostr-qel.md), [`volunteer-sporocarp.md`](volunteer-sporocarp.md).
