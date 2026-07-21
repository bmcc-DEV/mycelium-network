#!/usr/bin/env bash
# Mycelium — diagnóstico de membrana / alcançabilidade (zero VPS).
# Não promove esporocarp só porque há IPv6 na interface: inbound pode estar dropado.
set -euo pipefail

echo "Mycelium Network — Diagnóstico Descentralizado"
echo "=============================================="

suggest_membrane="folha"
wan_relayable="nao"

echo ""
echo "--- 1. Identidade de rede ---"
LOCAL_IP="$(hostname -I 2>/dev/null | awk '{print $1}' || true)"
IPV6_GLOBAL="$(ip -6 addr show scope global 2>/dev/null | grep -oP 'inet6 \K[23][0-9a-f:]+' | head -1 || true)"
IPV6_ULA="$(ip -6 addr show scope global 2>/dev/null | grep -oP 'inet6 \Kfd[0-9a-f:]+' | head -1 || true)"
echo "IPv4 local: ${LOCAL_IP:-N/A}"
echo "IPv6 global (2xxx/3xxx): ${IPV6_GLOBAL:-N/A}"
echo "IPv6 ULA (fd..): ${IPV6_ULA:-N/A}"

echo ""
echo "--- 2. Outbound ---"
echo -n "TCP outbound 1.1.1.1:443: "
if timeout 5 bash -c 'echo >/dev/tcp/1.1.1.1/443' 2>/dev/null; then
  echo "ok"
else
  echo "falhou"
fi
echo -n "UDP STUN (stun.l.google.com:19302): "
if timeout 3 bash -c 'echo test >/dev/udp/stun.l.google.com/19302' 2>/dev/null; then
  echo "envio ok (sem garantia de resposta)"
else
  echo "falhou / bloqueado"
fi

echo ""
echo "--- 3. NAT / IP publico ---"
PUBLIC_IP="$(curl -4 -sS --max-time 5 ifconfig.me 2>/dev/null || echo N/A)"
echo "IPv4 visto de fora: ${PUBLIC_IP}"
if [[ -n "${LOCAL_IP}" && "${LOCAL_IP}" == "${PUBLIC_IP}" ]]; then
  echo "Sem NAT IPv4 aparente (local == publico)."
  suggest_membrane="raiz"
elif [[ "${PUBLIC_IP}" != "N/A" && -n "${PUBLIC_IP}" ]]; then
  echo "Atras de NAT/CGNAT IPv4 (local != publico)."
else
  echo "Sem IPv4 publico detectavel."
fi

echo ""
echo "--- 4. IPv6 inbound (heuristica) ---"
if [[ -n "${IPV6_GLOBAL}" ]]; then
  echo "Tem IPv6 global: ${IPV6_GLOBAL}"
  echo "ATENCAO: muitos ISPs BR (ex. Vivo CPE) dropam TCP SYN inbound em TODAS as portas."
  echo "ICMP pode passar e ainda assim o no NAO e esporocarp WAN."
  echo "Teste externo: nc -vz ${IPV6_GLOBAL} 4001"
  echo "Sem probe positiva (MYCELIUM_REACHABLE=1 ou --assume-reachable), trate como floresta local / folha WAN."
  suggest_membrane="floresta"
else
  echo "Sem IPv6 global roteavel."
fi

echo ""
echo "--- 5. Portas Mycelium ---"
if command -v ss >/dev/null; then
  ss -ltnup 2>/dev/null | grep -E ':4001|:4002|:443|:7474' || echo "(nenhuma 4001/4002/443/7474 em escuta)"
else
  echo "ss indisponivel"
fi

echo ""
echo "--- 6. DNS ---"
if [[ -f /etc/resolv.conf ]]; then
  echo "resolv.conf: presente"
  grep -E '^nameserver' /etc/resolv.conf 2>/dev/null | head -3 || true
else
  echo "resolv.conf: AUSENTE (Android/container) — mycelium usa DNS Cloudflare embutido"
fi

echo ""
echo "--- 7. Bubblewrap ---"
if command -v bwrap >/dev/null; then
  echo "bwrap: $(bwrap --version 2>/dev/null | head -1 || echo presente)"
else
  echo "bwrap: ausente (Vacuum/chamber pode falhar)"
fi

echo ""
echo "--- 8. Membrana sugerida ---"
# WAN relayavel so com evidencia explicita
if [[ "${MYCELIUM_REACHABLE:-}" == "1" || "${MYCELIUM_REACHABLE:-}" == "true" ]]; then
  wan_relayable="sim"
  suggest_membrane="esporocarp"
  echo "MYCELIUM_REACHABLE=1 → esporocarp permitido (inbound verificado pelo operador)."
else
  echo "Sem MYCELIUM_REACHABLE=1 → NAO auto-promover a esporocarp."
  if [[ "${suggest_membrane}" == "floresta" ]]; then
    echo "IPv6 global sem probe → floresta local; folhas WAN devem usar outro /esporocarp no Spore Bank."
  fi
fi

echo ""
echo "=============================================="
echo "Sugestao: membrana=${suggest_membrane}  wan_relayable=${wan_relayable}"
echo "Bootstrap: mDNS | seeds.txt | MYCELIUM_DNS_SEEDS (DuckDNS TXT)"
echo "Limite: DHT/gossip/hole-punch NAO substituem um peer ja alcancavel."
echo "Docs: docs/rizomorphs.md"
