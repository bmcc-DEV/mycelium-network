# Candidatos a esporocarpo voluntário

Critério verde: IP público real + `probe-sporocarp.sh` → `"tcp":"ok"` + `verify-sporocarp.sh` exit 0.

**Hybrid Theory:** enquanto a tabela não tem verde, a folha CGNAT já semeia via Nostr+QEL (`mycelium sow --hybrid`). O voluntário desbloqueia **mesh live** (circuit relay), não o mailbox.

| Nome | Contacto | ISP | IPv4 público? | IPv6 aberto? | Port-forward? | Máquina | Estado |
|------|----------|-----|---------------|--------------|---------------|---------|--------|
| TushiBook (Bruno) | — | Vivo CGNAT | não (WAN≠LAN) | ICMP ok / TCP drop | não | casa | **folha** — não candidato |
| Amigo fibra | | | ? | ? | ? | PC | prospecto |
| Uni / lab | | | ? | ? | ? | servidor | prospecto |
| Familiar | | | ? | ? | ? | NAS/PC | prospecto |
| Hackerspace | | | ? | ? | ? | rack | prospecto |

## Checklist operacional (pista A)

### Agora (outreach)

1. [ ] Preencher **3–5 linhas** na tabela com nome + contacto real (não deixar só “Amigo fibra”).
2. [ ] Copiar [`pitch_voluntario.txt`](pitch_voluntario.txt) → WhatsApp / email / Signal (uma mensagem por prospecto).
3. [ ] Anotar data do envio na coluna Estado (`contacto enviado YYYY-MM-DD`).

### Quando responderem “sim”

4. [ ] WAN do router vs `curl ifconfig.me` — se diferente → CGNAT (marcar e passar ao próximo).
5. [ ] Se WAN == público → port-forward TCP(+UDP) **4001** → PC do voluntário.
6. [ ] Eles sobem listen; tu no **5G**:
   `./scripts/probe-sporocarp.sh <IP> 4001 telemovel-5g > proof.json`
7. [ ] Eles: `./scripts/verify-sporocarp.sh 4001 proof.json` →
   `MYCELIUM_REACHABLE=1 ./scripts/run-public-seed.sh`
8. [ ] Tu (folha): `./scripts/run-folha.sh` com bootstrap `…/esporocarp` → **`vizinhos >= 1`**.
9. [ ] Registar resultado em [`matriz_transporte_nat.md`](matriz_transporte_nat.md).

## Multiaddr canónico (quando verde)

```text
# /dns4/….duckdns.org/tcp/4001/p2p/PEERID/esporocarp
```

Pitch: [`pitch_voluntario.txt`](pitch_voluntario.txt) · Hybrid: [`nostr-qel.md`](nostr-qel.md) · Demo: `./scripts/hybrid-demo.sh`
