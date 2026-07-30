#!/usr/bin/env bash
# Stress test virtual — Mycelium Network
# Uso: ./scripts/stress-test.sh [duração_min] [num_nos]
# Default: 10 minutos, 3 nós

set -euo pipefail

M="$PWD/target/debug/mycelium"
DURATION_MIN="${1:-10}"
NUM_NODES="${2:-3}"
BASE_PORT=48000
BASE_HORIZON=7800
START=$(date +%s)
END=$((START + DURATION_MIN * 60))
PIDS=()
REPORT="/tmp/mycelium-stress-report-$(date +%s).json"

cleanup() {
  echo ""
  echo "[STRESS] Limpando $NUM_NODES nós..."
  for i in $(seq 1 $NUM_NODES); do
    "$M" --home "/tmp/stress-$i" shutdown 2>/dev/null || true
  done
  wait 2>/dev/null
  echo "[STRESS] OK"
}
trap cleanup EXIT INT TERM

echo "=============================================="
echo " Mycelium Stress Test"
echo " Duração: ${DURATION_MIN}min  Nós: ${NUM_NODES}"
echo "=============================================="

# Fase 1: Germinar todos os nós
echo ""
echo "[FASE 1] Germinando $NUM_NODES nós..."
for i in $(seq 1 $NUM_NODES); do
  rm -rf "/tmp/stress-$i"
  "$M" --home "/tmp/stress-$i" sprout --contribute "2cpu,4gb,50gb" > /dev/null 2>&1
done
echo "[FASE 1] OK"

# Fase 2: Subir daemons
echo ""
echo "[FASE 2] Subindo daemons..."
BOOTSTRAP=""
for i in $(seq 1 $NUM_NODES); do
  PORT=$((BASE_PORT + i))
  HORIZON=$((BASE_HORIZON + i))
  LOG="/tmp/stress-$i/daemon.log"
  NOSTR_FLAG="--nostr-transport --nostr-relay wss://nos.lol"

  if [ "$i" -eq 1 ]; then
    # Nó 1: seed (escuta TCP fixa)
    "$M" --home "/tmp/stress-1" daemon --no-mdns \
      --listen "/ip4/127.0.0.1/tcp/$PORT" \
      --horizon-port $HORIZON \
      $NOSTR_FLAG > "$LOG" 2>&1 &
    PIDS[$i]=$!
    BOOTSTRAP="/ip4/127.0.0.1/tcp/$PORT"
  else
    # Demais nós: bootstrap do nó 1
    "$M" --home "/tmp/stress-$i" daemon --no-mdns \
      --listen "/ip4/127.0.0.1/tcp/$PORT" \
      --bootstrap "$BOOTSTRAP" \
      --horizon-port $HORIZON \
      $NOSTR_FLAG > "$LOG" 2>&1 &
    PIDS[$i]=$!
  fi
  echo "  Nó $i: pid=${PIDS[$i]} porta=$PORT horizon=$HORIZON"
done
sleep 5
echo "[FASE 2] OK"

# Fase 3: Verificar conectividade
echo ""
echo "[FASE 3] Conectividade..."
for i in $(seq 1 $NUM_NODES); do
  V=$("$M" --home "/tmp/stress-$i" status 2>&1 | grep vizinhos | grep -oP '\d+')
  N=$("$M" --home "/tmp/stress-$i" status 2>&1 | grep NodeId | grep -oP '[a-f0-9]{16}')
  echo "  Nó $i ($N): $V vizinhos"
done

# Métricas a coletar
TOTAL_SOWS=0
TOTAL_RECALLS=0
TOTAL_ISOPUT=0
TOTAL_ISOGET=0
TOTAL_SIGNALS=0
TOTAL_DEPLOYS=0
PEAK_MEM=0
ROUND=0

# Fase 4: Loop de estresse
echo ""
echo "[FASE 4] Estresse por ${DURATION_MIN}min..."
echo ""

