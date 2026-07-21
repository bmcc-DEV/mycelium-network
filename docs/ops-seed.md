# Operar um seed / Volunteer Sporocarp (zero VPS)

O seed público **não exige VPS**. Preferência: peer voluntário com inbound
**verificado** (`MYCELIUM_REACHABLE=1`). Casa atrás de CGNAT/firewall ISP
(ex. Vivo residencial) é folha — ver [`volunteer-sporocarp.md`](volunteer-sporocarp.md)
e [`rizomorphs.md`](rizomorphs.md). Sem UPnP.

## Fluxo rápido (voluntário)

```bash
# 1) No peer: port-forward TCP(+UDP) 4001, daemon a escutar 0.0.0.0
# 2) De OUTRA rede (5G):
./scripts/verify-sporocarp.sh IP_PUBLICO 4001

# 3) Só se ok:
MYCELIUM_REACHABLE=1 ./scripts/run-public-seed.sh
# ou 24/7:
sudo MYCELIUM_REACHABLE=1 ./scripts/install-seed.sh \
  --announce-ip IPV4 --announce-ip6 IPV6

# 4) Exportar linha /esporocarp → seeds/mainnet.txt ou DuckDNS TXT
./scripts/export-seed.sh ~/.local/share/mycelium-seed
```

## Sporocarp manual

```bash
export MYCELIUM_HOME=/var/lib/mycelium-seed
export MYCELIUM_REACHABLE=1
export MYCELIUM_ANNOUNCE_IP6=$(curl -6 -s ifconfig.co)   # se IPv6 inbound ok
# ou IPv4 com port-forward:
# export MYCELIUM_ANNOUNCE_IP=$(curl -4 -s ifconfig.co)
export DUCKDNS_TOKEN=seu-token          # opcional
export DUCKDNS_DOMAIN=meuspores
export MYCELIUM_CONTROL_TOKEN=$(openssl rand -hex 16)

mycelium sprout --contribute 2cpu,4gb,100gb
mycelium daemon \
  --listen /ip6/::/tcp/4001 \
  --listen /ip4/0.0.0.0/tcp/4001 \
  --assume-reachable \
  --sporocarp \
  --no-mdns \
  --horizon-port 7474 \
  --contribute 2cpu,4gb,100gb
```

Com `--sporocarp` + reachable:

- membrana **esporocarp** + relay server (circuit v2)
- anúncio mesh `/mycelium/relay-mesh/v1` (só se `wan_reach`)
- publish DuckDNS TXT com `/esporocarp` (se `DUCKDNS_*`)

## Folhas (casa / 5G)

```bash
mycelium daemon --seed-file ./seeds/mainnet.txt --no-mdns
# ou: --public-bootstrap  (DNS TXT Spore Bank)
```

Folhas priorizam `/esporocarp`. Não dialam `/folha`.

## Catálogo

- **DNS:** DuckDNS TXT do voluntário  
- **HTTP/git:** `seeds/mainnet.txt` via `--public-bootstrap` / `--seed-file`

## Filosofia

Inbound verificado → esporocarp. IPv6 na interface ≠ alcançável. Sem “enganar” o NAT.
