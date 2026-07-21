# Esporocarpo voluntário (zero VPS)

Premissa: **nenhum nó é servidor central**. Casa atrás de CGNAT/firewall
(ex. Vivo / TushiBook) é **folha/sensor** — nunca `/esporocarp` sem prova.

**Hybrid Theory (A + B):**

- **Pista B (já útil sem voluntário):** outbound Nostr + QEL + GhostID + blockstore
  local (`mycelium sow --hybrid` / `recall --hybrid`) — ver [`nostr-qel.md`](nostr-qel.md).
  Relays públicos (`wss://`) substituem a mailbox WAN.
- **Pista A (este doc):** o esporocarpo voluntário desbloqueia **mesh live** libp2p
  (circuit relay, Horizon remoto). Sem proof → sem `/esporocarp`.

Enquanto a lista em [`candidatos.md`](candidatos.md) não tem verde, a folha CGNAT
já semeia via Nostr; o voluntário não é pré-requisito do mailbox.

Índice do ciclo: [`engenharia-reversa-bloqueio.md`](engenharia-reversa-bloqueio.md) ·
[`invariante_membrana.md`](invariante_membrana.md) · [`rizomorphs.md`](rizomorphs.md)
· [`nostr-qel.md`](nostr-qel.md)

## Gate (obrigatório)

```bash
# 1) No telemóvel 5G (outra rede):
./scripts/probe-sporocarp.sh 203.0.113.10 4001 telemovel-5g > proof.json

# 2) No peer voluntário (com daemon a escutar 0.0.0.0:4001):
./scripts/verify-sporocarp.sh 4001 proof.json
# → MYCELIUM_REACHABLE=1

# 3) Só então:
MYCELIUM_REACHABLE=1 ./scripts/run-public-seed.sh
./scripts/export-seed.sh ~/.local/share/mycelium-seed >> seeds/mainnet.txt
```

`curl ifconfig.me` **não** é prova. Placeholders tipo `IP_DO_AMIGO` são rejeitados.

## Folhas (casa / 5G)

```bash
# Casa (TushiBook / CGNAT) — script dedicado, sem --sporocarp:
./scripts/run-folha.sh
# ou:
mycelium --home ~/.local/share/mycelium daemon \
  --seed-file ./seeds/mainnet.txt \
  --no-mdns

# Telemóvel / outro home:
mycelium --home ~/mycelium daemon \
  --seed-file ./seeds/mainnet.txt \
  --no-mdns

mycelium status   # vizinhos >= 1 ; membrana ≠ esporocarp
```

Se um daemon antigo ainda tiver `--sporocarp --announce-ip …` sem proof: `pkill -f '--sporocarp'` e sobe de novo com `run-folha.sh`.

## Lattice cruzado

```bash
# casa — Hybrid (Nostr mailbox + blockstore local)
mycelium sow --message "floresta-viva" --hybrid
# telemóvel / outro home — cola o Qm… completo (sem <>)
mycelium --home ~/mycelium recall --plot Qm… --hybrid
```

Sem `--hybrid`, sow/recall locais (LAN / mesh) continuam a funcionar quando há vizinhos.

## Critérios de verde

| Teste | Sucesso |
|-------|---------|
| `probe-sporocarp.sh` | `"tcp":"ok"` |
| `verify-sporocarp.sh` | exit 0 + `MYCELIUM_REACHABLE=1` |
| Folha 5G / casa `status` | `vizinhos >= 1` |
| `sow` + `recall` | Plot no outro nó |
| Seedbook | linha `…/esporocarp` |

## O que não fazer

- Não promover o TushiBook/Vivo a esporocarpo WAN  
- Não reintroduzir UPnP  
- Não expor Horizon (`:7474`) na WAN  
- Não meter `MYCELIUM_CONTROL_TOKEN` no seedbook  

Pitch: [`pitch_voluntario.txt`](pitch_voluntario.txt) · Lista: [`candidatos.md`](candidatos.md)
