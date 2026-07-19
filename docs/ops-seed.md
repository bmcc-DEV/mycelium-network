# Operar um seed público

## Requisitos

- Porta **TCP 4001** (e idealmente UDP 4001 para QUIC) aberta no firewall / NAT
- Binário `mycelium` (`cargo install --path cli/mycelium-cli`)

## Install systemd (recomendado)

```bash
cargo build -p mycelium-cli --release
sudo ./scripts/install-seed.sh --announce-ip $(curl -4 -s ifconfig.me)
```

Isso cria user `mycelium`, `/etc/mycelium/seed.env` (com `MYCELIUM_CONTROL_TOKEN`), unit com `--relay`, e `systemctl enable --now mycelium-seed`.

## Subir manual

```bash
export MYCELIUM_HOME=/var/lib/mycelium-seed
export MYCELIUM_ANNOUNCE_IP=$(curl -4 -s ifconfig.me)
export MYCELIUM_CONTROL_TOKEN=$(openssl rand -hex 16)

mycelium sprout --contribute 2cpu,4gb,100gb
mycelium daemon \
  --listen /ip4/0.0.0.0/tcp/4001 \
  --announce-ip "$MYCELIUM_ANNOUNCE_IP" \
  --no-mdns \
  --relay \
  --horizon-port 7474 \
  --contribute 2cpu,4gb,100gb
```

Com `--relay`, o seed aceita circuites v2. Sem `MYCELIUM_CONTROL_TOKEN`, o daemon gera `{home}/control.token`.

Event Horizon fica em `127.0.0.1:7474` (não exponha sem proxy). Rate-limit: 120 req/min por IP.

## Publicar no catálogo

```bash
./scripts/export-seed.sh "$MYCELIUM_HOME"
# cole a linha TCP+/p2p/ em seeds/mainnet.txt, commit e push
```

Clientes (escutam via `/p2p-circuit` no seed):

```bash
mycelium daemon --public-bootstrap --no-mdns
# ou
mycelium daemon --seed-file ./seeds/mainnet.txt --no-mdns
```

Multiaddr relayed típico: `/ip4/SEED_IP/tcp/4001/p2p/SEED_PEER/p2p-circuit/p2p/LOCAL_PEER`

## NAT em casa

Sem port-forward, peers externos não alcançam o seed. Encaminhe TCP 4001 → máquina do seed no roteador, ou use um VPS com IP limpo. Relay ajuda peers só-NAT a se conectarem **através** do seed público.
