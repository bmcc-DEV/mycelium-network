# Graph Report - .  (2026-07-20)

## Corpus Check
- Corpus is ~36,314 words - fits in a single context window. You may not need a graph.

## Summary
- 905 nodes · 2231 edges · 37 communities (23 shown, 14 thin omitted)
- Extraction: 98% EXTRACTED · 2% INFERRED · 0% AMBIGUOUS · INFERRED: 51 edges (avg confidence: 0.83)
- Token cost: 0 input · 0 output

## Community Hubs (Navigation)
- Giggs Mesh Leaves
- Hyphae DHT Behaviour
- Layer Archive Stack
- Node Control Plane
- Membrane Listen Addrs
- Serde Trait Impls
- Isotope Atom Gossip
- Esporocarpo e Membrana
- Organism Lifecycle
- Diagnose Reachability
- Node Store Ions
- Pheromone Alarms
- Vacuum Chamber Cloud
- CLI Daemon Commands
- Chaos Entropy Keys
- Relay Mesh Ads
- Event Horizon Collapse
- Flywheel Build Dist
- Mailbox Store Forward
- WebRTC ICE Transport
- Lattice Spore Protocol
- CPE Probe Session
- Rhizomorph Biology
- Demo E2E Script
- Demo Horizon Script
- Demo Isotope Decay
- Demo Lattice Remote
- Demo Seedbook
- Cortex Medulla
- CPE Sensor Script
- Export Seed Script
- Install Seed Script
- Diagnose Script
- Probe Sporocarp Script
- Run Public Seed
- Verify Sporocarp Script
- Nutrient Cycling

## God Nodes (most connected - your core abstractions)
1. `ContentId` - 69 edges
2. `HyphaeNode` - 50 edges
3. `Organism` - 48 edges
4. `NodeId` - 44 edges
5. `NodeStore` - 28 edges
6. `VacuumError` - 28 edges
7. `OrganismError` - 27 edges
8. `HyphaeError` - 25 edges
9. `Membrane` - 24 edges
10. `RelayMesh` - 22 edges

## Surprising Connections (you probably didn't know these)
- `Esporocarpo voluntário zero VPS` --semantically_similar_to--> `Zero VPS Mesh`  [INFERRED] [semantically similar]
  docs/volunteer-sporocarp.md → README.md
- `Spore Bank` --semantically_similar_to--> `Spore Bank`  [INFERRED] [semantically similar]
  docs/glossary.md → README.md
- `daemon()` --calls--> `call()`  [INFERRED]
  cli/mycelium-cli/src/main.rs → crates/mycelium-node/src/control.rs
- `status()` --calls--> `call()`  [INFERRED]
  cli/mycelium-cli/src/main.rs → crates/mycelium-node/src/control.rs
- `rpc()` --calls--> `call()`  [INFERRED]
  cli/mycelium-cli/src/main.rs → crates/mycelium-node/src/control.rs

## Import Cycles
- 2-file cycle: `crates/vacuum/src/layers.rs -> crates/vacuum/src/lib.rs -> crates/vacuum/src/layers.rs`
- 2-file cycle: `crates/mycelium-hyphae/src/lib.rs -> crates/mycelium-hyphae/src/seeds.rs -> crates/mycelium-hyphae/src/lib.rs`
- 3-file cycle: `crates/vacuum/src/layers.rs -> crates/vacuum/src/lib.rs -> crates/vacuum/src/process.rs -> crates/vacuum/src/layers.rs`

## Hyperedges (group relationships)
- **Papéis de membrana (floresta/raiz/folha/esporocarp)** — docs_rizomorphs_floresta, docs_rizomorphs_raiz, docs_rizomorphs_folha, docs_rizomorphs_esporocarp [EXTRACTED 1.00]
- **Fluxo de verificação do esporocarpo voluntário** — readme_proof_json, readme_mycelium_reachable, docs_volunteer_sporocarp_gate, docs_invariante_membrana_invariante, docs_candidatos_criterio_verde [EXTRACTED 1.00]
- **Pilha de conectividade direct/circuit/mailbox** — docs_matriz_membranas_direct, docs_matriz_membranas_circuit, docs_matriz_membranas_mailbox_dtn, docs_live_vs_dtn_live_mesh, docs_live_vs_dtn_dtn_store_and_forward [INFERRED 0.85]

