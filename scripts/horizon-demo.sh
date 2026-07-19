#!/usr/bin/env bash
# Demo: sow (build.sh real) → Inertia build → Chamber (layers) → curl Event Horizon
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

if [[ ! -x "$BIN" ]]; then
  cargo build -p mycelium-cli --release
fi

"$BIN" --home "$HOME_A" sprout --contribute 2cpu,4gb,100gb
RUST_LOG=info "$BIN" --home "$HOME_A" daemon --contribute 2cpu,4gb,100gb --horizon-port "$PORT" \
  >/tmp/horizon-daemon.log 2>&1 &
sleep 2

BUILD_SH=$'#!/bin/sh\nmkdir -p dist\ncat > dist/index.html <<EOF\n<!doctype html><html><body><h1>built by inertia</h1><p>hello from the chamber</p></body></html>\nEOF\n'

SOW=$("$BIN" --home "$HOME_A" sow \
  --message "hello from the chamber" \
  --path "build.sh" \
  --content "$BUILD_SH")
echo "$SOW"
PLOT=${SOW#*plot semeado: }

SIG=$("$BIN" --home "$HOME_A" signal --plot "$PLOT" --quorum 1 --ion webapp --name ci)
echo "$SIG"
sleep 3

echo "== status =="
"$BIN" --home "$HOME_A" status

echo "== layers content-addressed =="
ls "$HOME_A/layers" | head

echo "== curl Event Horizon root =="
curl -sS "http://127.0.0.1:${PORT}/" | python3 -m json.tool

echo "== curl Ion via Singularity proxy =="
curl -sS "http://127.0.0.1:${PORT}/webapp/" | python3 -m json.tool

echo "== curl Chamber HTML (artefato do build) =="
curl -sS "http://127.0.0.1:${PORT}/webapp/index.html" | head -5
curl -sS "http://127.0.0.1:${PORT}/webapp/index.html" | grep -q "built by inertia"

echo "DONE"
