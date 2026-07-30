# Graph Report - Mycelium Network  (2026-07-22)

## Corpus Check
- 91 files · ~55,691 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 1374 nodes · 3327 edges · 50 communities (33 shown, 17 thin omitted)
- Extraction: 98% EXTRACTED · 2% INFERRED · 0% AMBIGUOUS · INFERRED: 79 edges (avg confidence: 0.82)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `4ffc24f9`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

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
- Nostr + QEL + GhostID (Fases 1–3)
- run-folha.sh
- proxy.rs
- candidate_relay.rs
- QelShard
- BlockStore
- volunteer-pipeline.sh
- mycelium-pqc/src/lib.rs
- GhostId
- AdaptiveLandscape
- hybrid-demo.sh
- Result

## God Nodes (most connected - your core abstractions)
1. `ContentId` - 75 edges
2. `HyphaeNode` - 52 edges
3. `Organism` - 51 edges
4. `NodeId` - 44 edges
5. `NostrError` - 33 edges
6. `NodeStore` - 28 edges
7. `VacuumError` - 28 edges
8. `HyphaeError` - 27 edges
9. `OrganismError` - 27 edges
10. `Membrane` - 24 edges

## Surprising Connections (you probably didn't know these)
- `candidate_cmd()` --calls--> `candidate_sleep_secs()`  [INFERRED]
  cli/mycelium-cli/src/main.rs → crates/mycelium-nostr/src/candidate_relay.rs
- `Esporocarpo voluntário zero VPS` --semantically_similar_to--> `Zero VPS Mesh`  [INFERRED] [semantically similar]
  docs/volunteer-sporocarp.md → README.md
- `Spore Bank` --semantically_similar_to--> `Spore Bank`  [INFERRED] [semantically similar]
  docs/glossary.md → README.md
- `candidate_cmd()` --calls--> `run_candidate_round()`  [INFERRED]
  cli/mycelium-cli/src/main.rs → crates/mycelium-nostr/src/candidate_relay.rs
- `candidate_cmd()` --calls--> `run_listen_round()`  [INFERRED]
  cli/mycelium-cli/src/main.rs → crates/mycelium-nostr/src/candidate_relay.rs

## Import Cycles
- 2-file cycle: `crates/mycelium-tropical/src/hilbert.rs -> crates/mycelium-tropical/src/lib.rs -> crates/mycelium-tropical/src/hilbert.rs`
- 2-file cycle: `crates/mycelium-tropical/src/godunov.rs -> crates/mycelium-tropical/src/lib.rs -> crates/mycelium-tropical/src/godunov.rs`
- 2-file cycle: `crates/mycelium-nostr/src/lib.rs -> crates/mycelium-nostr/src/relay_pool.rs -> crates/mycelium-nostr/src/lib.rs`
- 2-file cycle: `crates/mycelium-nostr/src/lib.rs -> crates/mycelium-nostr/src/nip94.rs -> crates/mycelium-nostr/src/lib.rs`
- 2-file cycle: `crates/mycelium-nostr/src/lib.rs -> crates/mycelium-nostr/src/shard_event.rs -> crates/mycelium-nostr/src/lib.rs`
- 2-file cycle: `crates/mycelium-qel/src/lib.rs -> crates/mycelium-qel/src/topological.rs -> crates/mycelium-qel/src/lib.rs`
- 2-file cycle: `crates/vacuum/src/layers.rs -> crates/vacuum/src/lib.rs -> crates/vacuum/src/layers.rs`
- 2-file cycle: `crates/mycelium-hyphae/src/lib.rs -> crates/mycelium-hyphae/src/seeds.rs -> crates/mycelium-hyphae/src/lib.rs`
- 3-file cycle: `crates/mycelium-nostr/src/lib.rs -> crates/mycelium-nostr/src/shard_event.rs -> crates/mycelium-nostr/src/relay_pool.rs -> crates/mycelium-nostr/src/lib.rs`
- 3-file cycle: `crates/mycelium-nostr/src/lib.rs -> crates/mycelium-nostr/src/relay_pool.rs -> crates/mycelium-nostr/src/nip94.rs -> crates/mycelium-nostr/src/lib.rs`
- 3-file cycle: `crates/mycelium-nostr/src/lib.rs -> crates/mycelium-nostr/src/shard_event.rs -> crates/mycelium-nostr/src/nip94.rs -> crates/mycelium-nostr/src/lib.rs`
- 3-file cycle: `crates/vacuum/src/layers.rs -> crates/vacuum/src/lib.rs -> crates/vacuum/src/process.rs -> crates/vacuum/src/layers.rs`
- 4-file cycle: `crates/mycelium-nostr/src/lib.rs -> crates/mycelium-nostr/src/shard_event.rs -> crates/mycelium-nostr/src/relay_pool.rs -> crates/mycelium-nostr/src/nip94.rs -> crates/mycelium-nostr/src/lib.rs`

