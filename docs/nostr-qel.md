# Nostr + QEL + GhostID (Fases 1–3) + Hybrid Theory

Desbloqueia o TushiBook / CGNAT **sem** inbound TCP e **sem** anunciar `/esporocarp` sem proof.

## Ideia

| Camada | Papel |
|--------|--------|
| **Nostr** (`wss://` outbound :443) | Mailbox + discovery — funciona atrás da Vivo |
| **QEL** (Shamir k-of-n) | Fragmenta o spore print; relays vêem no máximo 1 shard |
| **GhostID** (secp256k1 Schnorr) | Assina eventos Nostr; zero-fill no drop |
| **ipfs-blocks** (Fase 4 local) | Blockstore Blake3 no disco — sem bitswap WAN |

Esporocarpo voluntário (`MYCELIUM_REACHABLE`) **mantém-se** para mesh live libp2p. Nostr é caminho paralelo.

## Hybrid Theory (A + B)

Duas pistas complementares + um orquestrador:

| Pista | Papel | Critério |
|-------|--------|----------|
| **A — Mesh live** | Voluntário + proof → circuit relay | `vizinhos >= 1` na folha |
| **B — Mailbox / store** | Nostr QEL + blockstore local | `recall --hybrid` reconstrói |

```bash
# Semear: local + Nostr (k shards) + put spore print em ipfs-blocks/
mycelium sow --message "floresta" --hybrid

# Reconstruir (ordem): SporeBank local → Nostr → ipfs-blocks/
mycelium --home /tmp/folha-b recall --plot Qm… --hybrid
```

`--hybrid` implica QEL 3,7 + Nostr + GhostID + blockstore. A paisagem
[`mycelium-distancebridge`](../crates/mycelium-distancebridge) escolhe hints
(`select_transports` → `hybrid_hints_from_landscape`). O daemon corre um tick
Physarum a cada 5s (`status` → `physarum`).

**Fora de âmbito neste sprint:** bitswap WAN, DistanceBridge físico, LoRa/SMS, rewrite completo Godunov.

Demo: [`../scripts/hybrid-demo.sh`](../scripts/hybrid-demo.sh) · Social: [`candidatos.md`](candidatos.md) · Pitch: [`pitch_voluntario.txt`](pitch_voluntario.txt)

## Correções importantes

- `ContentId` é Blake3 32 bytes com prefixo cosmético `Qm` — **não** é CID IPFS/multihash.
- NIP-94 tag `i` = `ContentId.to_string()`; tag `x` = hex Blake3.
- Payload por evento ≤ 64 KiB (igual à mailbox DHT).
- Shards usam kind **31234** (addressable NIP-33) com tag **`d` = `{ContentId}:{index}`** para não se substituírem no mesmo GhostId. Cifra opcional **NIP-44** com `--to <pubkey>`. Indexação secundária via tag **`i`**.

## Crates

```
crates/mycelium-ghostid/   # Fase 1
crates/mycelium-qel/       # Fase 2
crates/mycelium-nostr/     # Fase 3
crates/mycelium-ipfs/      # Fase 4 mínima (blockstore local)
```

Feature CLI (ligada por default): `nostr` → inclui ghostid, qel, nostr, ipfs.

## Uso

```bash
# Instalar CLI nova (PATH antigo não tem --qel/--nostr/--hybrid):
cargo install --path cli/mycelium-cli --force
# ou: ./target/release/mycelium …

# Daemon folha
./scripts/run-folha.sh

# Hybrid (recomendado em CGNAT)
mycelium sow --message "floresta" --hybrid

# Só Nostr (sem blockstore)
mycelium sow --message "floresta" --qel 3,7 --nostr --ghost

# Noutro home (cola o Qm… completo — sem <> nem reticências):
mycelium --home /tmp/folha-b recall --plot Qmabc… --hybrid
```

Relays default: `relay.damus.io`, `nos.lol`, `relay.snort.social`, `relay.primal.net`.
(`relay.nostr.band` removido do default — timeouts frequentes.)

## Testes

```bash
cargo test -p mycelium-ghostid -p mycelium-qel -p mycelium-nostr -p mycelium-ipfs -p mycelium-cli
```

Integração contra relays reais: usar `mycelium sow --hybrid` com rede; CI não depende de wss.

## Roadmap (fases futuras — só doc)

| Fase | Entrega | Estado |
|------|---------|--------|
| tropical | `mycelium-tropical` Max-Plus / Physarum / CFL | ✅ crate |
| pqc | `mycelium-pqc` ML-KEM-1024 | ✅ crate |
| bridge0 | `mycelium-distancebridge` landscape | ✅ fase 0 |
| 4b | bitswap de rede (CID real opcional) | só doc |
| 5–6 | Integração recall bitswap + pin Sclerotium | só doc |
| 7 | DistanceBridge físico (LoRa/BT) | só doc |
| 8–9 | LoRa / SMS drivers | só doc |
| 10 | Demo ponta a ponta multi-path | só doc |
| daemon-rsa | Loop Godunov no organism | só doc |

Ponte ET-COSMIC: [`et-cosmic-bridge.md`](et-cosmic-bridge.md).

## Matriz vs esporocarpo

| Necessidade | Nostr QEL / hybrid | Esporocarpo voluntário |
|-------------|--------------------|-------------------------|
| Sow/recall pequeno atrás de CGNAT | Sim | Não necessário |
| Circuit relay NAT↔NAT live | Não | Sim |
| Horizon / chambers remotos | Não | Sim (mesh) |
| Proof `MYCELIUM_REACHABLE` | Não | Obrigatório |
