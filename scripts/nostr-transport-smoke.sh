#!/usr/bin/env bash
# Smoke: duas folhas locais via Nostr transport → vizinhos>=1 em cada lado.
#
# Uso:
#   ./scripts/nostr-transport-smoke.sh
#   RELAY=wss://nos.lol WAIT=90 ./scripts/nostr-transport-smoke.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="${BIN:-$ROOT/target/release/mycelium}"
RELAY="${RELAY:-wss://nos.lol}"
WAIT="${WAIT:-90}"
HOME_A="${HOME_A:-/tmp/myc-nostr-smoke-a}"
HOME_B="${HOME_B:-/tmp/myc-nostr-smoke-b}"
LOG_A="${LOG_A:-/tmp/myc-nostr-smoke-a.log}"
LOG_B="${LOG_B:-/tmp/myc-nostr-smoke-b.log}"

die() { echo "ERRO: $*" >&2; exit 1; }

[[ -x "$BIN" ]] || BIN="$(command -v mycelium || true)"
[[ -x "$BIN" ]] || die "mycelium não encontrado — cargo build -p mycelium-cli --release"

cleanup() {
  "$BIN" --home "$HOME_A" shutdown 2>/dev/null || true
  "$BIN" --home "$HOME_B" shutdown 2>/dev/null || true
  pkill -f "mycelium daemon.*horizon-port 17475" 2>/dev/null || true
  pkill -f "mycelium daemon.*horizon-port 17476" 2>/dev/null || true
}
trap cleanup EXIT

cleanup
rm -rf "$HOME_A" "$HOME_B"
mkdir -p "$HOME_A" "$HOME_B"

echo "== smoke Nostr transport ($RELAY) =="
nohup env MYCELIUM_HOME="$HOME_A" RUST_LOG=info \
  "$BIN" daemon --nostr-transport --no-mdns --horizon-port 17475 --nostr-relay "$RELAY" \
  >"$LOG_A" 2>&1 &
nohup env MYCELIUM_HOME="$HOME_B" RUST_LOG=info \
  "$BIN" daemon --nostr-transport --no-mdns --horizon-port 17476 --nostr-relay "$RELAY" \
  >"$LOG_B" 2>&1 &

ok_a=0
ok_b=0
deadline=$((SECONDS + WAIT))
while (( SECONDS < deadline )); do
  sa="$("$BIN" --home "$HOME_A" status 2>/dev/null || true)"
  sb="$("$BIN" --home "$HOME_B" status 2>/dev/null || true)"
  na="$(printf '%s\n' "$sa" | sed -n 's/.*vizinhos[[:space:]]*:[[:space:]]*\([0-9][0-9]*\).*/\1/p' | head -1)"
  nb="$(printf '%s\n' "$sb" | sed -n 's/.*vizinhos[[:space:]]*:[[:space:]]*\([0-9][0-9]*\).*/\1/p' | head -1)"
  echo "t+$((WAIT - (deadline - SECONDS)))s  A=${na:-?}  B=${nb:-?}"
  [[ "${na:-0}" -ge 1 ]] && ok_a=1
  [[ "${nb:-0}" -ge 1 ]] && ok_b=1
  if [[ "$ok_a" -eq 1 && "$ok_b" -eq 1 ]]; then
    echo "OK: vizinhos>=1 em A e B"
    exit 0
  fi
  sleep 5
done

echo "FALHOU após ${WAIT}s"
echo "--- log A (tail) ---"; tail -40 "$LOG_A" || true
echo "--- log B (tail) ---"; tail -40 "$LOG_B" || true
exit 1
