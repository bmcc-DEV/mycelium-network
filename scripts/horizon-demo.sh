#!/usr/bin/env bash
# Demo: sow → signal → Chamber processo → curl no Event Horizon
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="$ROOT/target/release/mycelium"
HOME_A=/tmp/myc-horizon
PORT=7475
rm -rf "$HOME_A"
mkdir -p "$HOME_A"

cleanup() {
  "$BIN" --home "$HOME_A" shutdown 2>/dev/null || true
  sleep 1
}
trap cleanup EXIT

"$BIN" --home "$HOME_A" sprout --contribute 2cpu,4gb,100gb
RUST_LOG=info "$BIN" --home "$HOME_A" daemon --contribute 2cpu,4gb,100gb --horizon-port "$PORT" \
  >/tmp/horizon-daemon.log 2>&1 &
sleep 2

SOW=$("$BIN" --home "$HOME_A" sow --message "hello from the chamber" --path "app.rs" --content 'fn main(){}')
echo "$SOW"
PLOT=${SOW#*plot semeado: }

SIG=$("$BIN" --home "$HOME_A" signal --plot "$PLOT" --quorum 1 --ion webapp --name ci)
echo "$SIG"
sleep 2

echo "== status =="
"$BIN" --home "$HOME_A" status

echo "== curl Event Horizon root =="
curl -sS "http://127.0.0.1:${PORT}/" | python3 -m json.tool

echo "== curl Ion via Singularity proxy =="
curl -sS "http://127.0.0.1:${PORT}/webapp/" | python3 -m json.tool

echo "== curl Chamber HTML =="
curl -sS "http://127.0.0.1:${PORT}/webapp/index.html" | head -5

echo "DONE"