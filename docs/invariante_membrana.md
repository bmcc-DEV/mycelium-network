# Invariante de membrana

```text
NUNCA anunciar /esporocarp sem prova externa válida.
```

## Formalização operacional

```text
reachable =
  external_probe.tcp(port) == ok
  (opcional: quic/webrtc probes futuros)

proof_from != self          # 5G / outra rede
proof_age recomendada < 24h # refresh periodicamente

IF reachable:
    membrane pode ser esporocarp
    MYCELIUM_REACHABLE=1
    publish seed com /esporocarp
ELSE:
    membrane = folha | floresta | raiz
    NÃO anunciar /esporocarp
    withdraw / não refresh Spore Bank
```

## Mentiras perigosas

| Mentira | Efeito |
|---------|--------|
| `MYCELIUM_REACHABLE=1` sem prova | folhas dialam o impossível |
| Anunciar IP CGNAT como público | timeouts em massa |
| Anunciar relay sem `--sporocarp`/`--relay` | circuit falha |
| Horizon público | exposição do plano de controlo |
| PeerId errado no seedbook | bootstrap falha |
| Proof eterna sem refresh | esporocarpo fantasma |

## Gate no repo

```bash
./scripts/probe-sporocarp.sh HOST 4001 from > proof.json   # externo
./scripts/verify-sporocarp.sh 4001 proof.json              # candidato
# só então:
MYCELIUM_REACHABLE=1 ./scripts/run-public-seed.sh
```

`curl ifconfig.me` **nunca** basta.