## Hyperedges (group relationships)
- **Papéis de membrana (floresta/raiz/folha/esporocarp)** — docs_rizomorphs_floresta, docs_rizomorphs_raiz, docs_rizomorphs_folha, docs_rizomorphs_esporocarp [EXTRACTED 1.00]
- **Fluxo de verificação do esporocarpo voluntário** — readme_proof_json, readme_mycelium_reachable, docs_volunteer_sporocarp_gate, docs_invariante_membrana_invariante, docs_candidatos_criterio_verde [EXTRACTED 1.00]
- **Pilha de conectividade direct/circuit/mailbox** — docs_matriz_membranas_direct, docs_matriz_membranas_circuit, docs_matriz_membranas_mailbox_dtn, docs_live_vs_dtn_live_mesh, docs_live_vs_dtn_dtn_store_and_forward [INFERRED 0.85]

## Communities (50 total, 17 thin omitted)

### Community 0 - "Giggs Mesh Leaves"
Cohesion: 0.06
Nodes (45): GiggsError, Leaf, lineage_walks_history(), Mesh, Plot, replication_roundtrip(), Error, HashMap (+37 more)

### Community 1 - "Hyphae DHT Behaviour"
Cohesion: 0.06
Nodes (53): Behaviour, addr_family_rank(), deterministic_seed_yields_deterministic_peer_id(), dht_local_store_roundtrip_via_put(), HyphaeConfig, HyphaeError, HyphaeNode, HyphaEvent (+45 more)

### Community 2 - "Layer Archive Stack"
Cohesion: 0.08
Nodes (45): Child, Command, archive_stacks_on_rootfs(), LayerArchive, LayerStore, AsRef, HashMap, Into (+37 more)

### Community 3 - "Node Control Plane"
Cohesion: 0.09
Nodes (68): Box, candidate_cmd(), CandidateCmd, chamber_serve(), Cli, Commands, daemon(), deploy() (+60 more)

### Community 4 - "Membrane Listen Addrs"
Cohesion: 0.15
Nodes (19): BTreeSet, accepts_dnsaddr(), accepts_mycelium_prefix_and_ipv6_sort(), membrane_flags_parse_and_filter(), parse_txt_blob(), parses_seed_file_ignoring_comments(), AsRef, Multiaddr (+11 more)

### Community 5 - "Serde Trait Impls"
Cohesion: 0.07
Nodes (46): plots_are_content_addressed(), build_sh_produces_dist_artifact(), collect_artifact(), execute(), Flywheel, InertiaError, materialize_leaves(), Momentum (+38 more)

### Community 6 - "Isotope Atom Gossip"
Cohesion: 0.09
Nodes (27): absorb_accepts_foreign_shard_for_gossip(), Atom, for_node_is_stable(), IsotopeError, key_for_shard(), keys_route_to_their_natural_shard(), last_writer_wins_on_fuse(), migrate_preserves_atoms() (+19 more)

### Community 7 - "Esporocarpo e Membrana"
Cohesion: 0.06
Nodes (43): Organismo (daemon), Política de Membrana, Singularity Event Horizon, Sporocarp, Vacuum Chamber, Critério verde (esporocarpo), Candidatos a esporocarpo voluntário, Regras de engajamento (+35 more)

