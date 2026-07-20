# Operar um seed / Sporocarp doméstico

O seed público **não exige VPS**. Preferência: **Volunteer Sporocarp** com IPv6 (floresta) ou port-forward IPv4 explícito (raiz). Sem UPnP/STUN — ver [`rizomorphs.md`](rizomorphs.md).

## Requisitos

- Ideal: **IPv6** nativo no ISP (floresta)
- Ou **port-forward TCP/UDP 4001** declarado com `--announce-ip` (raiz)
- Binário `mycelium` (`cargo install --path cli/mycelium-cli`)

## Sporocarp em casa (recomendado)

```bash
export MYCELIUM_HOME=/var/lib/mycelium-seed
export MYCELIUM_ANNOUNCE_IP6=$(curl -6 -s ifconfig.co)   # se tiver IPv6
# Se só IPv4 com port-forward:
# export MYCELIUM_ANNOUNCE_IP=$(curl -4 -s ifconfig.co)
export DUCKDNS_TOKEN=seu-token
export DUCKDNS_DOMAIN=meuspores
export MYCELIUM_CONTROL_TOKEN=$(openssl rand -hex 16)

mycelium sprout --contribute 2cpu,4gb,100gb
mycelium daemon \
  --listen /ip6/::/tcp/4001 \
  --listen /ip4/0.0.0.0/tcp/4001 \
  --sporocarp \
  --no-mdns \
  --horizon-port 7474 \
  --contribute 2cpu,4gb,100gb
```

Com `--sporocarp`:

- membrana **esporocarp** + relay server (circuit v2)
- publish periódico do melhor multiaddr + `/esporocarp` no DuckDNS TXT (se `DUCKDNS_*`)
- crédito ATP/Spores por circuito aceito

Sem `MYCELIUM_CONTROL_TOKEN`, o daemon gera `{home}/control.token`.

## Install systemd (VPS ou casa)

```bash
sudo ./scripts/install-seed.sh
```

O unit legado usa `--relay`. Para sporocarp, ajuste o service para `--sporocarp` e `DUCKDNS_*` em `/etc/mycelium/seed.env`.

## Clientes (folhas)

```bash
mycelium daemon --public-bootstrap --no-mdns
```

Folhas priorizam seeds `/esporocarp` e não dialam `/folha`.

## Catálogo

**DNS (preferido):** o sporocarp atualiza DuckDNS sozinho.

**HTTP:** `seeds/mainnet.txt` no GitHub via `--public-bootstrap`.

## Filosofia

IPv6 direto → esporocarp relay. IPv4 só com port-forward declarado. Sem “enganar” o NAT.
