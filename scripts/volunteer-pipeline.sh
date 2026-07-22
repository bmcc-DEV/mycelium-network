#!/usr/bin/env bash
# Pipeline automatizado da pista A (esporocarpo voluntário).
#
# Subcomandos:
#   cgnat-check              — WAN vs IP público (CGNAT?)
#   pitch [nome]             — mensagem pronta (+ clipboard se xclip/wl-copy)
#   probe <ip> [porta]       — gera proof.json (correr no 5G)
#   prep-listen              — no VOLUNTÁRIO: sobe listen :4001 para o probe
#   onboard [proof.json]     — no VOLUNTÁRIO: verify → seed → export → mainnet
#   folha-attach <multiaddr> — na FOLHA: seed book + restart + espera vizinhos
#   mark <nome> <estado>     — actualiza docs/candidatos.state.json
#   status                   — mostra estado dos candidatos + folha
#
# Docs: docs/candidatos.md · docs/volunteer-sporocarp.md
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
STATE="$ROOT/docs/candidatos.state.json"
PITCH="$ROOT/docs/pitch_voluntario.txt"
MAINNET="$ROOT/seeds/mainnet.txt"
MATRIZ="$ROOT/docs/matriz_transporte_nat.md"
PROOF_DEFAULT="${PROOF:-$ROOT/proof.json}"
PORT="${SEED_PORT:-4001}"

die() { echo "ERRO: $*" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || die "falta comando: $1"; }

ensure_state() {
  if [[ ! -f "$STATE" ]]; then
    cat >"$STATE" <<'EOF'
{
  "updated": null,
  "candidatos": []
}
EOF
  fi
}

usage() {
  cat <<EOF
Pipeline automatizado da pista A (esporocarpo voluntário).

  cgnat-check              WAN vs IP público (CGNAT?)
  pitch [nome]             mensagem + clipboard + mark estado
  prep-listen              no voluntário: listen :4001 para o probe
  probe <ip> [porta]       no 5G: gera proof.json
  onboard [proof.json]     no voluntário: verify → seed → mainnet
  folha-attach <multiaddr> na folha: seed + restart + vizinhos>=1
  mark <nome> <estado>     actualiza docs/candidatos.state.json
  status                   candidatos + folha + seeds

Exemplos:
  $0 cgnat-check
  $0 pitch "Amigo fibra"
  $0 prep-listen                          # voluntário
  $0 probe 203.0.113.10                   # 5G
  $0 onboard proof.json                   # voluntário
  $0 folha-attach '/ip4/…/tcp/4001/p2p/…/esporocarp'
  $0 status
EOF
}

cmd_cgnat_check() {
  need curl
  echo "== CGNAT check =="
  PUB4="$(curl -4 -sS --max-time 8 ifconfig.me 2>/dev/null || true)"
  PUB6="$(curl -6 -sS --max-time 8 ifconfig.co 2>/dev/null || true)"
  echo "IP público aparente IPv4: ${PUB4:-none}"
  echo "IP público aparente IPv6: ${PUB6:-none}"

  # Heurística: interfaces LAN vs public
  LAN4="$(ip -4 route get 1.1.1.1 2>/dev/null | awk '{for(i=1;i<=NF;i++) if($i=="src"){print $(i+1); exit}}' || true)"
  echo "src local p/ 1.1.1.1: ${LAN4:-?}"

  if [[ -n "$PUB4" && -n "$LAN4" && "$PUB4" != "$LAN4" ]]; then
    echo
    echo "RESULTADO: provável CGNAT / NAT (WAN≠LAN)."
    echo "→ NÃO é candidato a esporocarpo. Usa como FOLHA + Nostr hybrid."
    echo "→ Marca: $0 mark \"NOME\" \"cgnat — folha\""
    return 2
  fi
  if [[ -n "$PUB4" && -n "$LAN4" && "$PUB4" == "$LAN4" ]]; then
    echo
    echo "RESULTADO: IP público parece coincidir com a máquina."
    echo "→ Candidato possível. Abre TCP/UDP $PORT no router → este PC."
    echo "→ Depois (de 5G): $0 probe $PUB4"
    return 0
  fi
  echo
  echo "RESULTADO: inconclusivo — confirma WAN do router vs ifconfig.me manualmente."
  return 0
}

