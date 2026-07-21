#!/usr/bin/env bash
# Folha / sensor residencial (TushiBook, CGNAT, 5G).
# NÃO usa --sporocarp, --assume-reachable, nem announce WAN.
# Docs: docs/volunteer-sporocarp.md · docs/rizomorphs.md · docs/nostr-qel.md
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="${BIN:-$ROOT/target/release/mycelium}"
[[ -x "$BIN" ]] || BIN="$ROOT/target/debug/mycelium"
[[ -x "$BIN" ]] || BIN="$(command -v mycelium || true)"
HOME_DIR="${MYCELIUM_HOME:-$HOME/.local/share/mycelium}"
HORIZON="${HORIZON_PORT:-7474}"
SEED_FILE="${MYCELIUM_SEED_FILE:-$ROOT/seeds/mainnet.txt}"

if [[ ! -x "$BIN" ]]; then
  echo "ERRO: binário mycelium não encontrado. cargo build -p mycelium-cli --release" >&2
  exit 1
fi

# Preferir o binário do repo (PATH pode ter cargo install antigo sem --qel/--nostr).
if [[ -x "$ROOT/target/release/mycelium" ]]; then
  BIN="$ROOT/target/release/mycelium"
fi

for bad in MYCELIUM_REACHABLE MYCELIUM_ANNOUNCE_IP; do
  if [[ -n "${!bad:-}" ]]; then
    echo "AVISO: $bad está definido — ignorado neste run-folha.sh (folha ≠ esporocarpo)." >&2
  fi
done
# Folha não deve exigir auth no socket; se o shell herdou o token do esporocarpo,
# o daemon passa a rejeitar CLI sem MYCELIUM_CONTROL_TOKEN.
if [[ -n "${MYCELIUM_CONTROL_TOKEN:-}" ]]; then
  echo "AVISO: MYCELIUM_CONTROL_TOKEN herdado — a limpar para esta folha." >&2
  unset MYCELIUM_CONTROL_TOKEN
fi

if ss -ltn 2>/dev/null | grep -qE ':4001\b|:443\b'; then
  echo "AVISO: :4001 ou :443 ainda escuta nesta máquina." >&2
  echo "  Provavelmente um daemon --sporocarp fantasma. Para:" >&2
  echo "    pkill -f 'mycelium daemon.*--sporocarp' || kill \$(pgrep -f '--sporocarp')" >&2
fi

mkdir -p "$HOME_DIR"
PID_FILE="$HOME_DIR/mycelium.pid"

alive_via_rpc() {
  "$BIN" --home "$HOME_DIR" status 2>/dev/null | grep -q "Organismo vivo"
}

# Processo ainda a correr (pid file ou pgrep no home)?
daemon_pid_alive() {
  if [[ -f "$PID_FILE" ]]; then
    local p
    p=$(tr -d ' \n' <"$PID_FILE" || true)
    if [[ -n "$p" ]] && kill -0 "$p" 2>/dev/null; then
      echo "$p"
      return 0
    fi
  fi
  # fallback: daemon com este --home
  pgrep -f "mycelium.*--home ${HOME_DIR}.*daemon" 2>/dev/null | head -1 || true
}

if alive_via_rpc; then
  echo "folha já viva em $HOME_DIR"
  "$BIN" --home "$HOME_DIR" status | rg -i 'membrana|vizinhos|peer|home|wan' \
    || "$BIN" --home "$HOME_DIR" status | head -20
  exit 0
fi

EXISTING=$(daemon_pid_alive || true)
if [[ -n "${EXISTING:-}" ]]; then
  echo "daemon pid=$EXISTING ainda vivo mas RPC falhou — a reutilizar / esperar sock…"
  for _ in $(seq 1 20); do
    if alive_via_rpc; then
      echo "folha recuperada"
      "$BIN" --home "$HOME_DIR" status | head -20
      exit 0
    fi
    sleep 0.25
  done
  echo "AVISO: a matar pid $EXISTING (sock morto, processo zombie de controlo)" >&2
  kill "$EXISTING" 2>/dev/null || true
  sleep 1
  kill -9 "$EXISTING" 2>/dev/null || true
fi

# Horizon ocupado por outro processo?
if ss -ltn 2>/dev/null | grep -qE ":${HORIZON}\\b"; then
  echo "ERRO: :${HORIZON} já em uso. Opções:" >&2
  echo "  HORIZON_PORT=7476 $0" >&2
  echo "  ou: $BIN --home $HOME_DIR shutdown" >&2
  echo "  ou: pkill -f 'mycelium.*daemon'" >&2
  exit 1
fi

rm -f "$HOME_DIR/mycelium.sock" "$HOME_DIR/mycelium.tcp" "$HOME_DIR/mycelium.pid"

"$BIN" --home "$HOME_DIR" sprout --contribute 1cpu,2gb,50gb >/dev/null 2>&1 || true

ARGS=(daemon --no-mdns --horizon-port "$HORIZON" --contribute 1cpu,2gb,50gb)
if [[ -f "$SEED_FILE" ]]; then
  ARGS+=(--seed-file "$SEED_FILE")
fi
if [[ -n "${MYCELIUM_BOOTSTRAP:-}" ]]; then
  ARGS+=(--bootstrap "$MYCELIUM_BOOTSTRAP")
fi

echo "subindo FOLHA home=$HOME_DIR bin=$BIN seed=$SEED_FILE (sem --sporocarp)…"
nohup env RUST_LOG="${RUST_LOG:-info}" \
  "$BIN" --home "$HOME_DIR" "${ARGS[@]}" \
  >"$HOME_DIR/daemon-folha.log" 2>&1 &
echo $! >"$PID_FILE"

for _ in $(seq 1 40); do
  if alive_via_rpc; then
    break
  fi
  sleep 0.25
done

if ! alive_via_rpc; then
  echo "ERRO: folha não ficou viva. Log:" >&2
  tail -30 "$HOME_DIR/daemon-folha.log" 2>/dev/null || true
  exit 1
fi

"$BIN" --home "$HOME_DIR" status
echo
echo "OK folha. Esperado: membrana ≠ esporocarp; vizinhos=0 até haver seed verde."
echo "Usa este binário (não o cargo antigo):"
echo "  $BIN sow --message floresta --qel 3,7 --nostr --ghost"
echo "ou: cargo install --path cli/mycelium-cli --force"
echo "Sensor CPE: ./scripts/cpe-sensor.sh"
