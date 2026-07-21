#!/usr/bin/env bash
# Sobe (ou reutiliza) um Volunteer Sporocarp e imprime a linha para mainnet.txt.
# Requer prova de inbound: MYCELIUM_REACHABLE=1 (ver docs/volunteer-sporocarp.md).
# NÃO correras isto no TushiBook/Vivo residencial — só no peer com verify externo OK.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="${BIN:-$ROOT/target/release/mycelium}"
[[ -x "$BIN" ]] || BIN="$ROOT/target/debug/mycelium"
HOME_DIR="${MYCELIUM_HOME:-$HOME/.local/share/mycelium-seed}"
PORT="${SEED_PORT:-4001}"
HORIZON="${HORIZON_PORT:-7477}"
ANNOUNCE="${MYCELIUM_ANNOUNCE_IP:-$(curl -4 -sS --max-time 5 ifconfig.me || true)}"
ANNOUNCE6="${MYCELIUM_ANNOUNCE_IP6:-$(curl -6 -sS --max-time 5 ifconfig.co || true)}"

if [[ "${MYCELIUM_REACHABLE:-}" != "1" && "${MYCELIUM_REACHABLE:-}" != "true" ]]; then
  echo "ERRO: MYCELIUM_REACHABLE não está definido." >&2
  echo "Fluxo:" >&2
  echo "  1) 5G:  ./scripts/probe-sporocarp.sh <IP> ${PORT} telemovel-5g > proof.json" >&2
  echo "  2) peer: ./scripts/verify-sporocarp.sh ${PORT} proof.json" >&2
  echo "  3) peer: MYCELIUM_REACHABLE=1 $0" >&2
  echo "Docs: docs/engenharia-reversa-bloqueio.md" >&2
  exit 1
fi

if [[ "${MYCELIUM_I_KNOW_THIS_IS_NOT_VIVO:-}" != "1" ]]; then
  # Heurística: se o announce público bate com CGNAT típico BR + aviso.
  echo "AVISO: este script é para o PEER VOLUNTÁRIO alcançável, não para o CPE Vivo."
  echo "Se estás no TushiBook residencial, Ctrl-C e lê docs/volunteer-sporocarp.md"
  echo "Para forçar (só após verify-sporocarp OK de 5G): MYCELIUM_I_KNOW_THIS_IS_NOT_VIVO=1"
  if [[ -z "${MYCELIUM_I_KNOW_THIS_IS_NOT_VIVO:-}" ]]; then
    sleep 3
  fi
fi

# Porta já ocupada pelo mycelium principal?
if ss -ltn 2>/dev/null | grep -qE ":${PORT}\\b"; then
  echo "ERRO: TCP :${PORT} já está em uso nesta máquina." >&2
  echo "Provavelmente o daemon principal (~/.local/share/mycelium) já escuta aqui." >&2
  echo "Esporocarpo voluntário = OUTRO host, ou SEED_PORT=4003 MYCELIUM_HOME=… distintos." >&2
  exit 1
fi

mkdir -p "$HOME_DIR"
if [[ ! -x "$BIN" ]]; then
  cargo build -p mycelium-cli --release
  BIN="$ROOT/target/release/mycelium"
fi

if [[ -S "$HOME_DIR/mycelium.sock" ]] || [[ -f "$HOME_DIR/mycelium.tcp" ]]; then
  if "$BIN" --home "$HOME_DIR" status 2>/dev/null | grep -q "Organismo vivo"; then
    echo "seed já rodando em $HOME_DIR"
  else
    echo "socket/estado stale em $HOME_DIR — remove mycelium.sock/tcp se o daemon morreu"
  fi
else
  "$BIN" --home "$HOME_DIR" sprout --contribute 2cpu,4gb,100gb >/dev/null || true
  echo "subindo sporocarp listen=0.0.0.0:${PORT} announce=${ANNOUNCE:-?} ip6=${ANNOUNCE6:-?} …"
  nohup env RUST_LOG=info \
    MYCELIUM_ANNOUNCE_IP="${ANNOUNCE}" \
    MYCELIUM_ANNOUNCE_IP6="${ANNOUNCE6}" \
    MYCELIUM_REACHABLE=1 \
    "$BIN" --home "$HOME_DIR" daemon \
      --listen "/ip4/0.0.0.0/tcp/${PORT}" \
      --listen "/ip6/::/tcp/${PORT}" \
      --listen "/ip4/0.0.0.0/udp/${PORT}/quic-v1" \
      --listen "/ip6/::/udp/${PORT}/quic-v1" \
      ${ANNOUNCE:+--announce-ip "$ANNOUNCE"} \
      ${ANNOUNCE6:+--announce-ip6 "$ANNOUNCE6"} \
      --assume-reachable \
      --no-mdns \
      --sporocarp \
      --horizon-port "$HORIZON" \
      --contribute 2cpu,4gb,100gb \
      >"$HOME_DIR/daemon.log" 2>&1 &
  for i in $(seq 1 40); do
    if [[ -S "$HOME_DIR/mycelium.sock" ]] || [[ -f "$HOME_DIR/mycelium.tcp" ]]; then
      break
    fi
    sleep 0.25
  done
  if ! "$BIN" --home "$HOME_DIR" status 2>/dev/null | grep -q "Organismo vivo"; then
    echo "ERRO: daemon não ficou vivo. Últimas linhas de $HOME_DIR/daemon.log:" >&2
    tail -20 "$HOME_DIR/daemon.log" 2>/dev/null || true
    exit 1
  fi
fi

echo "== status =="
"$BIN" --home "$HOME_DIR" status || true
echo "== seed line (cole em seeds/mainnet.txt) =="
"$ROOT/scripts/export-seed.sh" "$HOME_DIR"
echo
echo "Re-teste de fora (5G): ./scripts/verify-sporocarp.sh ${ANNOUNCE:-HOST} ${PORT}"
