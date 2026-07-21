#!/usr/bin/env bash
# Hybrid Theory demo: pista B (Nostr + ipfs-blocks) + lembrete pista A (voluntário).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="${BIN:-$ROOT/target/release/mycelium}"
[[ -x "$BIN" ]] || BIN="$ROOT/target/debug/mycelium"
[[ -x "$BIN" ]] || BIN="$(command -v mycelium || true)"

if [[ ! -x "$BIN" ]]; then
  echo "ERRO: mycelium não encontrado. cargo build -p mycelium-cli --release" >&2
  exit 1
fi

HOME_A="${MYCELIUM_HOME:-$HOME/.local/share/mycelium}"
HOME_B="${HYBRID_HOME_B:-/tmp/mycelium-hybrid-b}"
MSG="${HYBRID_MSG:-hybrid-theory-$(date +%H%M%S)}"

echo "== Hybrid Theory demo =="
echo "bin=$BIN"
echo "home_a=$HOME_A  home_b=$HOME_B"

# Folha viva?
if ! "$BIN" --home "$HOME_A" status 2>/dev/null | grep -q "Organismo vivo"; then
  echo "folha down — a tentar ./scripts/run-folha.sh"
  "$ROOT/scripts/run-folha.sh" || true
fi

if ! "$BIN" --home "$HOME_A" status 2>/dev/null | grep -q "Organismo vivo"; then
  echo "ERRO: precisa de folha viva em $HOME_A" >&2
  exit 1
fi

rm -rf "$HOME_B"
mkdir -p "$HOME_B"

echo
echo "== sow --hybrid =="
OUT=$("$BIN" --home "$HOME_A" sow --message "$MSG" --hybrid 2>&1 | tee /dev/stderr)
CID=$(echo "$OUT" | grep -oE 'Qm[0-9a-f]{64}' | head -1 || true)
if [[ -z "$CID" ]]; then
  echo "ERRO: sem ContentId na saída do sow" >&2
  exit 1
fi
echo "CID=$CID"

HEX="${CID#Qm}"
BLOCK="$HOME_A/ipfs-blocks/$HEX"
if [[ ! -f "$BLOCK" ]]; then
  echo "ERRO: blockstore sem plot em $BLOCK" >&2
  exit 1
fi
echo "ipfs-blocks OK ($(wc -c <"$BLOCK") bytes)"

echo
echo "== recall --hybrid (outro home, via Nostr) =="
"$BIN" --home "$HOME_B" recall --plot "$CID" --hybrid

echo
echo "== verify ipfs-only home (sem SporeBank do plot) =="
HOME_C="${HYBRID_HOME_C:-/tmp/mycelium-hybrid-c}"
rm -rf "$HOME_C"
mkdir -p "$HOME_C/ipfs-blocks"
cp "$BLOCK" "$HOME_C/ipfs-blocks/$HEX"
# sprout mínimo para organism.json / sporebank vazio
"$BIN" --home "$HOME_C" sprout --contribute 1cpu,1gb,1gb >/dev/null 2>&1 || true
# Forçar falha Nostr rápida: sem rede não é fiável; ipfs path após nostr.
# Aqui só validamos get local via recall hybrid (Nostr pode ganhar — se plot
# ainda estiver nos relays). Preferimos provar o ficheiro + absorb local:
if "$BIN" --home "$HOME_C" recall --plot "$CID" --hybrid 2>&1 | tee /tmp/hybrid-c.log | grep -qE 'reconstruído|local|ipfs-blocks'; then
  echo "home_c recall OK"
else
  echo "AVISO: recall home_c falhou (Nostr/ipfs). Log:" >&2
  cat /tmp/hybrid-c.log >&2 || true
fi

echo
echo "== Pista A (social) — próximos passos =="
echo "1. Preencher contactos em docs/candidatos.md"
echo "2. Enviar docs/pitch_voluntario.txt"
echo "3. Quando verde: probe → verify → MYCELIUM_REACHABLE → vizinhos >= 1"
echo
echo "OK Hybrid Theory B demonstrado. CID=$CID msg=$MSG"
