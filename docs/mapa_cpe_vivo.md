# Mapa CPE Vivo (TushiBook) — sensor

Preencher com pcap + probes externos. **Não** anunciar `/esporocarp` a partir deste host.

Actualização automática local: `./scripts/cpe-sensor.sh [--pcap]`

## Topologia

```text
externo → Internet → ASN Vivo → CGNAT? → CPE (192.168.15.1) → LAN → TushiBook
```

| Campo | Valor |
|-------|--------|
| Data (sensor local) | 2026-07-20 |
| IPv4 LAN | `192.168.15.136/24` |
| IPv4 curl (aparente) | `177.102.211.60` |
| IPv6 global (iface) | `2804:7f7:e03a:db83:399c:201f:2328:6c81/64` (+ outros) |
| Gateway | `192.168.15.1` (wlp0s20f3) |
| WAN router == curl? | **não medido na UI**; LAN ≠ curl → **NAT/CGNAT muito provável** |
| Hairpin ao público:4001 | **fail** |
| Hairpin ao público:443 | **fail** |
| Escuta local | `0.0.0.0:4001` + `:443` (TCP) — daemon activo |
| Firewall host (nft/ufw) | sem regras visíveis sem sudo / ufw n/a |

## O que passa / o que morre

| L3/L4 | LAN | Outbound WAN | Inbound WAN | Evidência | Onde morre (hipótese) |
|-------|-----|--------------|-------------|-----------|----------------------|
| IPv4 TCP 4001 | ✅ | ✅ | ❌ | nc 5G timeout (sessões anteriores) | CGNAT/CPE/upstream |
| IPv4 TCP 443 | ✅ | ✅ | ❌ | nc 5G timeout | mesma política |
| IPv6 TCP 4001 | ✅ | ✅ | ❌ | nc 5G timeout | firewall IPv6 stateful |
| IPv6 TCP 443 | ✅ | ✅ | ❌ | nc 5G timeout | firewall IPv6 stateful |
| IPv4 UDP 4001 | ✅ | ✅ | ❓ | | a confirmar com pcap |
| IPv6 UDP 4001 | ✅ | ✅ | ❓ | | a confirmar com pcap |
| ICMPv6 echo | ✅ | ✅ | ✅/⚠️ | ping6 historicamente ok | |
| LAN mDNS / TCP local | ✅ | — | — | `vizinhos: 2` | local only |
| Hairpin IPv4 | — | — | ❌ | `cpe-sensor` 2026-07-20 | CPE sem U-turn |

## Interpretação tcpdump

| Externo | Host vê | Diagnóstico |
|---------|---------|-------------|
| timeout | nenhum SYN | morre antes do host |
| timeout | SYN, sem SYN-ACK | firewall/daemon local |
| timeout | SYN + SYN-ACK | return path bloqueado |
| refused | RST | reject / porta fechada |
| succeeded | 3-way | inbound real |

## Captura (próximo passo sensor)

```bash
./scripts/cpe-sensor.sh --pcap
# noutro terminal: sudo tcpdump … (comando impresso)
# no 5G: ./scripts/probe-sporocarp.sh 177.102.211.60 4001 telemovel-5g
#        ./scripts/probe-sporocarp.sh 2804:7f7:… 4001 telemovel-5g
```

## Estado perigoso actual

Há (ou houve) daemon com `--sporocarp --announce-ip 177.102.211.60` **sem** `proof.json`.
Isso viola [`invariante_membrana.md`](invariante_membrana.md).

Acção:

```bash
mycelium --home ~/.local/share/mycelium shutdown
# depois só folha / floresta local — sem --sporocarp até haver voluntário verde
```

## Conclusão operacional

```text
membrane = floresta LAN / folha WAN
reachable = false
relay_server = false  (não anunciar)
papel = sensor + folha outbound
próximo alvo = 1 peer voluntário (docs/candidatos.md)
```
