# 🍄 Mycelium Network — O Substrato Vivo do The Lattice

> *"A floresta não é uma coleção de árvores. É uma rede subterrânea de fungos que alimenta, comunica e cura."*

Nuvem P2P viva em Rust: hifas (libp2p), feromônios, Spore Bank, e o fluxo Lattice ponta a ponta.

## Quick start

```bash
# 1. Planta a semente (identidade + recursos em disco)
cargo run -p mycelium-cli --release -- --home /tmp/node-a sprout --contribute 2cpu,4gb,100gb

# 2. Desperta o daemon (Event Horizon HTTP em :7474)
cargo run -p mycelium-cli --release -- --home /tmp/node-a daemon --contribute 2cpu,4gb,100gb --horizon-port 7474

# 3. Noutro terminal — fluxo Lattice → Chamber viva
cargo run -p mycelium-cli --release -- --home /tmp/node-a sow --message "hello"
# anote o ContentId (Qm…)
cargo run -p mycelium-cli --release -- --home /tmp/node-a signal --plot Qm… --quorum 1 --ion webapp

# 4. Acesse o Ion pelo Singularity (proxy HTTP real)
curl -s http://127.0.0.1:7474/webapp/ | jq .
```

Demo automatizada do horizon: `./scripts/horizon-demo.sh`

Dois nós com bootstrap remoto:

```bash
# terminal A — seed na porta 4001
mycelium --home /tmp/a daemon --listen /ip4/0.0.0.0/tcp/4001

# terminal B
mycelium --home /tmp/b daemon --bootstrap /ip4/IP_DE_A/tcp/4001/p2p/PEERID_DE_A
```

Bootstrap além da LAN (catálogo HTTP + `/dnsaddr/`):

```bash
mycelium seeds fetch --url ./seeds/mainnet.example.txt
mycelium daemon --public-bootstrap --bootstrap-url https://seu.host/seeds.txt
# ou arquivo local:
mycelium daemon --seed-file ./seeds/mainnet.example.txt --listen /ip4/0.0.0.0/tcp/4001
```

Vacuum usa **bubblewrap** por padrão quando `bwrap` está no PATH; layers content-addressed em `{home}/layers/` e limites soft de RAM (`RLIMIT_AS`).

Rede só com seed book (sem mDNS / sem `--bootstrap` manual):

```bash
./scripts/seedbook-demo.sh
```

Demos: `./scripts/e2e-demo.sh` · `./scripts/horizon-demo.sh` · `./scripts/seedbook-demo.sh` · `./scripts/isotope-decay-demo.sh`

## Fluxo ponta a ponta

```
Giggs sow Plot → Spore Bank (disco + DHT) → gossip hifas
       → TheField Signal + quórum
       → Inertia Vectors (build/test/deploy)
       → Vacuum Chamber → Plasma Ion → Singularity Event Horizon
```

## Comandos CLI

| Comando | Função |
|---|---|
| `sprout` | Inicializa identidade/recursos sem subir rede |
| `daemon` | Organismo persistente (Ctrl-C ou `shutdown`) |
| `status` | Estado vivo (socket) ou offline (disco) |
| `sow` | Semeia Plot → Spore Bank + gossip/DHT |
| `signal` | Emite Signal de pipeline no TheField |
| `resonate` | Contribui para o quórum de um Signal |
| `recall` | Lê Plot local; se ausente, consulta DHT |
| `bootstrap` | Dial explícito a um peer remoto |
| `seeds list/add/fetch` | Seed book (bootstrap público) |
| `isotope-put` / `isotope-get` | Estado Isotope (anel 4 + Decay pelas hifas) |
| `deploy` | One-shot: sow → signal → URL do Event Horizon |
| `shutdown` | Hiberna o daemon (estado fica em disco) |

## Crates

| Crate | Papel |
|---|---|
| `mycelium-core` | NodeId, ContentId, Resources, FruitingBody |
| `mycelium-hyphae` | libp2p QUIC/TCP, mDNS, Kademlia, gossip, métricas, bootstrap |
| `mycelium-pheromones` | Identidade ed25519 |
| `mycelium-nutrients` | Ledger ATP/Enzymes/Mycelia/Spores/Resilience |
| `mycelium-sporebank` | Plots em disco + chaves DHT |
| `mycelium-node` | Daemon, protocolo Lattice, socket de controle |
| `giggs` … `plasma` | Componentes do Lattice |
| `mycelium-cli` | Binário `mycelium` |

## Persistência (`MYCELIUM_HOME` / `--home`)

```
gland.seed          identidade (PeerId estável)
ledger.json         nutrientes
resources.json      contribuição
organism.json       field, ions, métricas de hifas, bootstrap
nucleus.json        Isotope (átomos LWW)
layers/             Vacuum layers content-addressed
builds/             workbench do Inertia
sporebank/plots/    Plots content-addressed
listen_addrs.json   multiaddrs para bootstrap de pares
seeds.txt           seed book mesclado
mycelium.sock       plano de controle do daemon
```

## Publicar um seed

```bash
./scripts/run-public-seed.sh          # sobe seed + imprime multiaddr
# encaminhe TCP 4001 no NAT, depois:
# edite seeds/mainnet.txt e push
mycelium daemon --public-bootstrap --no-mdns
```

Seed 24/7: `sudo ./scripts/install-seed.sh` (inclui `--relay`).  
Docs: [docs/ops-seed.md](docs/ops-seed.md) · [docs/protocol.md](docs/protocol.md) · Console: `http://127.0.0.1:7474/console`  
Auth do socket: `MYCELIUM_CONTROL_TOKEN=…`

## Desenvolvimento

```bash
cargo build --workspace
cargo test --workspace
./scripts/e2e-demo.sh
```

## Licença

MIT OR Apache-2.0 — veja [LICENSE-MIT](LICENSE-MIT) e [LICENSE-APACHE](LICENSE-APACHE).

> *"O futuro da computação não é construir castelos de silício. É plantar florestas de código."*
