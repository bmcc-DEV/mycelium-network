# Política de Membrana (Rizomorfos sem VPS)

Regra de ouro: **IPv6 é floresta** quando a hifa directa funciona. **IPv4 é solo compactado** — ou raiz com port-forward explícito, ou folha outbound-only.

O Mycelium **não** faz UPnP. STUN **público** (como DNS) e DCUtR **só sobre circuito já estabelecido** com um peer `/esporocarp` do mesh são opcionais/experimentais — **não** substituem um peer já alcançável no bootstrap.

Guia do próximo marco: [`volunteer-sporocarp.md`](volunteer-sporocarp.md) ·
ciclo operacional: [`engenharia-reversa-bloqueio.md`](engenharia-reversa-bloqueio.md).

| Estado | Condição | Listen | Papel |
|--------|----------|--------|-------|
| **Floresta** | IPv6 global (sem prova de inbound WAN) | `[::]:…` + loopback v4 | Hifa local/directa se o peer alcançar; **não** auto-relay WAN |
| **Raiz** | `--announce-ip` (port-forward declarado) | `0.0.0.0` + `[::]` se houver | Aceita IPv4 externo se o forward existir |
| **Folha** | NAT sem announce | `127.0.0.1` (+ `[::]` se global) | Só inicia conexões; usa esporocarp |
| **Esporocarp** | `--sporocarp` **ou** (`--assume-reachable`/`MYCELIUM_REACHABLE=1` + IPv6/announce) | dual-stack + relay server | Ponte / ATP por circuito |

## Alcançabilidade WAN (obrigatória para esporocarp útil)

Ter IPv6 global na interface **não** prova inbound. ISPs BR (ex. Vivo CPE) podem deixar ICMP passar e **dropar todo TCP SYN** (incl. :443).

- Declare inbound verificado: `MYCELIUM_REACHABLE=1` ou `--assume-reachable`.
- Sem isso, `--sporocarp` ainda liga relay, mas o daemon **avisa** que o TXT pode anunciar um nó morto para o mundo.
- Diagnóstico: `./scripts/mycelium-diagnose.sh`

Bootstrap (DHT/gossip/hole-punch) **sempre** precisa de pelo menos um peer já alcançável: mDNS, `seeds.txt`, ou DNS TXT Spore Bank.

## Bootstrap (Spore Bank)

1. CLI / `seeds.txt` local  
2. **DNS TXT** (`MYCELIUM_DNS_SEEDS` ou `_mycelium.seeds.duckdns.org`)  
3. HTTP legado (`--public-bootstrap`)

Formato TXT (flag opcional):

```text
/ip6/2001:db8::1/tcp/4001/p2p/12D3KooW…/floresta
/ip4/203.0.113.5/tcp/4001/p2p/12D3KooW…/raiz
/ip6/2001:db8::2/tcp/4001/p2p/12D3KooW…/esporocarp
```

Folhas priorizam `/esporocarp`. Ninguém diala seeds `/folha`.

## Relay mesh (sem VPS)

Qualquer nó **com inbound verificado** pode ser esporocarp. Anúncios de relay no gossip `/mycelium/relay-mesh/v1` e DHT `/mycelium/relays/` só para peers que passaram no critério de alcançabilidade. Folhas dialam `/p2p-circuit` quando o dial directo falha.

STUN público (Google/Cloudflare/…) é **infraestrutura partilhada**, não um VPS Mycelium — substituível; não torna o teu CPE um esporocarp.

## WebRTC experimental

Build opcional: `cargo build -p mycelium-cli --features webrtc`. Liga `libp2p-webrtc` 0.9 (**webrtc-direct**, sem API de STUN no crate — a lista pública em `PUBLIC_STUN_SERVERS` é documentação/diagnose).

```bash
cargo run -p mycelium-cli --features webrtc -- daemon --webrtc --webrtc-port 4002
```

## Variáveis de ambiente

| Env | Papel |
|---|---|
| `MYCELIUM_ANNOUNCE_IP` | Declara raiz IPv4 (port-forward 4001) |
| `MYCELIUM_ANNOUNCE_IP6` | IPv6 público anunciado |
| `MYCELIUM_REACHABLE` | `1` = inbound verificado → auto-esporocarp se IPv6/announce |
| `MYCELIUM_DNS_SEEDS` | Nome DNS TXT do Spore Bank |
| `DUCKDNS_TOKEN` / `DUCKDNS_DOMAIN` | Publish TXT no esporocarp |
| `MYCELIUM_BOOTSTRAP_URL` | Catálogo HTTP alternativo |
| `MYCELIUM_CONTROL_TOKEN` | Auth do socket (obrigatório em relay/sporocarp) |

## CLI

```bash
# Diagnóstico
./scripts/mycelium-diagnose.sh

# Folha (auto): outbound + relay client
mycelium daemon --public-bootstrap --no-mdns

# Esporocarp com inbound confirmado (amigo / uni / port-forward real)
MYCELIUM_REACHABLE=1 mycelium daemon \
  --sporocarp \
  --listen /ip6/::/tcp/4001 \
  --listen /ip4/0.0.0.0/tcp/4001 \
  --announce-ip6 2001:db8::1 \
  --no-mdns

# Raiz IPv4 legada (port-forward manual no roteador)
mycelium daemon --membrane raiz --announce-ip 203.0.113.5 \
  --listen /ip4/0.0.0.0/tcp/4001 --no-mdns
```

`--sporocarp` implica relay + publish DuckDNS (~5 min). `--upnp` é **ignorado**.

## Status

`mycelium status` inclui `membrana`, `sporocarp`, `wan_reach`, `is_relay`, `relay_mesh`, `dns_seed`.

## Notas

- Crédito ATP/Spores por circuito no esporocarp é simbólico (por evento; débito na folha = backlog).
- Feromônio carrega `membrane` no corpo (default `folha` em pacotes legados).
- Mailbox DHT (`/mycelium/mailbox/`) entrega mensagens assíncronas **após** bootstrap na DHT.
