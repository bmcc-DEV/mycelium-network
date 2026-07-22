# Nostr Transport — libp2p sobre CandidateRelay

Faz uma folha CGNAT obter `vizinhos >= 1` **sem** esporocarpo, usando relays Nostr públicos como transporte.

## Princípio

O backchannel (kind 39406) é um canal bidirecional outbound (WSS). O crate `mycelium-nostr-transport` expõe-o como `Transport` libp2p; o Swarm aplica Noise + Yamux em cima (como TCP).

```text
Aplicação / Kademlia / GossipSub
        ↓
   Noise + Yamux   (stack libp2p)
        ↓
   NostrTransport  (AsyncRead/Write + seq/ACK)
        ↓
   kind 39401 descoberta + kind 39406 dados
        ↓
   wss://relay…   (outbound — CGNAT OK)
```

**Invariante intacto:** isto **não** é `/esporocarp`. Não anuncia relay circuit. Não exige `MYCELIUM_REACHABLE`.

## Multiaddr

Forma lógica: `/nostr/<relay>/<ghost_hex>`

Codificação actual (multicodec `/nostr` ainda não existe):

```text
/unix/mycelium-nostr/<relay_url_hex>/<ghost_hex64>
```

Helpers: `encode_nostr_multiaddr` / `parse_nostr_multiaddr` / `listen_multiaddr`.

## Uso

```bash
# Build (default CLI já inclui nostr-transport)
cargo build -p mycelium-cli --release

# Folha/Floresta: Nostr transport liga-se sozinho (auto).
MYCELIUM_HOME=/tmp/folha-a mycelium daemon --no-mdns

# Forçar ON / OFF:
mycelium daemon --nostr-transport
mycelium daemon --no-nostr-transport

# Smoke dois homes:
./scripts/nostr-transport-smoke.sh
```

Env: `MYCELIUM_NOSTR_TRANSPORT=1` (força ON), `MYCELIUM_NOSTR_RELAY=wss://nos.lol`.

GhostID estável em `{home}/candidate.session` (TTL ~1h). Re-announce a cada ~60s;
discovery filtra peers stale e prefere anúncios `listen:`.

## Limitações (MVP)

- Latência 200–400 ms via relay público
- Rate-limit dos relays — só controlo/gossip leve, não sync grande
- DCUtR / hole punch: fora deste MVP
- GhostID-por-pacote HD: fora deste MVP

## Relação com CandidateRelay CLI

| Comando | Papel |
|---------|--------|
| `mycelium candidate listen/send` | chat backchannel sem Swarm |
| `mycelium daemon` (folha/floresta) | mesh libp2p (`vizinhos`) via Nostr |

Ver [`candidate-relay.md`](candidate-relay.md).
