# Testes de Realidade — Mycelium Network

## ✅ 30 Jul 2026 — CGNAT real (Vivo) ↔ 5G

**Resultado:** CONECTADO. Primeira mesh CGNAT↔CGNAT via Nostr transport.

```
casa: Vivo CGNAT (2804:7f7:e03a:be::/64)
5g:   Claro/Tim (2804:214:8857:b801::/64)

nostr-relay: wss://nos.lol
vizinhos:    1 (bidirecional)
sow → recall: plot "ponte-5g" entregue com sucesso
tempo descoberta: ~4s (45s tick + handshake)
```

**Comandos usados:**

```bash
# Terminal 1 — Casa (Vivo CGNAT)
./target/debug/mycelium --home ~/mycelium-casa daemon --no-mdns --nostr-transport

# Terminal 2 — 5G (mesmo notebook, rede diferente)
mkdir -p /tmp/mycelium-5g
./target/debug/mycelium --home /tmp/mycelium-5g daemon --no-mdns --nostr-transport

# ~60s depois:
# vizinhos: 1

# Casa → 5G
./target/debug/mycelium --home ~/mycelium-casa sow \
  --message "ponte-5g" --path "teste.txt" \
  --content "Conexao CGNAT direta sobre Nostr"
# Qm3668c54ee70a63c06843b1b967d0462a9a76ad78d136a2c4a1f1980e101a0e07

./target/debug/mycelium --home /tmp/mycelium-5g recall \
  --plot Qm3668c54ee70a63c06843b1b967d0462a9a76ad78d136a2c4a1f1980e101a0e07
# ✅ plot 3668c54e — "ponte-5g" (1 leaves)
```

**Comprovação:** sem VPS, sem circuit relay, sem mDNS, sem STUN.
Dependência externa: único relay Nostr público (`wss://nos.lol`).

---

## Setup básico

```bash
# Build com tudo
cargo build --features nostr,nostr-transport,pqc-transport -p mycelium-cli

# alias pra facilitar
alias M='$PWD/target/debug/mycelium --home'
```

---

## 1. CGNAT real ↔ CGNAT real (prioridade máxima)

### Cenário A — Nostr transport (sem VPS, sem relay circuit)

**Nó A (Casa CGNAT):**
```bash
# Terminal 1
M /tmp/mycelium-a sprout --contribute "2cpu,4gb,50gb"
M /tmp/mycelium-a daemon --nostr-transport --nostr-relay wss://nos.lol --no-mdns
```

**Nó B (Outra casa CGNAT / 5G):**
```bash
# Terminal 2
M /tmp/mycelium-b sprout --contribute "1cpu,2gb,10gb"
M /tmp/mycelium-b daemon --nostr-transport --nostr-relay wss://nos.lol --no-mdns
```

**Verificar vizinhança:**
```bash
# Em ambos
M /tmp/mycelium-a status
M /tmp/mycelium-b status
# Procurar: "vizinhos >= 1" e "nostr-transport activo"
```

**Troca de Plot via Nostr:**
```bash
# Nó A
M /tmp/mycelium-a sow --message "ola-mundo" --path "hello.txt" --content "CGNAT rulez" --nostr --ghost

# Copiar o ContentId (Qm...)

# Nó B
M /tmp/mycelium-b recall --plot Qm<id> --nostr --qel-threshold 3
```

**CandidateRelay CGNAT↔CGNAT:**
```bash
# Nó A — mostra ghost ID
M /tmp/mycelium-a candidate whoami
# Copiar pk_hex (64 hex chars)

# Nó A — escuta
M /tmp/mycelium-a candidate listen

# Nó B — envia mensagem cifrada
M /tmp/mycelium-b candidate send --to <pk_hex_do_A> -m "oi do outro CGNAT"

# Nó B — descobre peers
M /tmp/mycelium-b candidate --once --relay wss://nos.lol
```

### Cenário B — Circuit relay via seed público

**Seed (VPS / IP público):**
```bash
./scripts/run-public-seed.sh
# ou manual:
M /tmp/mycelium-seed sprout
MYCELIUM_REACHABLE=1 M /tmp/mycelium-seed daemon \
  --listen /ip4/0.0.0.0/tcp/4001 \
  --sporocarp --public-bootstrap --no-mdns
```

**Folha CGNAT A:**
```bash
M /tmp/mycelium-a daemon --seed-file seeds/mainnet.txt --no-mdns
```

**Folha CGNAT B:**
```bash
M /tmp/mycelium-b daemon --seed-file seeds/mainnet.txt --no-mdns
```

**Verificar circuito:**
```bash
M /tmp/mycelium-a status  # active_relay deve mostrar PeerId do seed
M /tmp/mycelium-b status  # active_relay deve mostrar PeerId do seed
M /tmp/mycelium-a sow --message "via-relay" --path "test.txt" --content "hello"
M /tmp/mycelium-b recall --plot <Qm_id>
```

