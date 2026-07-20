# Política de Membrana (Rizomorfos sem VPS)

Regra de ouro: **IPv6 é floresta** (hifa direta). **IPv4 é solo compactado** — ou raiz com port-forward explícito, ou folha outbound-only. O Mycelium **não** faz STUN, hole punch (DCUtR) nem UPnP.

| Estado | Condição | Listen | Papel |
|--------|----------|--------|-------|
| **Floresta** | IPv6 global detectado | `[::]:…` + loopback v4 | Aceita hifa direta IPv6 |
| **Raiz** | `--announce-ip` (port-forward declarado) | `0.0.0.0` + `[::]` se houver | Aceita IPv4 externo |
| **Folha** | NAT sem announce | `127.0.0.1` (+ `[::]` se global) | Só inicia conexões; usa esporocarp |
| **Esporocarp** | `--sporocarp` | dual-stack + relay server | Ponte / ATP por circuito |

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

## Variáveis de ambiente

| Env | Papel |
|---|---|
| `MYCELIUM_ANNOUNCE_IP` | Declara raiz IPv4 (port-forward 4001) |
| `MYCELIUM_ANNOUNCE_IP6` | IPv6 anunciado |
| `MYCELIUM_DNS_SEEDS` | Nome DNS TXT do Spore Bank |
| `DUCKDNS_TOKEN` / `DUCKDNS_DOMAIN` | Publish TXT no esporocarp |
| `MYCELIUM_BOOTSTRAP_URL` | Catálogo HTTP alternativo |
| `MYCELIUM_CONTROL_TOKEN` | Auth do socket (obrigatório em relay/sporocarp) |

## CLI

```bash
# Folha (auto): outbound + relay client
mycelium daemon --public-bootstrap --no-mdns

# Floresta / esporocarp doméstico
mycelium daemon \
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

`mycelium status` inclui `membrana`, `sporocarp`, `dns_seed`.

## Notas

- Crédito ATP/Spores por circuito no esporocarp é simbólico (por evento; débito na folha = backlog).
- Feromônio carrega `membrane` no corpo (default `folha` em pacotes legados).