### Community 8 - "Organism Lifecycle"
Cohesion: 0.06
Nodes (48): BTreeMap, Compat, accept_connection(), dial_connection(), duplex_pair(), NostrConnection, now_secs(), parse_incoming_frame() (+40 more)

### Community 9 - "Diagnose Reachability"
Cohesion: 0.11
Nodes (16): bad_signature_rejected(), entropy_collector_extracts_32_bytes(), EntropyCollector, GhostError, GhostId, now_secs(), Default, Drop (+8 more)

### Community 10 - "Node Store Ions"
Cohesion: 0.12
Nodes (14): default_horizon_port(), IonRecord, NodeStore, OrganismState, AsRef, Error, Option, Path (+6 more)

### Community 11 - "Pheromone Alarms"
Cohesion: 0.10
Nodes (23): Alarm, Contribution, evaporated_pheromone_is_rejected(), Gland, legacy_json_defaults_membrane_to_folha(), now_secs(), Pheromone, PheromoneBody (+15 more)

### Community 12 - "Vacuum Chamber Cloud"
Cohesion: 0.07
Nodes (33): Resources, Vitality, cannot_inject_duplicate_ion(), chamber(), Charge, Cloud, cloud_recombines_negative_ions(), fruiting_body_contract() (+25 more)

### Community 13 - "CLI Daemon Commands"
Cohesion: 0.10
Nodes (24): Organism, OrganismConfig, OrganismError, HashMap, HashSet, HorizonTable, Option, Path (+16 more)

### Community 14 - "Chaos Entropy Keys"
Cohesion: 0.17
Nodes (19): ChaosKey, EntropyError, fewer_than_threshold_fails(), gf_add(), gf_inv(), gf_mul(), half_life_evaporates_secret(), lagrange_at_zero() (+11 more)

### Community 15 - "Relay Mesh Ads"
Cohesion: 0.14
Nodes (13): rejects_unreachable_ads(), RelayAdvertisement, RelayHealth, RelayMesh, RelayMeshConfig, HashMap, Instant, Multiaddr (+5 more)

### Community 16 - "Event Horizon Collapse"
Cohesion: 0.15
Nodes (16): collapsed_node_leaves_the_horizon(), EventHorizon, heaviest_orbit_wins(), Orbit, resistance_drags_gravity_down(), route_by_ion_name(), HashMap, HorizonTable (+8 more)

### Community 17 - "Flywheel Build Dist"
Cohesion: 0.09
Nodes (25): bellman_prefers_strong_edge(), BellmanOperator, Self, Vec, weights_from_network(), cfl_clamped(), cfl_timestep(), CflConfig (+17 more)

### Community 18 - "Mailbox Store Forward"
Cohesion: 0.18
Nodes (15): Nutrient, cannot_metabolize_more_than_balance(), Exchange, history_records_exchanges(), Ledger, NutrientError, pledge_credits_all_currencies(), HashMap (+7 more)

### Community 19 - "WebRTC ICE Transport"
Cohesion: 0.18
Nodes (11): build(), Default, Keypair, Result, Self, String, Transport, Vec (+3 more)

### Community 20 - "Lattice Spore Protocol"
Cohesion: 0.29
Nodes (7): CI workspace (test + integration), Spore Bank, Envelope v:1, mycelium/lattice/v1, Fluxo Lattice, Mycelium Network, Spore Bank

### Community 21 - "CPE Probe Session"
Cohesion: 0.83
Nodes (3): cleanup(), probe_phone(), cpe-probe-session.sh script

### Community 22 - "Rhizomorph Biology"
Cohesion: 0.67
Nodes (3): Anastomose, Hifa (Hypha), Rhizomorph

### Community 37 - "Nostr + QEL + GhostID (Fases 1–3)"
Cohesion: 0.06
Nodes (29): Algoritmo (P0), CandidateRelay — terceiro estado (kind 39401), CLI, Kinds QEL (reserva), Problema, Relação com esporocarpo voluntário, Riscos, Roadmap (+21 more)

