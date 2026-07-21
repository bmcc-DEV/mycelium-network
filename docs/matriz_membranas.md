# Matriz de membranas — direct | circuit | mailbox

Atualizar após cada experimento com esporocarpo voluntário.

| Par | Direct | Circuit (esporocarpo) | Mailbox DTN | Evidência |
|-----|--------|----------------------|-------------|-----------|
| LAN ↔ LAN | ✅ | — | fallback | `vizinhos: 2` TushiBook 2026-07-20 |
| Folha Vivo ↔ Esporocarpo aberto | ⏳ | ⏳ | fallback | aguarda voluntário |
| Folha 5G ↔ Esporocarpo aberto | ⏳ | ⏳ | fallback | aguarda voluntário |
| Folha Vivo ↔ Folha 5G | ❌ sem 3º | ⏳ via relay | ✅ async | tese atual |
| Dois NAT cone | ⚠️ ICE | ✅ | ✅ | |
| Dois NAT simétricos | ❌ | ✅ | ✅ | |
| Ambos CGNAT | ❌ | ✅ se relay | ✅ | |
| Sem terceiro aberto | ❌ WAN | ❌ | ✅ | mailbox só pós-bootstrap |

## Notas

- Bootstrap **sempre** precisa de um peer já alcançável (mDNS / seed / DNS TXT).
- Circuit só depois de dial outbound ao `/esporocarp`.
- `MYCELIUM_REACHABLE=1` sem `proof.json` → envenena esta matriz.
