#!/usr/bin/env bash
# Sobe (ou reutiliza) o seed público local e imprime a linha para mainnet.txt.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="${BIN:-$ROOT/target/release/mycelium}"
HOME_DIR="${MYCELIUM_HOME:-$HOME/.local/share/mycelium-seed}"
PORT="${SEED_PORT:-4001}"
HORIZON="${HORIZON_PORT:-7477}"
ANNOUNCE="${MYCELIUM_ANNOUNCE_IP:-$(curl -4 -sS --max-time 5 ifconfig.me || true)}"

mkdir -p "$HOME_DIR"
if [[ ! -x "$BIN" ]]; then
  cargo build -p mycelium-cli --release
fi

if [[ -S "$HOME_DIR/mycelium.sock" ]]; then
  echo "seed já rodando em $HOME_DIR"
else
  "$BIN" --home "$HOME_DIR" sprout --contribute 2cpu,4gb,100gb >/dev/null
  echo "subindo seed listen=0.0.0.0:${PORT} announce=${ANNOUNCE:-?} …"
  nohup env RUST_LOG=info \
    MYCELIUM_ANNOUNCE_IP="${ANNOUNCE}" \
    "$BIN" --home "$HOME_DIR" daemon \
      --listen "/ip4/0.0.0.0/tcp/${PORT}" \
      ${ANNOUNCE:+--announce-ip "$ANNOUNCE"} \
      --no-mdns \
      --horizon-port "$HORIZON" \
      --contribute 2cpu,4gb,100gb \
      >"$HOME_DIR/daemon.log" 2>&1 &
  for i in $(seq 1 40); do
    [[ -f "$HOME_DIR/listen_addrs.json" ]] && break
    sleep 0.25
  done
fi

echo "== status =="
"$BIN" --home "$HOME_DIR" status || true
echo "== seed line (cole em seeds/mainnet.txt) =="
"$ROOT/scripts/export-seed.sh" "$HOME_DIR"
# Preferir linha com IP anunciado (não 127.0.0.1)
python3 - <<PY
import json
addrs=json.load(open("$HOME_DIR/listen_addrs.json"))
pub=[a for a in addrs if "/tcp/" in a and "/p2p/" in a and "127.0.0.1" not in a]
pick = pub[0] if pub else next((a for a in addrs if "/tcp/" in a and "/p2p/" in a), addrs[0])
print("PUBLIC_CANDIDATE=", pick)
open("$HOME_DIR/public_seed.txt","w").write(pick+"\n")
PY
