#!/usr/bin/env bash
# Exporta um multiaddr TCP+p2p dialável com sufixo /esporocarp para o seed book.
# Uso: ./scripts/export-seed.sh /tmp/myc-seed-a
set -euo pipefail
HOME_DIR="${1:?uso: $0 <MYCELIUM_HOME>}"
ADDRS="$HOME_DIR/listen_addrs.json"
test -f "$ADDRS"
python3 - "$ADDRS" <<'PY'
import json, sys, ipaddress, re

addrs = json.load(open(sys.argv[1]))

def ip_of(a: str):
    m = re.search(r"/ip4/([^/]+)/", a) or re.search(r"/ip6/([^/]+)/", a)
    return m.group(1) if m else None

def score(a: str) -> int:
    if "/tcp/" not in a or "/p2p/" not in a:
        return -100
    raw = ip_of(a)
    if not raw:
        return -50
    try:
        ip = ipaddress.ip_address(raw)
    except ValueError:
        return -40
    if ip.is_loopback:
        return -30
    if ip.is_link_local:
        return -20
    if ip.is_private:  # RFC1918 + ULA + docker bridges
        return -10
    if ip.version == 4:
        return 50
    return 40  # IPv6 global

ranked = sorted(addrs, key=score, reverse=True)
pick = ranked[0] if ranked and score(ranked[0]) > 0 else None
if not pick:
    pick = next((a for a in addrs if "/tcp/" in a and "/p2p/" in a), None)
    if not pick:
        sys.exit(f"nenhum listen addr em {sys.argv[1]}")
    print(
        "# AVISO: nenhum IP global nos listen addrs — NÃO uses em mainnet",
        file=sys.stderr,
    )

for suf in ("/esporocarp", "/floresta", "/raiz", "/folha"):
    if pick.endswith(suf):
        pick = pick[: -len(suf)]
        break
print(pick + "/esporocarp")
PY
