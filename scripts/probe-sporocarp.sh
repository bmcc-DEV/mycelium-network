#!/usr/bin/env bash
# Probe externo de inbound TCP (gerar prova para o voluntário).
# Correr de OUTRA rede (telemóvel 5G) — NÃO da LAN do peer.
#
# Uso: ./scripts/probe-sporocarp.sh <host|ip> [porta] [from]
# Ex:  ./scripts/probe-sporocarp.sh 203.0.113.10 4001 telemovel-5g > proof.json
set -euo pipefail

HOST="${1:?uso: $0 <IP_PUBLICO_REAL> [porta] [from]}"
PORT="${2:-4001}"
FROM="${3:-external}"
TS="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

case "$HOST" in
  IP_DO_AMIGO|SEU_IP|IP_PUBLICO|HOST|IP_OU_HOST|IP_CANDIDATO|IP_VOLUNTARIO|example.com|*EXEMPLO*|*exemplo*)
    echo "{\"probe\":\"sporocarp\",\"error\":\"placeholder\",\"target\":\"$HOST\"}" >&2
    echo "ERRO: '$HOST' é placeholder — use IP/DNS real." >&2
    exit 1
    ;;
esac

TCP="fail"
DETAIL=""
if command -v nc >/dev/null 2>&1; then
  if OUT=$(nc -vz -w 5 "$HOST" "$PORT" 2>&1); then
    TCP="ok"
  fi
  DETAIL=$(printf '%s' "$OUT" | tr '\n' ' ' | sed 's/"/\\"/g')
elif command -v timeout >/dev/null 2>&1; then
  if timeout 5 bash -c "echo >/dev/tcp/${HOST}/${PORT}" 2>/dev/null; then
    TCP="ok"
  fi
else
  echo "{\"probe\":\"sporocarp\",\"error\":\"nc missing\"}" >&2
  exit 1
fi

printf '{"probe":"sporocarp","target":"%s","port":%s,"tcp":"%s","from":"%s","ts":"%s"}\n' \
  "$HOST" "$PORT" "$TCP" "$FROM" "$TS"

if [[ "$TCP" != "ok" ]]; then
  echo "probe tcp=fail (${DETAIL:-timeout/refused})" >&2
  exit 1
fi
exit 0