cmd_pitch() {
  local nome="${1:-}"
  [[ -f "$PITCH" ]] || die "falta $PITCH"
  echo "======== pitch ========"
  if [[ -n "$nome" ]]; then
    echo "Olá $nome —"
    echo
  fi
  cat "$PITCH"
  echo
  echo "======================="
  echo
  echo "Passos para o voluntário (cola também):"
  cat <<EOF
1) Port-forward TCP+UDP $PORT → este PC
2) git clone / cargo build -p mycelium-cli --release
3) Espera eu gerar proof.json no 5G
4) ./scripts/volunteer-pipeline.sh onboard proof.json
EOF
  local clip=""
  if command -v wl-copy >/dev/null 2>&1; then clip=wl-copy
  elif command -v xclip >/dev/null 2>&1; then clip="xclip -selection clipboard"
  elif command -v pbcopy >/dev/null 2>&1; then clip=pbcopy
  fi
  if [[ -n "$clip" ]]; then
    {
      [[ -n "$nome" ]] && echo "Olá $nome —" && echo
      cat "$PITCH"
    } | eval "$clip"
    echo "(copiado para o clipboard)"
  fi
  if [[ -n "$nome" ]]; then
    cmd_mark "$nome" "contacto enviado $(date +%F)"
  fi
}

cmd_probe() {
  local host="${1:?uso: $0 probe <ip|dns> [porta]}"
  local port="${2:-$PORT}"
  local out="${3:-$PROOF_DEFAULT}"
  echo "== probe externo → $out =="
  "$ROOT/scripts/probe-sporocarp.sh" "$host" "$port" "telemovel-5g" | tee "$out"
  echo
  echo "OK. Envia $out ao voluntário e pede:"
  echo "  ./scripts/volunteer-pipeline.sh onboard $out"
}

cmd_prep_listen() {
  # Sobe listen TCP/QUIC 4001 SEM --sporocarp (só para o probe 5G).
  local home_seed="${MYCELIUM_HOME:-$HOME/.local/share/mycelium-seed}"
  local bin="${BIN:-$ROOT/target/release/mycelium}"
  [[ -x "$bin" ]] || bin="$ROOT/target/debug/mycelium"
  [[ -x "$bin" ]] || bin="$(command -v mycelium || true)"
  [[ -x "$bin" ]] || die "mycelium não encontrado — cargo build -p mycelium-cli --release"

  if ss -ltn 2>/dev/null | grep -qE ":${PORT}\\b"; then
    echo "Já escuta :$PORT — pronto para probe 5G."
    return 0
  fi

  mkdir -p "$home_seed"
  "$bin" --home "$home_seed" sprout --contribute 2cpu,4gb,100gb >/dev/null 2>&1 || true
  echo "A subir listen 0.0.0.0:$PORT (sem /esporocarp) em $home_seed …"
  nohup env RUST_LOG="${RUST_LOG:-info}" \
    "$bin" --home "$home_seed" daemon \
      --listen "/ip4/0.0.0.0/tcp/${PORT}" \
      --listen "/ip6/::/tcp/${PORT}" \
      --listen "/ip4/0.0.0.0/udp/${PORT}/quic-v1" \
      --listen "/ip6/::/udp/${PORT}/quic-v1" \
      --no-mdns \
      --horizon-port "${HORIZON_PORT:-7477}" \
      --contribute 2cpu,4gb,100gb \
      >"$home_seed/daemon-prep.log" 2>&1 &
  echo $! >"$home_seed/mycelium.pid"

  for _ in $(seq 1 40); do
    if ss -ltn 2>/dev/null | grep -qE ":${PORT}\\b"; then
      echo "OK: :$PORT em escuta. Diz ao Bruno o IP público e espera o proof.json."
      echo "  IP: $(curl -4 -sS --max-time 5 ifconfig.me || echo '?')"
      return 0
    fi
    sleep 0.25
  done
  die "listen não subiu — vê $home_seed/daemon-prep.log"
}

