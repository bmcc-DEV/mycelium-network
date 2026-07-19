#!/usr/bin/env bash
# Demo Isotope Decay: put no dono do shard → get no peer via hifas.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="$ROOT/target/release/mycelium"
A=/tmp/myc-iso-a
B=/tmp/myc-iso-b
SEEDS=/tmp/myc-iso-seeds.txt
rm -rf "$A" "$B" "$SEEDS"
mkdir -p "$A" "$B"

cleanup() {
  "$BIN" --home "$A" shutdown 2>/dev/null || true
  "$BIN" --home "$B" shutdown 2>/dev/null || true
  sleep 1
}
trap cleanup EXIT

[[ -x "$BIN" ]] || cargo build -p mycelium-cli --release

echo "== sprout =="
"$BIN" --home "$A" sprout --contribute 2cpu,4gb,100gb
"$BIN" --home "$B" sprout --contribute 2cpu,4gb,100gb

echo "== daemon A (seed) =="
RUST_LOG=info "$BIN" --home "$A" daemon \
  --listen /ip4/127.0.0.1/tcp/14021 --no-mdns --horizon-port 17621 \
  --contribute 2cpu,4gb,100gb >/tmp/iso-a.log 2>&1 &
for i in $(seq 1 40); do [[ -f "$A/listen_addrs.json" ]] && break; sleep 0.25; done
"$ROOT/scripts/export-seed.sh" "$A" > "$SEEDS"

echo "== daemon B =="
RUST_LOG=info "$BIN" --home "$B" daemon \
  --seed-file "$SEEDS" --no-mdns --horizon-port 17622 \
  --contribute 2cpu,4gb,100gb >/tmp/iso-b.log 2>&1 &

ok=0
for i in $(seq 1 40); do
  nb=$("$BIN" --home "$B" status 2>/dev/null | awk '/vizinhos/{print $3; exit}' || echo 0)
  [[ "${nb:-0}" -ge 1 ]] && { ok=1; break; }
  sleep 0.4
done
test "$ok" = "1"

echo "== shards =="
"$BIN" --home "$A" status | grep isotope
"$BIN" --home "$B" status | grep isotope

echo "== put em A até achar chave owned=true =="
KEY=""
for i in $(seq 0 128); do
  out=$("$BIN" --home "$A" isotope-put --key "decay-$i" --value "hello-decay-$i")
  if echo "$out" | grep -q 'owned=true'; then
    KEY="decay-$i"
    echo "$out"
    break
  fi
done
test -n "$KEY"

echo "== get em B (Decay) chave=$KEY =="
out=$("$BIN" --home "$B" isotope-get --key "$KEY")
echo "$out"
echo "$out" | grep -q "hello-decay"
grep -q "decay query" /tmp/iso-b.log || grep -q "Decay" /tmp/iso-b.log || true
grep -Eq "decay (query|reply)" /tmp/iso-a.log /tmp/iso-b.log

echo "== deploy one-shot em A =="
"$BIN" --home "$A" deploy \
  --message "isotope deploy" \
  --path build.sh \
  --content $'#!/bin/sh\nmkdir -p dist\necho \'<h1>deploy</h1>\' > dist/index.html\n' \
  --ion webapp --timeout 25 | tee /tmp/iso-deploy.out
grep -q "Event Horizon" /tmp/iso-deploy.out
curl -sS "http://127.0.0.1:17621/webapp/" | grep -qi deploy

echo "DONE — Isotope Decay + deploy"