while [ $(date +%s) -lt $END ]; do
  ROUND=$((ROUND + 1))
  NOW=$(date +%s)
  ELAPSED=$((NOW - START))
  REMAINING=$((END - NOW))

  # A cada ronda (~30s), executa operações em nós alternados
  NODE=$(( (ROUND % NUM_NODES) + 1 ))
  NODE2=$(( ((ROUND + 1) % NUM_NODES) + 1 ))
  if [ "$NODE2" -eq 0 ]; then NODE2=$NUM_NODES; fi

  echo -n "[${ELAPSED}s / ${REMAINING}s restantes] Rodada $ROUND — Nó $NODE: "

  # 1) Sow
  SOW_OUT=$("$M" --home "/tmp/stress-$NODE" sow \
    --message "stress-r$ROUND" \
    --path "data.txt" \
    --content "payload-$(date +%s)" 2>&1) && TOTAL_SOWS=$((TOTAL_SOWS + 1))
  PLOT_ID=$(echo "$SOW_OUT" | grep -oP 'Qm[a-f0-9]{64}')
  echo -n "sow "

  # 2) Recall (no peer)
  if [ -n "$PLOT_ID" ]; then
    "$M" --home "/tmp/stress-$NODE2" recall --plot "$PLOT_ID" > /dev/null 2>&1 && TOTAL_RECALLS=$((TOTAL_RECALLS + 1))
    echo -n "recall "
  fi

  # 3) Isotope put
  "$M" --home "/tmp/stress-$NODE" isotope-put \
    --key "stress-key" \
    --value "$ROUND" \
    --clock "$(date +%s)" > /dev/null 2>&1 && TOTAL_ISOPUT=$((TOTAL_ISOPUT + 1))
  echo -n "iso "

  # 4) Isotope get (no peer — Decay) a cada 3 rondas
  if [ $((ROUND % 3)) -eq 0 ]; then
    "$M" --home "/tmp/stress-$NODE2" isotope-get --key "stress-key" > /dev/null 2>&1 && TOTAL_ISOGET=$((TOTAL_ISOGET + 1))
    echo -n "decay "
  fi

  # 5) Signal a cada 5 rondas
  if [ $((ROUND % 5)) -eq 0 ]; then
    SIG_OUT=$("$M" --home "/tmp/stress-$NODE" signal \
      --plot "$PLOT_ID" --quorum 1 --ion "stress-ion" --name "ci" 2>&1) && TOTAL_SIGNALS=$((TOTAL_SIGNALS + 1))
    echo -n "signal "
  fi

  # 6) Matar e subir um nó aleatório a cada 8 rondas
  if [ $((ROUND % 8)) -eq 0 ]; then
    KILL_NODE=$(( (ROUND / 8) % NUM_NODES + 1 ))
    KILL_PID=${PIDS[$KILL_NODE]}
    echo -n "| kill $KILL_NODE "
    kill -9 "$KILL_PID" 2>/dev/null || true
    sleep 2
    PORT=$((BASE_PORT + KILL_NODE))
    HORIZON=$((BASE_HORIZON + KILL_NODE))
    LOG="/tmp/stress-$KILL_NODE/daemon.log"
    if [ "$KILL_NODE" -eq 1 ]; then
      "$M" --home "/tmp/stress-$KILL_NODE" daemon --no-mdns \
        --listen "/ip4/127.0.0.1/tcp/$PORT" \
        --horizon-port $HORIZON \
        --nostr-transport > "$LOG" 2>&1 &
    else
      "$M" --home "/tmp/stress-$KILL_NODE" daemon --no-mdns \
        --listen "/ip4/127.0.0.1/tcp/$PORT" \
        --bootstrap "/ip4/127.0.0.1/tcp/$((BASE_PORT + 1))" \
        --horizon-port $HORIZON \
        --nostr-transport > "$LOG" 2>&1 &
    fi
    PIDS[$KILL_NODE]=$!
    echo -n "restart "
  fi

  echo ""

  # Coleta métricas
  for i in $(seq 1 $NUM_NODES); do
    "$M" --home "/tmp/stress-$i" status > "/tmp/metrics-node-$i.txt" 2>&1 || true
  done

  sleep 25
done

# Fase 5: Relatório
echo ""
echo "=============================================="
echo " RELATÓRIO DE ESTRESSE"
echo " Duração: ${DURATION_MIN}min  Nós: ${NUM_NODES}  Rodadas: $ROUND"
echo "=============================================="
echo ""

echo "Operações:"
echo "  Sows:       $TOTAL_SOWS"
echo "  Recalls:    $TOTAL_RECALLS"
echo "  IsotopePut: $TOTAL_ISOPUT"
echo "  IsotopeGet: $TOTAL_ISOGET"
echo "  Signals:    $TOTAL_SIGNALS"
echo ""

echo "Estado final dos nós:"
for i in $(seq 1 $NUM_NODES); do
  S=$(cat "/tmp/metrics-node-$i.txt" 2>/dev/null || echo "OFFLINE")
  N=$(echo "$S" | grep NodeId | grep -oP '[a-f0-9]{16}')
  V=$(echo "$S" | grep vizinhos | grep -oP '\d+')
  P=$(echo "$S" | grep plots | grep -oP '\d+')
  A=$(echo "$S" | grep ATP | grep -oP 'ATP=\K\d+')
  I=$(echo "$S" | grep ions | grep -oP '(?<=\[).*(?=\])')
  H=$(echo "$S" | grep horizon | grep -oP 'http[^ ]+')
  echo "  Nó $i ($N): vizinhos=$V plots=$P ATP=$A ions=$I horizon=$H"
done

echo ""
echo "Métricas agregadas:"
for i in $(seq 1 $NUM_NODES); do
  echo "--- Nó $i ---"
  curl -s "http://127.0.0.1:$((BASE_HORIZON + i))/metrics" 2>/dev/null | grep mycelium | grep -v "#" || echo "  (sem resposta)"
done

echo ""
echo "Tamanho dos homes:"
for i in $(seq 1 $NUM_NODES); do
  echo -n "  Nó $i: "
  du -sh "/tmp/stress-$i/" 2>/dev/null | awk '{print $1}'
done

echo ""
echo "=== STRESS TEST CONCLUÍDO ==="

# Salva relatório JSON
cat > "$REPORT" <<EOF
{
  "duração_min": $DURATION_MIN,
  "nós": $NUM_NODES,
  "rodadas": $ROUND,
  "sows": $TOTAL_SOWS,
  "recalls": $TOTAL_RECALLS,
  "isotope_puts": $TOTAL_ISOPUT,
  "isotope_gets": $TOTAL_ISOGET,
  "signals": $TOTAL_SIGNALS,
  "timestamp": $(date +%s)
}
EOF
echo "Relatório salvo em: $REPORT"
