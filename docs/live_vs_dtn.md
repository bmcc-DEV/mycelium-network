# Live mesh vs DTN / store-and-forward

## LIVE MESH (tempo real)

| Função | Live? | Motivo |
|--------|-------|--------|
| mDNS LAN | sim | descoberta local |
| dial / identify | sim | handshake |
| relay circuit | sim | sessão stateful |
| `status` / control socket | sim | plano de controlo |
| Horizon local | sim | proxy local |
| gossipsub | sim/semi | propagação rápida |
| DHT query | sim/semi | lookup |
| WebRTC ICE | sim | negociação |

## DTN / STORE-AND-FORWARD

| Função | DTN? | Motivo |
|--------|------|--------|
| Plots Giggs | ✅ | content-addressed |
| `sow` / `recall` | ✅ | propagação eventual |
| Signals TheField | ✅ | quórum pode ser async |
| Isotope atoms | ✅ | LWW eventual |
| Vacuum layers | ✅ | content-addressed |
| mailbox DHT | ✅ | desenhado para isso |
| Entropy secrets | ⚠️ | preferir meia-vida, não persistir |

## Split

```text
LIVE  = vizinhos, relay, dial, ICE, status, horizon
DTN   = Plots, Signals, Isotope, artifacts, mailbox
```

Sem terceiro peer alcançável: **só DTN físico** (encontros LAN) ou falha explícita de bootstrap WAN — sem milagre.
