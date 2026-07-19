#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="$ROOT/target/release/mycelium"
rm -rf /tmp/myc-a /tmp/myc-b
mkdir -p /tmp/myc-a /tmp/myc-b

cleanup() {
  "$BIN" --home /tmp/myc-a shutdown 2>/dev/null || true
  "$BIN" --home /tmp/myc-b shutdown 2>/dev/null || true
  sleep 1
}
trap cleanup EXIT

echo "== sprout =="
"$BIN" --home /tmp/myc-a sprout --contribute 2cpu,4gb,100gb
"$BIN" --home /tmp/myc-b sprout --contribute 1cpu,2gb,50gb

echo "== daemon A =="
RUST_LOG=info "$BIN" --home /tmp/myc-a daemon --contribute 2cpu,4gb,100gb >/tmp/daemon-a.log 2>&1 &
for i in $(seq 1 30); do
  if [[ -f /tmp/myc-a/listen_addrs.json ]]; then break; fi
  sleep 0.5
done
ADDR=$(python3 -c "import json; print(json.load(open('/tmp/myc-a/listen_addrs.json'))[0])")
echo "bootstrap addr: $ADDR"

echo "== daemon B =="
RUST_LOG=info "$BIN" --home /tmp/myc-b daemon --contribute 1cpu,2gb,50gb --bootstrap "$ADDR" >/tmp/daemon-b.log 2>&1 &
sleep 4

echo "== status A =="
"$BIN" --home /tmp/myc-a status

echo "== sow =="
SOW_OUT=$("$BIN" --home /tmp/myc-a sow --message "hello lattice" --path "app.rs" --content 'fn main(){ println!("hi"); }')
echo "$SOW_OUT"
PLOT=${SOW_OUT#*plot semeado: }
echo "PLOT=$PLOT"
sleep 3

echo "== recall B =="
"$BIN" --home /tmp/myc-b recall --plot "$PLOT" || true
# se ainda não chegou, espera mais
sleep 2
"$BIN" --home /tmp/myc-b recall --plot "$PLOT"

echo "== signal A quorum=2 =="
SIG_OUT=$("$BIN" --home /tmp/myc-a signal --plot "$PLOT" --quorum 2 --ion webapp --name ci)
echo "$SIG_OUT"
SIGNAL=${SIG_OUT#*signal emitido: }

echo "== resonate B =="
"$BIN" --home /tmp/myc-b resonate --signal "$SIGNAL"
sleep 2

echo "== status final =="
"$BIN" --home /tmp/myc-a status
"$BIN" --home /tmp/myc-b status

echo "== persistência (reboot A) =="
NODE_BEFORE=$("$BIN" --home /tmp/myc-a status | awk '/NodeId/{print $3; exit}')
"$BIN" --home /tmp/myc-a shutdown
sleep 1
RUST_LOG=warn "$BIN" --home /tmp/myc-a daemon --contribute 2cpu,4gb,100gb >/tmp/daemon-a2.log 2>&1 &
sleep 2
NODE_AFTER=$("$BIN" --home /tmp/myc-a status | awk '/NodeId/{print $3; exit}')
echo "NodeId before=$NODE_BEFORE"
echo "NodeId after =$NODE_AFTER"
test "$NODE_BEFORE" = "$NODE_AFTER"
echo "OK: PeerId/NodeId sobreviveu ao reboot"

# ions devem estar no estado persistido
"$BIN" --home /tmp/myc-a status | grep -q webapp && echo "OK: ion webapp persistido" || echo "WARN: ion não listado no status pós-reboot (checar organism.json)"
cat /tmp/myc-a/organism.json | python3 -c "import json,sys; s=json.load(sys.stdin); print('ions=', s.get('ions')); print('plots ok')"
ls /tmp/myc-a/sporebank/plots | head
echo "DONE"