cmd_onboard() {
  # Corre no PC do VOLUNTÁRIO após receber proof.json
  local proof="${1:-$PROOF_DEFAULT}"
  [[ -f "$proof" ]] || die "falta prova: $proof (gera com: $0 probe <IP> no 5G)"

  if ! ss -ltn 2>/dev/null | grep -qE ":${PORT}\\b"; then
    echo "Porta $PORT down — a correr prep-listen…"
    cmd_prep_listen
  fi

  echo "== 1/4 verify-sporocarp =="
  if ! "$ROOT/scripts/verify-sporocarp.sh" "$PORT" "$proof"; then
    die "verify falhou — corrige port-forward / proof e repete"
  fi

  echo
  echo "== 2/4 reiniciar como sporocarp (REACHABLE) =="
  local home_seed="${MYCELIUM_HOME:-$HOME/.local/share/mycelium-seed}"
  local bin="${BIN:-$ROOT/target/release/mycelium}"
  [[ -x "$bin" ]] || bin="$(command -v mycelium || true)"
  "$bin" --home "$home_seed" shutdown 2>/dev/null || true
  sleep 1
  # liberta :4001 se ficou zombie
  if [[ -f "$home_seed/mycelium.pid" ]]; then
    kill "$(tr -d ' \n' <"$home_seed/mycelium.pid")" 2>/dev/null || true
  fi
  sleep 1

  MYCELIUM_REACHABLE=1 MYCELIUM_I_KNOW_THIS_IS_NOT_VIVO=1 \
    "$ROOT/scripts/run-public-seed.sh"

  echo
  echo "== 3/4 export seed line =="
  local line
  line="$("$ROOT/scripts/export-seed.sh" "$home_seed" | tail -1)"
  [[ -n "$line" ]] || die "export-seed vazio"
  echo "SEED: $line"

  if ! grep -qxF "$line" "$MAINNET" 2>/dev/null; then
    {
      echo
      echo "# auto onboard $(date -u +%Y-%m-%dT%H:%MZ)"
      echo "$line"
    } >>"$MAINNET"
    echo "Acrescentado a seeds/mainnet.txt"
  else
    echo "Já estava em seeds/mainnet.txt"
  fi

  echo
  echo "== 4/4 matriz =="
  if [[ -f "$MATRIZ" ]]; then
    if ! grep -qF "$line" "$MATRIZ" 2>/dev/null; then
      echo "| $(date +%F) | 5G/probe | voluntário TCP $PORT | circuit | ✅ onboard | \`$line\` |" >>"$MATRIZ"
      echo "Linha acrescentada a matriz_transporte_nat.md"
    fi
  fi

  echo
  echo "VERDE. Na FOLHA (Bruno):"
  echo "  ./scripts/volunteer-pipeline.sh folha-attach '$line'"
}

