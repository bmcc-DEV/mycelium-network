#!/usr/bin/env bash
# Sensor CPE — medições locais (TushiBook como folha/sensor, NÃO esporocarpo).
# Uso: ./scripts/cpe-sensor.sh [--pcap]
# Com --pcap: imprime comando tcpdump e espera Enter (não captura sozinho — precisa sudo).
set -euo pipefail

OUT_DIR="${MYCELIUM_SENSOR_DIR:-/tmp/mycelium-sensor}"
mkdir -p "$OUT_DIR"
TS="$(date -u +%Y%m%dT%H%M%SZ)"
REPORT="$OUT_DIR/report-$TS.txt"

{
  echo "Mycelium CPE sensor — $TS"
  echo "================================"
  echo
  echo "--- IPv4 locais ---"
  ip -4 addr show scope global 2>/dev/null | awk '/inet /{print $2}' || true
  echo
  echo "--- IPv6 globais ---"
  ip -6 addr show scope global 2>/dev/null | awk '/inet6 /{print $2}' || true
  echo
  echo "--- Gateway ---"
  ip route | awk '/default/{print}' || true
  echo
  IP4="$(curl -4 -sS --max-time 5 ifconfig.me 2>/dev/null || true)"
  IP6="$(curl -6 -sS --max-time 5 ifconfig.co 2>/dev/null || true)"
  echo "--- curl (aparente; NÃO é prova inbound) ---"
  echo "IPv4: ${IP4:-none}"
  echo "IPv6: ${IP6:-none}"
  echo
  LAN4="$(ip -4 route get 1.1.1.1 2>/dev/null | awk '{for(i=1;i<=NF;i++) if($i=="src"){print $(i+1); exit}}')"
  echo "--- CGNAT heurística ---"
  echo "src LAN para WAN: ${LAN4:-?}"
  echo "curl IPv4:        ${IP4:-?}"
  if [[ -n "${LAN4:-}" && -n "${IP4:-}" && "$LAN4" != "$IP4" ]]; then
    echo "parecer: NAT/CGNAT (LAN != público aparente)"
  else
    echo "parecer: possível IP público na iface (ainda precisa probe externo)"
  fi
  echo
  echo "--- Escuta mycelium ---"
  ss -ltnp 2>/dev/null | grep -E ':(4001|443|4002)\b' || echo "(nenhuma)"
  ss -lunp 2>/dev/null | grep -E ':(4001|443|4002)\b' || true
  echo
  echo "--- Hairpin (LAN → IP público:4001/443) ---"
  if [[ -n "${IP4:-}" ]]; then
    if nc -z -w 3 "$IP4" 4001 2>/dev/null; then echo "hairpin TCP 4001: ok"; else echo "hairpin TCP 4001: fail"; fi
    if nc -z -w 3 "$IP4" 443 2>/dev/null; then echo "hairpin TCP 443: ok"; else echo "hairpin TCP 443: fail"; fi
  else
    echo "sem IPv4 aparente"
  fi
  echo
  echo "--- Outbound ---"
  if nc -z -w 5 1.1.1.1 443 2>/dev/null; then echo "TCP 1.1.1.1:443: ok"; else echo "TCP 1.1.1.1:443: fail"; fi
  echo
  echo "--- Daemon mycelium ---"
  pgrep -af 'mycelium daemon' | grep -v 'cpe-sensor\|pgrep' || echo "(nenhum)"
  if pgrep -af 'mycelium daemon' 2>/dev/null | grep -q -- '--sporocarp'; then
    echo
    echo "AVISO: há daemon com --sporocarp neste host."
    echo "Sem proof.json externo, isto pode anunciar /esporocarp morto."
    echo "Preferir: mycelium shutdown e subir como folha (sem --sporocarp)."
  fi
  echo
  echo "--- Próximo (5G) ---"
  echo "Com tcpdump a correr no TushiBook, no telemóvel:"
  echo "  ./scripts/probe-sporocarp.sh ${IP4:-IPV4} 4001 telemovel-5g"
  echo "  ./scripts/probe-sporocarp.sh ${IP6:-IPV6} 4001 telemovel-5g"
  echo "Esperado neste CPE: tcp=fail → preencher docs/mapa_cpe_vivo.md"
  echo
  echo "Relatório: $REPORT"
} | tee "$REPORT"

if [[ "${1:-}" == "--pcap" ]]; then
  echo
  echo "Corre noutro terminal (sudo):"
  echo "  sudo tcpdump -ni any -w $OUT_DIR/cpe-$TS.pcap \\"
  echo "    'tcp port 4001 or udp port 4001 or udp port 4002 or tcp port 443 or icmp or icmp6'"
  echo "Depois probes de 5G; Ctrl-C no tcpdump; analisa:"
  echo "  tcpdump -nr $OUT_DIR/cpe-$TS.pcap 'tcp[tcpflags] & tcp-syn != 0'"
fi

echo
echo "Docs: docs/engenharia-reversa-bloqueio.md · docs/mapa_cpe_vivo.md"