## Communities (37 total, 14 thin omitted)

### Community 0 - "Giggs Mesh Leaves"
Cohesion: 0.05
Nodes (62): GiggsError, Leaf, lineage_walks_history(), Mesh, Plot, plots_are_content_addressed(), replication_roundtrip(), Error (+54 more)

### Community 1 - "Hyphae DHT Behaviour"
Cohesion: 0.08
Nodes (38): Behaviour, addr_family_rank(), deterministic_seed_yields_deterministic_peer_id(), dht_local_store_roundtrip_via_put(), HyphaeConfig, HyphaeError, HyphaeNode, HyphaEvent (+30 more)

### Community 2 - "Layer Archive Stack"
Cohesion: 0.08
Nodes (45): Child, Command, archive_stacks_on_rootfs(), LayerArchive, LayerStore, AsRef, HashMap, Into (+37 more)

### Community 3 - "Node Control Plane"
Cohesion: 0.08
Nodes (53): Box, ConnectInfo, auth_ok_strips_field(), auth_required_rejects_missing(), call(), ControlMsg, exchange_line(), handle_client_lines() (+45 more)

### Community 4 - "Membrane Listen Addrs"
Cohesion: 0.10
Nodes (26): BTreeSet, Membrane, default_listen_addrs(), folha_listen_is_loopback_v4(), Multiaddr, Option, Vec, seed_dial_rank() (+18 more)

### Community 5 - "Serde Trait Impls"
Cohesion: 0.09
Nodes (22): Nutrient, Error, Result, S, Self, cannot_metabolize_more_than_balance(), Exchange, history_records_exchanges() (+14 more)

### Community 6 - "Isotope Atom Gossip"
Cohesion: 0.09
Nodes (27): absorb_accepts_foreign_shard_for_gossip(), Atom, for_node_is_stable(), IsotopeError, key_for_shard(), keys_route_to_their_natural_shard(), last_writer_wins_on_fuse(), migrate_preserves_atoms() (+19 more)

### Community 7 - "Esporocarpo e Membrana"
Cohesion: 0.06
Nodes (43): Organismo (daemon), Política de Membrana, Singularity Event Horizon, Sporocarp, Vacuum Chamber, Critério verde (esporocarpo), Candidatos a esporocarpo voluntário, Regras de engajamento (+35 more)

### Community 8 - "Organism Lifecycle"
Cohesion: 0.16
Nodes (16): Organism, OrganismConfig, OrganismError, HashMap, HashSet, HorizonTable, Option, Path (+8 more)

### Community 9 - "Diagnose Reachability"
Cohesion: 0.10
Nodes (21): detect_global_ipv6(), diagnose_membrane(), env_assume_reachable(), FruitingBody, Resources, Option, Vitality, Chamber (+13 more)

### Community 10 - "Node Store Ions"
Cohesion: 0.12
Nodes (14): default_horizon_port(), IonRecord, NodeStore, OrganismState, AsRef, Error, Option, Path (+6 more)

### Community 11 - "Pheromone Alarms"
Cohesion: 0.10
Nodes (23): Alarm, Contribution, evaporated_pheromone_is_rejected(), Gland, legacy_json_defaults_membrane_to_folha(), now_secs(), Pheromone, PheromoneBody (+15 more)

### Community 12 - "Vacuum Chamber Cloud"
Cohesion: 0.12
Nodes (18): cannot_inject_duplicate_ion(), chamber(), Charge, Cloud, cloud_recombines_negative_ions(), fruiting_body_contract(), Ion, ion_senses_demand_and_asks_for_replicas() (+10 more)

### Community 13 - "CLI Daemon Commands"
Cohesion: 0.16
Nodes (30): chamber_serve(), Cli, Commands, daemon(), deploy(), DeployOpts, isotope_get_poll(), main() (+22 more)

### Community 14 - "Chaos Entropy Keys"
Cohesion: 0.17
Nodes (19): ChaosKey, EntropyError, fewer_than_threshold_fails(), gf_add(), gf_inv(), gf_mul(), half_life_evaporates_secret(), lagrange_at_zero() (+11 more)

