#!/usr/bin/env bash
# Instala mycelium-seed como serviço systemd 24/7.
# Uso: sudo ./scripts/install-seed.sh [--announce-ip IP]
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN_SRC="${MYCELIUM_BIN:-$ROOT/target/release/mycelium}"
ANNOUNCE_IP=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --announce-ip) ANNOUNCE_IP="$2"; shift 2 ;;
    *) echo "uso: $0 [--announce-ip IP]"; exit 1 ;;
  esac
done

if [[ "$(id -u)" -ne 0 ]]; then
  echo "rode como root: sudo $0" >&2
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
if [[ -z "$ANNOUNCE_IP" ]]; then
  echo "defina --announce-ip (IP público)" >&2
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
MYCELIUM_CONTROL_TOKEN=$TOKEN
RUST_LOG=info
EOF
chmod 600 /etc/mycelium/seed.env
chown root:mycelium /etc/mycelium/seed.env

install -m 0644 "$ROOT/deploy/mycelium-seed.service" /etc/systemd/system/mycelium-seed.service

# Sprout inicial (identidade persistente).
sudo -u mycelium env MYCELIUM_HOME=/var/lib/mycelium-seed \
  /usr/local/bin/mycelium sprout --contribute 2cpu,4gb,100gb || true

systemctl daemon-reload
systemctl enable --now mycelium-seed

echo
echo "OK — mycelium-seed ativo"
echo "  announce : $ANNOUNCE_IP"
echo "  token    : /etc/mycelium/seed.env (MYCELIUM_CONTROL_TOKEN)"
echo "  status   : systemctl status mycelium-seed"
echo "  seed     : sudo -u mycelium MYCELIUM_HOME=/var/lib/mycelium-seed MYCELIUM_CONTROL_TOKEN=\$TOKEN mycelium status"
echo "  export   : $ROOT/scripts/export-seed.sh /var/lib/mycelium-seed"
