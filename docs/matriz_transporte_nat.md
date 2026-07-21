# Matriz transporte × NAT × sucesso

Ordem recomendada: TCP → QUIC → WebRTC-direct → circuit → mailbox.

| Transporte | Alvo TCP aberto | Alvo UDP aberto | Folha cone | Folha simétrica | Sem 3º peer |
|------------|-----------------|-----------------|------------|-----------------|-------------|
| TCP direto | ✅ | — | ❌ inbound | ❌ | ❌ WAN |
| QUIC direto | — | ✅ | ⚠️ | ❌ geralmente | ❌ WAN |
| WebRTC-direct | — | ✅ | ✅/⚠️ STUN | ⚠️/❌ | ❌ |
| Circuit relay | ✅ relay | ✅ relay | ✅ | ✅ | ❌ |
| Mailbox DTN | — | — | ✅ async | ✅ async | ✅ async* |

\* Mailbox só após o nó já ter entrada na DHT (pós-bootstrap).

## Experimentos (preencher)

| Data | From | To | Transporte | Resultado | Log/status |
|------|------|-----|------------|-----------|------------|
| 2026-07-20 | LAN webrtc-test | seed local | TCP | ✅ vizinhos=2 | terminal 22 |
| 2026-07-20 | folha CGNAT (casa) | outro home `/tmp/myc-b` | Nostr QEL (wss) | ✅ plot reconstruído | sow→recall shards 3/3; Hybrid Theory B |
| | 5G | voluntário TCP 4001 | | ⏳ | pista A — falta candidato verde |
| | 5G | voluntário QUIC 4001 | | ⏳ | |
| | 5G | voluntário webrtc 4002 | | ⏳ | |