### Community 40 - "proxy.rs"
Cohesion: 0.11
Nodes (24): ConnectInfo, allow_ip(), console(), health(), HorizonHandle, proxy(), rate_gate(), rate_table() (+16 more)

### Community 41 - "candidate_relay.rs"
Cohesion: 0.07
Nodes (65): announcement_is_kind_39401(), backchannel_event_is_kind_39406(), BackchannelMessage, build_backchannel_event(), candidate_sleep_secs(), CandidatePeer, CandidateRelay, CandidateRoundReport (+57 more)

### Community 42 - "QelShard"
Cohesion: 0.12
Nodes (35): anderson_cage_channels(), fallback_order(), fallback_puts_preferred_first(), hybrid_hints_from_landscape(), hybrid_hints_split_mailbox_store(), internet_prefers_nostr(), Vec, select_transports() (+27 more)

### Community 43 - "BlockStore"
Cohesion: 0.09
Nodes (37): Boxed, AddrError, encode_nostr_multiaddr(), is_nostr_multiaddr(), parse_nostr_multiaddr(), roundtrip(), Multiaddr, Result (+29 more)

### Community 45 - "volunteer-pipeline.sh"
Cohesion: 0.33
Nodes (14): cmd_cgnat_check(), cmd_folha_attach(), cmd_mark(), cmd_onboard(), cmd_pitch(), cmd_prep_listen(), cmd_probe(), cmd_status() (+6 more)

### Community 46 - "mycelium-pqc/src/lib.rs"
Cohesion: 0.26
Nodes (11): KemEncap, KemKeyPair, mlkem_decapsulate(), mlkem_encapsulate(), mlkem_keygen(), mlkem_roundtrip(), PqcError, Drop (+3 more)

### Community 51 - "AdaptiveLandscape"
Cohesion: 0.43
Nodes (3): AdaptiveLandscape, Self, Vec

### Community 53 - "Result"
Cohesion: 0.07
Nodes (22): detect_global_ipv6(), diagnose_membrane(), env_assume_reachable(), FruitingBody, Membrane, ParseResourcesError, Error, Option (+14 more)

## Knowledge Gaps
- **52 isolated node(s):** `cpe-sensor.sh script`, `e2e-demo.sh script`, `export-seed.sh script`, `horizon-demo.sh script`, `hybrid-demo.sh script` (+47 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **17 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `Organism` connect `CLI Daemon Commands` to `Giggs Mesh Leaves`, `Hyphae DHT Behaviour`, `Layer Archive Stack`, `Membrane Listen Addrs`, `Serde Trait Impls`, `Isotope Atom Gossip`, `proxy.rs`, `Node Store Ions`, `Pheromone Alarms`, `Vacuum Chamber Cloud`, `Mailbox Store Forward`, `Result`?**
  _High betweenness centrality (0.272) - this node is a cross-community bridge._
- **Why does `run_candidate_round()` connect `candidate_relay.rs` to `Hyphae DHT Behaviour`, `Node Control Plane`?**
  _High betweenness centrality (0.176) - this node is a cross-community bridge._
- **Why does `HyphaeNode` connect `Hyphae DHT Behaviour` to `CLI Daemon Commands`, `Relay Mesh Ads`?**
  _High betweenness centrality (0.170) - this node is a cross-community bridge._
- **What connects `cpe-sensor.sh script`, `e2e-demo.sh script`, `export-seed.sh script` to the rest of the system?**
  _52 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Giggs Mesh Leaves` be split into smaller, more focused modules?**
  _Cohesion score 0.06491228070175438 - nodes in this community are weakly interconnected._
- **Should `Hyphae DHT Behaviour` be split into smaller, more focused modules?**
  _Cohesion score 0.05806858826004629 - nodes in this community are weakly interconnected._
- **Should `Layer Archive Stack` be split into smaller, more focused modules?**
  _Cohesion score 0.07675675675675675 - nodes in this community are weakly interconnected._