#!/usr/bin/env bash
# Demo: seed A com --listen fixo → seeds.txt → B e C só com --seed-file --no-mdns
# (sem --bootstrap manual, sem mDNS).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="$ROOT/target/release/mycelium"
SEED_PORT=14001
SEEDS=/tmp/myc-seeds.txt
rm -rf /tmp/myc-seed-a /tmp/myc-seed-b /tmp/myc-seed-c "$SEEDS"
mkdir -p /tmp/myc-seed-a /tmp/myc-seed-b /tmp/myc-seed-c

cleanup() {
  "$BIN" --home /tmp/myc-seed-a shutdown 2>/dev/null || true
  "$BIN" --home /tmp/myc-seed-b shutdown 2>/dev/null || true
  "$BIN" --home /tmp/myc-seed-c shutdown 2>/dev/null || true
  sleep 1
}
trap cleanup EXIT

if [[ ! -x "$BIN" ]]; then
  echo "building mycelium…"
  cargo build -p mycelium-cli --release
fi

echo "== sprout =="
"$BIN" --home /tmp/myc-seed-a sprout --contribute 2cpu,4gb,100gb
"$BIN" --home /tmp/myc-seed-b sprout --contribute 1cpu,2gb,50gb
"$BIN" --home /tmp/myc-seed-c sprout --contribute 1cpu,2gb,50gb

echo "== daemon A (seed público, sem mDNS) =="
RUST_LOG=info "$BIN" --home /tmp/myc-seed-a daemon \
  --contribute 2cpu,4gb,100gb \
  --listen "/ip4/127.0.0.1/tcp/${SEED_PORT}" \
  --no-mdns \
  --horizon-port 17474 \
  >/tmp/seed-a.log 2>&1 &

for i in $(seq 1 40); do
  if [[ -f /tmp/myc-seed-a/listen_addrs.json ]]; then break; fi
  sleep 0.25
done
test -f /tmp/myc-seed-a/listen_addrs.json

python3 - <<'PY'
import json
addrs = json.load(open("/tmp/myc-seed-a/listen_addrs.json"))
# Prefer TCP dialable with /p2p/
pick = next((a for a in addrs if "/tcp/" in a and "/p2p/" in a), addrs[0])
open("/tmp/myc-seeds.txt", "w").write("# seedbook-demo\n" + pick + "\n")
print("seeds.txt ←", pick)
PY

echo "== daemon B/C (só --seed-file --no-mdns) =="
RUST_LOG=info "$BIN" --home /tmp/myc-seed-b daemon \
  --contribute 1cpu,2gb,50gb \
  --seed-file "$SEEDS" \
  --no-mdns \
  --horizon-port 17475 \
  >/tmp/seed-b.log 2>&1 &
RUST_LOG=info "$BIN" --home /tmp/myc-seed-c daemon \
  --contribute 1cpu,2gb,50gb \
  --seed-file "$SEEDS" \
  --no-mdns \
  --horizon-port 17476 \
  >/tmp/seed-c.log 2>&1 &

echo "== aguardando anastomose =="
ok=0
for i in $(seq 1 40); do
  nb=$("$BIN" --home /tmp/myc-seed-b status 2>/dev/null | awk '/vizinhos/{print $3; exit}' || echo 0)
  nc=$("$BIN" --home /tmp/myc-seed-c status 2>/dev/null | awk '/vizinhos/{print $3; exit}' || echo 0)
  echo "  tick $i: B.neighbors=$nb C.neighbors=$nc"
  if [[ "${nb:-0}" -ge 1 && "${nc:-0}" -ge 1 ]]; then
    ok=1
    break
  fi
  sleep 0.5
done
test "$ok" = "1"

echo "== isotope put A → get B =="
"$BIN" --home /tmp/myc-seed-a isotope-put --key forest --value "mycelium-alive"
sleep 2
"$BIN" --home /tmp/myc-seed-b isotope-get --key forest | grep -q mycelium-alive

echo "== status =="
"$BIN" --home /tmp/myc-seed-a status
"$BIN" --home /tmp/myc-seed-b status
"$BIN" --home /tmp/myc-seed-c status

echo "DONE — B e C encontraram A só via seed book (sem mDNS / sem --bootstrap)"
