# Operar um seed público

## Requisitos

- Porta **TCP 4001** (e idealmente UDP 4001 para QUIC) aberta no firewall / NAT
- Binário `mycelium` (`cargo install --path cli/mycelium-cli`)

## Subir

```bash
export MYCELIUM_HOME=/var/lib/mycelium-seed
export MYCELIUM_ANNOUNCE_IP=$(curl -4 -s ifconfig.me)   # IP público
export MYCELIUM_CONTROL_TOKEN=$(openssl rand -hex 16)   # opcional

mycelium sprout --contribute 2cpu,4gb,100gb
mycelium daemon \
  --listen /ip4/0.0.0.0/tcp/4001 \
  --announce-ip "$MYCELIUM_ANNOUNCE_IP" \
  --no-mdns \
  --horizon-port 7474 \
  --contribute 2cpu,4gb,100gb
```

Systemd: veja `deploy/mycelium-seed.service`.

## Publicar no catálogo

```bash
./scripts/export-seed.sh "$MYCELIUM_HOME"
# cole a linha TCP+/p2p/ em seeds/mainnet.txt, commit e push
```

Clientes:

```bash
mycelium daemon --public-bootstrap --no-mdns
# ou
mycelium daemon --seed-file ./seeds/mainnet.txt --no-mdns
```

## NAT em casa

Sem port-forward, peers externos não alcançam o seed. Encaminhe TCP 4001 → máquina do seed no roteador, ou use um VPS com IP limpo.
