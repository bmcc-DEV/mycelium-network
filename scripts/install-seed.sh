#!/usr/bin/env bash
# Instala mycelium-seed como Volunteer Sporocarp (systemd 24/7, sem VPS).
# Uso: sudo MYCELIUM_REACHABLE=1 ./scripts/install-seed.sh [--announce-ip IP] [--announce-ip6 IP6]
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN_SRC="${MYCELIUM_BIN:-$ROOT/target/release/mycelium}"
ANNOUNCE_IP=""
ANNOUNCE_IP6=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --announce-ip) ANNOUNCE_IP="$2"; shift 2 ;;
    --announce-ip6) ANNOUNCE_IP6="$2"; shift 2 ;;
    *) echo "uso: $0 [--announce-ip IP] [--announce-ip6 IP6]"; exit 1 ;;
  esac
done

if [[ "$(id -u)" -ne 0 ]]; then
  echo "rode como root: sudo MYCELIUM_REACHABLE=1 $0" >&2
  exit 1
fi

if [[ "${MYCELIUM_REACHABLE:-}" != "1" && "${MYCELIUM_REACHABLE:-}" != "true" ]]; then
  echo "ERRO: defina MYCELIUM_REACHABLE=1 só depois de ./scripts/verify-sporocarp.sh" >&2
  echo "Docs: docs/volunteer-sporocarp.md" >&2
  exit 1
fi

if [[ ! -x "$BIN_SRC" ]]; then
  echo "binário não encontrado: $BIN_SRC" >&2
  echo "rode: cargo build -p mycelium-cli --release" >&2
  exit 1
fi

if [[ -z "$ANNOUNCE_IP" ]]; then
  ANNOUNCE_IP="$(curl -4 -fsS ifconfig.me 2>/dev/null || true)"
fi
if [[ -z "$ANNOUNCE_IP6" ]]; then
  ANNOUNCE_IP6="$(curl -6 -fsS ifconfig.co 2>/dev/null || true)"
fi
if [[ -z "$ANNOUNCE_IP" && -z "$ANNOUNCE_IP6" ]]; then
  echo "defina --announce-ip e/ou --announce-ip6 (IP público)" >&2
  exit 1
fi

id mycelium &>/dev/null || useradd -r -s /usr/sbin/nologin -d /var/lib/mycelium-seed mycelium
mkdir -p /var/lib/mycelium-seed /etc/mycelium
chown mycelium:mycelium /var/lib/mycelium-seed

install -m 0755 "$BIN_SRC" /usr/local/bin/mycelium

TOKEN="$(openssl rand -hex 16 2>/dev/null || head -c 16 /dev/urandom | xxd -p)"
cat >/etc/mycelium/seed.env <<EOF
MYCELIUM_HOME=/var/lib/mycelium-seed
MYCELIUM_ANNOUNCE_IP=$ANNOUNCE_IP
MYCELIUM_ANNOUNCE_IP6=$ANNOUNCE_IP6
MYCELIUM_REACHABLE=1
MYCELIUM_CONTROL_TOKEN=$TOKEN
RUST_LOG=info
# Opcional — Spore Bank DNS vivo (não é VPS):
# DUCKDNS_TOKEN=
# DUCKDNS_DOMAIN=
EOF
chmod 600 /etc/mycelium/seed.env
chown root:mycelium /etc/mycelium/seed.env

install -m 0644 "$ROOT/deploy/mycelium-seed.service" /etc/systemd/system/mycelium-seed.service

sudo -u mycelium env MYCELIUM_HOME=/var/lib/mycelium-seed \
  /usr/local/bin/mycelium sprout --contribute 2cpu,4gb,100gb || true

systemctl daemon-reload
systemctl enable --now mycelium-seed

echo
echo "OK — mycelium-seed (sporocarp voluntário) ativo"
echo "  announce v4 : ${ANNOUNCE_IP:-—}"
echo "  announce v6 : ${ANNOUNCE_IP6:-—}"
echo "  token       : /etc/mycelium/seed.env"
echo "  status      : systemctl status mycelium-seed"
echo "  export      : $ROOT/scripts/export-seed.sh /var/lib/mycelium-seed"
echo "  verify      : $ROOT/scripts/verify-sporocarp.sh ${ANNOUNCE_IP:-HOST} 4001"
echo "  docs        : docs/volunteer-sporocarp.md"
