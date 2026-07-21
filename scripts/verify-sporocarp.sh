#!/usr/bin/env bash
# Gate do esporocarpo: escuta local + prova externa (proof.json).
#
# No candidato (peer voluntário):
#   ./scripts/verify-sporocarp.sh [porta] [proof.json]
#
# Atalho externo (equivale a probe):
#   ./scripts/verify-sporocarp.sh <host> [porta]
#
# Verde → pode exportar MYCELIUM_REACHABLE=1 e correr run-public-seed.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"

# Atalho: primeiro arg parece host → delega ao probe
if [[ $# -ge 1 && "$1" != /* && ! "$1" =~ ^[0-9]+$ ]]; then
  exec "$ROOT/scripts/probe-sporocarp.sh" "$@"
fi

PORT="${1:-4001}"
PROOF="${2:-reachability-proof.json}"

echo "🍄 verify-sporocarp"
echo "porta: $PORT"
echo "prova: $PROOF"
echo

# 1. porta local em escuta
listening=0
if command -v ss >/dev/null 2>&1; then
  if ss -ltn 2>/dev/null | grep -qE ":${PORT}\\b"; then
    listening=1
  fi
fi
if [[ "$listening" -ne 1 ]]; then
  echo "❌ porta $PORT não está em escuta local (ss)"
  echo "   Sobe o daemon com --listen /ip4/0.0.0.0/tcp/$PORT primeiro."
  exit 1
fi
echo "✅ porta $PORT em escuta local"

# 2. IPs aparentes — NÃO são prova
IP4="$(curl -4 -sS --max-time 5 ifconfig.me 2>/dev/null || true)"
IP6="$(curl -6 -sS --max-time 5 ifconfig.co 2>/dev/null || true)"
echo "IPv4 aparente: ${IP4:-none}  (não é prova de inbound)"
echo "IPv6 aparente: ${IP6:-none}  (não é prova de inbound)"
echo

# 3. prova externa
if [[ ! -f "$PROOF" ]]; then
  cat <<EOF
⚠️  Falta prova externa ($PROOF).

A partir de telemóvel 5G (outra rede):

  ./scripts/probe-sporocarp.sh ${IP4:-IP_PUBLICO} $PORT telemovel-5g > $PROOF

Se IPv6:

  ./scripts/probe-sporocarp.sh ${IP6:-IPV6_PUBLICO} $PORT telemovel-5g >> $PROOF

Depois, neste host:

  ./scripts/verify-sporocarp.sh $PORT $PROOF
EOF
  exit 1
fi

# 4. validar prova (qualquer linha com tcp=ok)
ok=0
if command -v jq >/dev/null 2>&1; then
  if jq -s 'any(.[]; .tcp == "ok")' "$PROOF" 2>/dev/null | grep -q true; then
    ok=1
  fi
elif grep -q '"tcp":"ok"' "$PROOF"; then
  ok=1
fi

if [[ "$ok" -ne 1 ]]; then
  echo "❌ prova externa inválida ou TCP falhou em $PROOF"
  echo "MYCELIUM_REACHABLE=0"
  exit 1
fi

echo "✅ prova externa TCP ok"
echo
echo "MYCELIUM_REACHABLE=1"
echo "Próximo: MYCELIUM_REACHABLE=1 ./scripts/run-public-seed.sh"
echo "Docs: docs/volunteer-sporocarp.md · docs/engenharia-reversa-bloqueio.md"
exit 0
