# Candidatos a esporocarpo voluntário

Critério verde: IP público real + probe TCP ok + verify exit 0.

**Hybrid Theory:** sem verde, a folha já semeia via `mycelium sow --hybrid`. O voluntário desbloqueia **mesh live**.

## Pipeline automático (usa isto)

```bash
# 0) Neste PC (candidato?): CGNAT?
./scripts/volunteer-pipeline.sh cgnat-check

# 1) Outreach — pitch no clipboard + marca estado
./scripts/volunteer-pipeline.sh pitch "Amigo fibra"

# 2) No VOLUNTÁRIO (depois do port-forward): abre listen para o probe
./scripts/volunteer-pipeline.sh prep-listen

# 3) No telemóvel 5G:
./scripts/volunteer-pipeline.sh probe <IP_PUBLICO>
# → gera proof.json

# 4) No PC do VOLUNTÁRIO (com proof.json):
./scripts/volunteer-pipeline.sh onboard proof.json
# → verify + seed + export → seeds/mainnet.txt

# 5) Na FOLHA (Bruno / TushiBook):
./scripts/volunteer-pipeline.sh folha-attach '/ip4/…/tcp/4001/p2p/…/esporocarp'
# → seed book + restart + espera vizinhos>=1

# Estado
./scripts/volunteer-pipeline.sh status
./scripts/volunteer-pipeline.sh mark "Amigo fibra" "verde"
```

Estado persistente: [`candidatos.state.json`](candidatos.state.json) (actualizado por `pitch` / `mark`).

## Tabela (manual / referência)

| Nome | Contacto | ISP | IPv4 público? | IPv6 aberto? | Port-forward? | Máquina | Estado |
|------|----------|-----|---------------|--------------|---------------|---------|--------|
| TushiBook (Bruno) | — | Vivo CGNAT | não (WAN≠LAN) | ICMP ok / TCP drop | não | casa | **folha** — não candidato |
| Amigo fibra | | | ? | ? | ? | PC | prospecto |
| Uni / lab | | | ? | ? | ? | servidor | prospecto |
| Familiar | | | ? | ? | ? | NAS/PC | prospecto |
| Hackerspace | | | ? | ? | ? | rack | prospecto |

## O que ainda é humano

Só o passo **enviar a mensagem** e o voluntário **abrir a porta no router**. O resto (prova, seed, mainnet, attach da folha, matriz) está no pipeline.

Pitch: [`pitch_voluntario.txt`](pitch_voluntario.txt) · Ops: [`volunteer-sporocarp.md`](volunteer-sporocarp.md) · Demo Nostr: `./scripts/hybrid-demo.sh`