### Community 15 - "Relay Mesh Ads"
Cohesion: 0.13
Nodes (13): rejects_unreachable_ads(), RelayAdvertisement, RelayHealth, RelayMesh, RelayMeshConfig, HashMap, Instant, Multiaddr (+5 more)

### Community 16 - "Event Horizon Collapse"
Cohesion: 0.15
Nodes (16): collapsed_node_leaves_the_horizon(), EventHorizon, heaviest_orbit_wins(), Orbit, resistance_drags_gravity_down(), route_by_ion_name(), HashMap, HorizonTable (+8 more)

### Community 17 - "Flywheel Build Dist"
Cohesion: 0.19
Nodes (20): build_sh_produces_dist_artifact(), collect_artifact(), execute(), Flywheel, InertiaError, materialize_leaves(), Momentum, Error (+12 more)

### Community 18 - "Mailbox Store Forward"
Cohesion: 0.25
Nodes (13): ack_key(), is_expired(), mailbox_key(), mailbox_prefix(), MailboxAck, MailboxContentType, MailboxMessage, make_ack() (+5 more)

### Community 19 - "WebRTC ICE Transport"
Cohesion: 0.18
Nodes (11): build(), Default, Result, Self, String, Vec, webrtc_available(), webrtc_listen_addr() (+3 more)

### Community 20 - "Lattice Spore Protocol"
Cohesion: 0.29
Nodes (7): CI workspace (test + integration), Spore Bank, Envelope v:1, mycelium/lattice/v1, Fluxo Lattice, Mycelium Network, Spore Bank

### Community 21 - "CPE Probe Session"
Cohesion: 0.83
Nodes (3): cleanup(), probe_phone(), cpe-probe-session.sh script

### Community 22 - "Rhizomorph Biology"
Cohesion: 0.67
Nodes (3): Anastomose, Hifa (Hypha), Rhizomorph

## Knowledge Gaps
- **26 isolated node(s):** `cpe-sensor.sh script`, `e2e-demo.sh script`, `export-seed.sh script`, `horizon-demo.sh script`, `install-seed.sh script` (+21 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **14 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `Organism` connect `Organism Lifecycle` to `Giggs Mesh Leaves`, `Hyphae DHT Behaviour`, `Layer Archive Stack`, `Node Control Plane`, `Membrane Listen Addrs`, `Serde Trait Impls`, `Isotope Atom Gossip`, `Diagnose Reachability`, `Node Store Ions`, `Pheromone Alarms`, `Vacuum Chamber Cloud`, `Flywheel Build Dist`?**
  _High betweenness centrality (0.345) - this node is a cross-community bridge._
- **Why does `NodeId` connect `Giggs Mesh Leaves` to `Serde Trait Impls`, `Isotope Atom Gossip`, `Organism Lifecycle`, `Diagnose Reachability`, `Pheromone Alarms`, `Vacuum Chamber Cloud`, `Chaos Entropy Keys`, `Event Horizon Collapse`, `Flywheel Build Dist`?**
  _High betweenness centrality (0.180) - this node is a cross-community bridge._
- **Why does `ContentId` connect `Giggs Mesh Leaves` to `Layer Archive Stack`, `Serde Trait Impls`, `Isotope Atom Gossip`, `Organism Lifecycle`, `Diagnose Reachability`, `Flywheel Build Dist`?**
  _High betweenness centrality (0.167) - this node is a cross-community bridge._
- **What connects `cpe-sensor.sh script`, `e2e-demo.sh script`, `export-seed.sh script` to the rest of the system?**
  _26 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Giggs Mesh Leaves` be split into smaller, more focused modules?**
  _Cohesion score 0.05071119356833643 - nodes in this community are weakly interconnected._
- **Should `Hyphae DHT Behaviour` be split into smaller, more focused modules?**
  _Cohesion score 0.07758031442241968 - nodes in this community are weakly interconnected._
- **Should `Layer Archive Stack` be split into smaller, more focused modules?**
  _Cohesion score 0.07675675675675675 - nodes in this community are weakly interconnected._