### Cenário C — Kill e recuperação

```bash
# No meio de uma transferência Nostr, matar o daemon A:
pkill -f "mycelium.*home.*a"

# Subir de novo:
M /tmp/mycelium-a daemon --nostr-transport --no-mdns

# Verificar se o Plot chegou no B mesmo com A tendo caído:
M /tmp/mycelium-b status  # vizinhos ainda >= 1?
```

---

## 2. Persistência + Recovery

```bash
# Prepara
M /tmp/mycelium-persist sprout

# Sobe
M /tmp/mycelium-persist daemon --no-mdns &  # background
sleep 2

# Cria dados
M /tmp/mycelium-persist sow --message "persist-test" --path "data.txt" --content "42"
M /tmp/mycelium-persist isotope-put --key "foo" --value "bar"
M /tmp/mycelium-persist status

# Desliga limpo
M /tmp/mycelium-persist shutdown
sleep 1

# Sobe de novo
M /tmp/mycelium-persist daemon --no-mdns &
sleep 2

# Verifica
M /tmp/mycelium-persist status
# NodeId deve ser o mesmo
# plots >= 1
# isotope-get --key "foo" deve retornar "bar"

# Shutdown limpo
M /tmp/mycelium-persist shutdown

# Teste crash (kill -9):
M /tmp/mycelium-persist daemon --no-mdns &
sleep 2
kill -9 $(cat /tmp/mycelium-persist/mycelium.pid)
sleep 1

# Sobe de novo — ver o que sobreviveu
M /tmp/mycelium-persist daemon --no-mdns &
sleep 2
M /tmp/mycelium-persist status
M /tmp/mycelium-persist isotope-get --key "foo"

# Shutdown
M /tmp/mycelium-persist shutdown
```

---

## 3. Fluxo Lattice completo multi-nó

```bash
# Terminal 1 — Nó A (faz deploy)
M /tmp/lattice-a sprout --contribute "4cpu,8gb,100gb"
M /tmp/lattice-a daemon --listen /ip4/127.0.0.1/tcp/49001 --no-mdns &
sleep 2
M /tmp/lattice-a status

# Terminal 2 — Nó B (vai resonar)
M /tmp/lattice-b sprout --contribute "2cpu,4gb,50gb"
M /tmp/lattice-b daemon --listen /ip4/127.0.0.1/tcp/49002 \
  --bootstrap /ip4/127.0.0.1/tcp/49001 --no-mdns &
sleep 3

# Nó B conectou?
M /tmp/lattice-b status  # vizinhos >= 1

# Terminal 1 — A faz deploy
M /tmp/lattice-a deploy \
  --message "meu-primeiro-ion" \
  --path "build.sh" \
  --content '#!/bin/sh
echo "<h1>Vivo</h1>" > dist/index.html
' \
  --ion webapp --name ci --quorum 1

# Isso deve printar a URL do Event Horizon

# Terminal 2 — B ressoa o signal (pegar signal_id do status do A)
M /tmp/lattice-a status  # signals >= 1, copiar Qm...
M /tmp/lattice-b resonate --signal Qm<signal_id>

# Verificar se o Ion subiu
curl http://127.0.0.1:7474/
curl http://127.0.0.1:7474/webapp/
curl http://127.0.0.1:7474/webapp/index.html

# Migrar o Ion
M /tmp/lattice-a ion-migrate --ion webapp --target $(M /tmp/lattice-b status | grep NodeId | awk '{print $2}')

# Verificar se a URL continua funcionando
curl http://127.0.0.1:7474/webapp/
```

---

## 4. Isotope + Decay sob gossip

```bash
# 3 nós no mesmo anel
M /tmp/iso-a sprout
M /tmp/iso-b sprout
M /tmp/iso-c sprout

# Sobe em portas diferentes
M /tmp/iso-a daemon --listen /ip4/127.0.0.1/tcp/49101 --no-mdns &
M /tmp/iso-b daemon --listen /ip4/127.0.0.1/tcp/49102 \
  --bootstrap /ip4/127.0.0.1/tcp/49101 --no-mdns &
M /tmp/iso-c daemon --listen /ip4/127.0.0.1/tcp/49103 \
  --bootstrap /ip4/127.0.0.1/tcp/49101 --no-mdns &
sleep 4

# Escrita concorrente na mesma chave
M /tmp/iso-a isotope-put --key "estado" --value "alpha" --clock 100
M /tmp/iso-b isotope-put --key "estado" --value "beta" --clock 200
M /tmp/iso-c isotope-put --key "estado" --value "gamma" --clock 150
sleep 2

# Cada um lê — o maior clock (200 "beta") deve prevalecer
M /tmp/iso-a isotope-get --key "estado"
M /tmp/iso-b isotope-get --key "estado"
M /tmp/iso-c isotope-get --key "estado"

# Decay: ler chave que não está no shard local
# A itself auto-decide qual shard possui
M /tmp/iso-a isotope-put --key "chave-importante" --value "segredo"
sleep 2
M /tmp/iso-b isotope-get --key "chave-importante"  # deve fazer Decay
M /tmp/iso-c isotope-get --key "chave-importante"

# Limpeza
for d in /tmp/iso-*; do M $d shutdown; done
```

