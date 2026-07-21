# Engenharia Reversa do Bloqueio — índice operacional

> **O TushiBook deixa de ser alvo ofensivo. É sensor e folha.  
> O alvo real é a ausência de um terceiro peer alcançável.**

## Regras de engajamento

| Regra | Motivo |
|-------|--------|
| Só testar nós próprios ou com autorização | Evitar scanning/abuso |
| `MYCELIUM_REACHABLE=1` só com `proof.json` | Evitar envenenar Spore Bank |
| `curl ifconfig.me` ≠ reachability | Só IP aparente |
| Horizon `:7474` nunca público | Plano de controlo local |
| Token de controlo ≠ seedbook | Auth local, não trust de rede |

## Ordem de ataque

1. Social — [`candidatos.md`](candidatos.md) + [`pitch_voluntario.txt`](pitch_voluntario.txt)  
2. Verify — `probe-sporocarp.sh` (5G) → `verify-sporocarp.sh` (candidato)  
3. Sporocarp — `MYCELIUM_REACHABLE=1 ./scripts/run-public-seed.sh`  
4. Folhas — casa + telemóvel com `seeds/mainnet.txt`  
5. Lattice — `sow` / `recall` cruzado  
6. Depois — QUIC / WebRTC / mailbox DTN  

## Scripts

```bash
# Externo (5G):
./scripts/probe-sporocarp.sh 203.0.113.10 4001 telemovel-5g > proof.json

# No voluntário:
./scripts/verify-sporocarp.sh 4001 proof.json
# → MYCELIUM_REACHABLE=1

MYCELIUM_REACHABLE=1 ./scripts/run-public-seed.sh
./scripts/export-seed.sh ~/.local/share/mycelium-seed >> seeds/mainnet.txt
```

## Entregáveis

| Ficheiro | Papel |
|----------|--------|
| [`mapa_cpe_vivo.md`](mapa_cpe_vivo.md) | Onde o SYN morre |
| [`matriz_membranas.md`](matriz_membranas.md) | direct / circuit / mailbox |
| [`matriz_transporte_nat.md`](matriz_transporte_nat.md) | TCP/QUIC/WebRTC/relay |
| [`live_vs_dtn.md`](live_vs_dtn.md) | Tempo real vs store-and-forward |
| [`invariante_membrana.md`](invariante_membrana.md) | Nunca `/esporocarp` sem prova |
| [`candidatos.md`](candidatos.md) | Lista social |
| [`pitch_voluntario.txt`](pitch_voluntario.txt) | Pitch 5 linhas |
| [`volunteer-sporocarp.md`](volunteer-sporocarp.md) | Runbook do voluntário |

## Captura CPE (sensor TushiBook)

```bash
./scripts/cpe-sensor.sh --pcap
# Em paralelo (5G): ./scripts/probe-sporocarp.sh <ip4|ip6> 4001 telemovel-5g
# Preencher: docs/mapa_cpe_vivo.md
```

Frase-guia: *Não atacar o TushiBook. Atacar a ausência de terceiro alcançável.*
