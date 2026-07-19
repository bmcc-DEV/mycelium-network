#!/usr/bin/env bash
# Exporta o primeiro dialable TCP+p2p de um nó seed para o seed book.
# Uso: ./scripts/export-seed.sh /tmp/myc-seed-a
set -euo pipefail
HOME_DIR="${1:?uso: $0 <MYCELIUM_HOME>}"
ADDRS="$HOME_DIR/listen_addrs.json"
test -f "$ADDRS"
python3 - <<PY
import json, sys
addrs = json.load(open("$ADDRS"))
pick = next((a for a in addrs if "/tcp/" in a and "/p2p/" in a), None)
if not pick:
    pick = addrs[0] if addrs else None
if not pick:
    sys.exit("nenhum listen addr em $ADDRS")
print(pick)
PY