---

## 5. Seed book + bootstrap público

```bash
# Terminal 1 — Seed público
M /tmp/seed-public sprout
MYCELIUM_REACHABLE=1 M /tmp/seed-public daemon \
  --listen /ip4/0.0.0.0/tcp/4001 \
  --sporocarp --public-bootstrap --no-mdns &
sleep 3

# Exporta seed line
M /tmp/seed-public status | grep listen

# Terminal 2 — Nó remoto (outra máquina / container)
M /tmp/folha-remota sprout
M /tmp/folha-remota daemon \
  --seed-file seeds/mainnet.txt \
  --no-mdns --public-bootstrap &
sleep 5

# Verificar descoberta
M /tmp/folha-remota status  # vizinhos >= 1
M /tmp/folha-remota seeds list  # seeds carregados

# Trocar plot via seed
M /tmp/folha-remota sow --message "descoberto-via-seed" --path "seed.txt" --content "ok"
```

---

## 6. Vacuum + isolamento real

```bash
# Deploy com conteúdo que tenta escapar do sandbox
M /tmp/vacuum-test sprout --contribute "2cpu,4gb,10gb"
M /tmp/vacuum-test daemon --listen /ip4/127.0.0.1/tcp/49201 --no-mdns &
sleep 2

# Ion que deveria estar isolado
M /tmp/vacuum-test deploy \
  --message "sandbox-test" \
  --path "build.sh" \
  --content '#!/bin/sh
echo "Tentando ler /etc/passwd..."
cat /etc/passwd > dist/leaked.txt 2>&1 || echo "ISOLADO" > dist/leaked.txt
echo "<pre>" > dist/index.html
cat dist/leaked.txt >> dist/index.html
echo "</pre>" >> dist/index.html
' \
  --ion sandbox --name test --quorum 1

# Ver resultado — se "ISOLADO" aparece, o sandbox funcionou
curl http://127.0.0.1:7474/sandbox/
curl http://127.0.0.1:7474/sandbox/index.html
```

---

## 7. Observabilidade 30min

```bash
M /tmp/monitor sprout
M /tmp/monitor daemon --listen /ip4/127.0.0.1/tcp/49301 --no-mdns &
sleep 2

# Loop de atividade
for i in $(seq 1 30); do
  M /tmp/monitor sow --message "batch-$i" --path "data.txt" --content "payload-$i"
  M /tmp/monitor isotope-put --key "batch" --value "$i" --clock $i
  sleep 60
done &

# Enquanto isso, verificar métricas
watch -n 10 "
  echo '=== METRICS ==='
  curl -s http://127.0.0.1:7474/metrics | grep mycelium
  echo ''
  echo '=== HEALTH ==='
  curl -s http://127.0.0.1:7474/health
  echo ''
  echo '=== CONSOLE ==='
  curl -s http://127.0.0.1:7474/console | grep -oP '(?<=<li>).*?(?=</li>)'
  echo ''
  du -sh /tmp/monitor/
"

# Depois de 30min, verificar:
echo "=== Tamanho do home ==="
du -sh /tmp/monitor/
echo "=== Arquivos ==="
find /tmp/monitor/ -type f | wc -l
echo "=== PID ainda vivo? ==="
cat /tmp/monitor/mycelium.pid 2>/dev/null && ps -p $(cat /tmp/monitor/mycelium.pid) > /dev/null 2>&1 && echo "SIM" || echo "MORREU"

M /tmp/monitor shutdown
```

---

## Checklist de aprovação

| Teste | Critério de aprovação |
|-------|----------------------|
| 1A | `vizinhos >= 1` sem VPS; Plot chega via Nostr |
| 1B | Circuit relay funcional; folha alcança seed |
| 1C | Daemon recupera após kill |
| 2 | NodeId, ledger, plots, isotope intactos após reboot |
| 3 | Ion sobe, responde HTTP, migra sem quebrar |
| 4 | LWW converge para clock maior; Decay responde |
| 5 | Descoberta via seed book sem mDNS |
| 6 | Sandbox impede acesso a `/etc/passwd` |
| 7 | Sem vazamento de memória por 30min+ |