cmd_folha_attach() {
  local addr="${1:?uso: $0 folha-attach '<multiaddr/…/esporocarp>'}"
  [[ "$addr" == *"/esporocarp"* ]] || die "multiaddr deve terminar em /esporocarp (proof gate)"

  echo "== folha-attach =="
  if ! grep -qxF "$addr" "$MAINNET" 2>/dev/null; then
    echo "$addr" >>"$MAINNET"
    echo "Acrescentado a seeds/mainnet.txt"
  fi

  # seed book local da folha
  local home_folha="${MYCELIUM_HOME:-$HOME/.local/share/mycelium}"
  mkdir -p "$home_folha"
  local seeds_txt="$home_folha/seeds.txt"
  touch "$seeds_txt"
  if ! grep -qxF "$addr" "$seeds_txt" 2>/dev/null; then
    echo "$addr" >>"$seeds_txt"
    echo "Acrescentado a $seeds_txt"
  fi

  local bin="${BIN:-$ROOT/target/release/mycelium}"
  [[ -x "$bin" ]] || bin="$(command -v mycelium || true)"
  [[ -x "$bin" ]] || die "mycelium não encontrado"

  if "$bin" --home "$home_folha" status 2>/dev/null | grep -q "Organismo vivo"; then
    echo "A injectar bootstrap via RPC…"
    "$bin" --home "$home_folha" bootstrap --addr "$addr" 2>/dev/null \
      || echo "AVISO: bootstrap RPC falhou — a reiniciar folha"
  fi

  # Reinicia folha com seed file do repo (contém a linha)
  "$bin" --home "$home_folha" shutdown 2>/dev/null || true
  sleep 1
  MYCELIUM_SEED_FILE="$MAINNET" "$ROOT/scripts/run-folha.sh" || true

  echo
  echo "A esperar vizinhos>=1 (até ~60s)…"
  local ok=0
  for _ in $(seq 1 30); do
    local st
    st="$("$bin" --home "$home_folha" status 2>/dev/null || true)"
    local n
    n="$(printf '%s\n' "$st" | sed -n 's/.*vizinhos[[:space:]]*:[[:space:]]*\([0-9][0-9]*\).*/\1/p' | head -1)"
    if [[ "${n:-0}" -ge 1 ]]; then
      echo "OK: vizinhos=$n"
      printf '%s\n' "$st" | rg -i 'vizinhos|membrana|physarum|peer' || true
      ok=1
      break
    fi
    sleep 2
  done
  if [[ "$ok" -ne 1 ]]; then
    echo "Ainda vizinhos=0 — verifica firewall do voluntário / multiaddr / PeerId."
    "$bin" --home "$home_folha" status 2>/dev/null | head -20 || true
    exit 1
  fi
}

cmd_mark() {
  ensure_state
  local nome="${1:?uso: $0 mark <nome> <estado>}"
  local estado="${2:?uso: $0 mark <nome> <estado>}"
  python3 - "$STATE" "$nome" "$estado" <<'PY'
import json, sys, datetime
path, nome, estado = sys.argv[1], sys.argv[2], sys.argv[3]
with open(path) as f:
    data = json.load(f)
found = False
for c in data.get("candidatos", []):
    if c.get("nome", "").lower() == nome.lower():
        c["estado"] = estado
        c["updated"] = datetime.date.today().isoformat()
        found = True
        break
if not found:
    data.setdefault("candidatos", []).append({
        "nome": nome,
        "estado": estado,
        "updated": datetime.date.today().isoformat(),
    })
data["updated"] = datetime.datetime.utcnow().strftime("%Y-%m-%dT%H:%MZ")
with open(path, "w") as f:
    json.dump(data, f, indent=2, ensure_ascii=False)
    f.write("\n")
print(f"marcado: {nome} → {estado}")
PY
}

cmd_status() {
  ensure_state
  echo "== candidatos.state.json =="
  if command -v jq >/dev/null 2>&1; then
    jq -r '.candidatos[]? | "- \(.nome): \(.estado) (\(.updated // "?"))"' "$STATE" 2>/dev/null \
      || cat "$STATE"
  else
    cat "$STATE"
  fi
  echo
  echo "== folha =="
  local bin="${BIN:-$ROOT/target/release/mycelium}"
  [[ -x "$bin" ]] || bin="$(command -v mycelium || true)"
  if [[ -x "$bin" ]]; then
    "$bin" status 2>/dev/null | rg -i 'Organismo|vizinhos|membrana|physarum|wan' || echo "(folha down — ./scripts/run-folha.sh)"
  else
    echo "(sem binário mycelium)"
  fi
  echo
  echo "== seeds/mainnet.txt (esporocarp) =="
  grep -E 'esporocarp|^#' "$MAINNET" | grep -v '^#' | head -10 || echo "(nenhuma linha activa)"
}

main() {
  local cmd="${1:-}"
  shift || true
  case "$cmd" in
    cgnat-check|cgnat) cmd_cgnat_check "$@" ;;
    pitch) cmd_pitch "$@" ;;
    probe) cmd_probe "$@" ;;
    prep-listen|prep) cmd_prep_listen "$@" ;;
    onboard) cmd_onboard "$@" ;;
    folha-attach|attach) cmd_folha_attach "$@" ;;
    mark) cmd_mark "$@" ;;
    status) cmd_status "$@" ;;
    -h|--help|help|"") usage ;;
    *) die "comando desconhecido: $cmd (help)" ;;
  esac
}

main "$@"
