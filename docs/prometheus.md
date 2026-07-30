# Prometheus /metrics

## Endpoint

```
GET /metrics  →  text/plain; version=0.0.4
```

Disponível no Event Horizon (porta 7474 por default).

## Métricas exportadas

| Métrica | Tipo | Descrição |
|---------|------|-----------|
| `mycelium_neighbors` | gauge | Número de vizinhos conectados |
| `mycelium_plots` | gauge | Plots no Spore Bank |
| `mycelium_signals` | gauge | Signals no TheField |
| `mycelium_ions` | gauge | Ions em órbita no Plasma |
| `mycelium_atp` | gauge | Saldo de ATP |
| `mycelium_enzymes` | gauge | Saldo de Enzymes |
| `mycelium_mycelia` | gauge | Saldo de Mycelia |
| `mycelium_spores` | gauge | Saldo de Spores |
| `mycelium_resilience` | gauge | Saldo de Resilience |
| `mycelium_anastomoses` | counter | Total de conexões formadas |
| `mycelium_messages_in` | counter | Mensagens gossip recebidas |
| `mycelium_messages_out` | counter | Mensagens gossip enviadas |
| `mycelium_isotope_atoms` | gauge | Átomos no Nucleus |
| `mycelium_membrane{membrane="..."}` | gauge | Membrana atual (label) |
| `mycelium_physarum_phase{phase="..."}` | gauge | Fase Physarum (label) |

## Frequência

O snapshot é gerado a cada **30 segundos** pelo organismo e publicado no
`EventHorizon`, que o serve no endpoint `/metrics`.

## Exemplo de saída

```
# HELP mycelium_neighbors Número de vizinhos
# TYPE mycelium_neighbors gauge
mycelium_neighbors 3
# HELP mycelium_atp Saldo de ATP
# TYPE mycelium_atp gauge
mycelium_atp 42
# HELP mycelium_membrane Membrana atual
# TYPE mycelium_membrane gauge
mycelium_membrane{membrane="folha"} 1
```

## Integração com Prometheus

```yaml
scrape_configs:
  - job_name: 'mycelium'
    scrape_interval: 30s
    static_configs:
      - targets: ['127.0.0.1:7474']
        labels:
          group: 'substrato'
```

## Código

- **Snapshot:** `mycelium-node/src/organism.rs` → `metrics_tick`
- **Endpoint:** `singularity/src/proxy.rs` → rota `/metrics`
- **Armazenamento:** `singularity/src/lib.rs` → `EventHorizon.metrics`
