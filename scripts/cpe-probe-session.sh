#!/usr/bin/env bash
# Sessão sensor completa: tcpdump + probe via adb (5G) + análise SYN.
# Corre no TushiBook COM o telemóvel em adb e rede 5G (não Wi‑Fi da casa).
#
# Uso: ./scripts/cpe-probe-session.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT_DIR="${MYCELIUM_SENSOR_DIR:-/tmp/mycelium-sensor}"
mkdir -p "$OUT_DIR"
TS="$(date -u +%Y%m%dT%H%M%SZ)"
PCAP="$OUT_DIR/cpe-$TS.pcap"
LOG="$OUT_DIR/session-$TS.log"

IP4="$(curl -4 -sS --max-time 5 ifconfig.me 2>/dev/null || true)"
IP6="$(curl -6 -sS --max-time 5 ifconfig.co 2>/dev/null || true)"

exec > >(tee -a "$LOG") 2>&1

echo "=== cpe-probe-session $TS ==="
echo "IPv4 aparente: ${IP4:-none}"
echo "IPv6 aparente: ${IP6:-none}"

if ! command -v tcpdump >/dev/null; then
  echo "A instalar tcpdump (sudo) — sem apt-get update (evita repos partidos)…"
  if ! sudo apt-get install -y tcpdump; then
    echo "ERRO: não instalou tcpdump. Repo apt provavelmente partido (ex. nvidia unsigned)." >&2
    echo "Corrige com um destes:" >&2
    echo "  sudo apt-get install -y tcpdump" >&2
    echo "  # ou desactiva o repo nvidia em /etc/apt/sources.list.d/ e tenta de novo" >&2
    echo "  # ou: sudo apt-get install -y ./tcpdump_*.deb  (pacote local)" >&2
    exit 1
  fi
fi

if ! adb devices 2>/dev/null | grep -q $'\tdevice$'; then
  echo "ERRO: nenhum device adb. Liga o telemóvel (USB debugging) e:"
  echo "  adb devices"
  exit 1
fi
echo "adb: $(adb devices | awk '/device$/{print $1}')"

# Preferir dados móveis — aviso se estiver em Wi‑Fi da LAN
WIFI="$(adb shell dumpsys connectivity 2>/dev/null | grep -i 'WIFI' | head -3 || true)"
echo "connectivity hint: ${WIFI:-(verifica manualmente que estás em 5G, não Wi‑Fi casa)}"

echo "A iniciar tcpdump → $PCAP"
sudo tcpdump -ni any -w "$PCAP" \
  'tcp port 4001 or udp port 4001 or udp port 4002 or tcp port 443 or icmp or icmp6' &
TPID=$!
sleep 2

cleanup() {
  echo "A parar tcpdump ($TPID)…"
  sudo kill "$TPID" 2>/dev/null || true
  wait "$TPID" 2>/dev/null || true
}
trap cleanup EXIT

probe_phone() {
  local host="$1" port="$2" label="$3"
  echo
  echo "--- probe $label $host:$port via adb ---"
  # toybox nc no Android
  if adb shell "command -v nc >/dev/null && nc -z -w 5 $host $port" 2>/dev/null; then
    echo "PHONE nc: ok"
    "$ROOT/scripts/probe-sporocarp.sh" "$host" "$port" "adb-phone" | tee "$OUT_DIR/proof-${label}-$TS.json" || true
  else
    # fallback: probe a partir do PC não vale para inbound WAN (hairpin)
    echo "PHONE nc: fail ou nc ausente — a gravar probe do host (só referência)"
    adb shell "ping -c 1 -W 3 $host" 2>/dev/null | head -5 || true
    set +e
    "$ROOT/scripts/probe-sporocarp.sh" "$host" "$port" "from-lan-NOT-valid" | tee "$OUT_DIR/proof-${label}-lanref-$TS.json"
    set -e
  fi
}

[[ -n "$IP4" ]] && probe_phone "$IP4" 4001 ipv4-4001
[[ -n "$IP4" ]] && probe_phone "$IP4" 443 ipv4-443
# IPv6 no Android: brackets
if [[ -n "$IP6" ]]; then
  echo
  echo "--- probe ipv6 $IP6:4001 via adb ---"
  if adb shell "nc -z -w 5 $IP6 4001" 2>/dev/null; then
    echo "PHONE nc6: ok"
  else
    echo "PHONE nc6: fail"
  fi
  set +e
  "$ROOT/scripts/probe-sporocarp.sh" "$IP6" 4001 "adb-phone-v6" | tee "$OUT_DIR/proof-ipv6-4001-$TS.json"
  set -e
fi

sleep 1
cleanup
trap - EXIT

echo
echo "=== análise SYN ==="
sudo tcpdump -nr "$PCAP" 'tcp[tcpflags] & tcp-syn != 0' 2>/dev/null | head -40 || \
  tcpdump -nr "$PCAP" 'tcp[tcpflags] & tcp-syn != 0' 2>/dev/null | head -40 || true

SYN_COUNT=$(sudo tcpdump -nr "$PCAP" 'tcp[tcpflags] & tcp-syn != 0 and tcp[tcpflags] & tcp-ack == 0' 2>/dev/null | wc -l || echo 0)
echo "SYN (sem ACK) count≈ $SYN_COUNT"
echo
echo "Interpretação:"
echo "  probe fail + SYN=0 → pacote morre antes do host (CGNAT/CPE/upstream)"
echo "  probe fail + SYN>0 → chega ao host; ver SYN-ACK / firewall local"
echo
echo "PCAP: $PCAP"
echo "LOG:  $LOG"
echo "Actualiza docs/mapa_cpe_vivo.md com estes resultados."
