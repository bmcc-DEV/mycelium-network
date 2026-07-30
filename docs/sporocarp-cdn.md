# Sporocarp CDN

## Problema

Plots e layers são content-addressed (BLAKE3) mas não têm uma rota HTTP
para acesso público. Um `curl` não consegue baixar um Plot.

## Solução: rotas HTTP no Event Horizon

### Rotas

| Rota | Descrição |
|------|-----------|
| `GET /plots/{id}` | Retorna o Plot (JSON) do Spore Bank local |
| `GET /plots/{id}/leaves/{path}` | Retorna uma leaf específica do Plot |
| `GET /layers/{id}` | Retorna uma layer do LayerStore |
| `GET /plots/{id}/raw` | Retorna bytes crus do spore_print |

### Implementação

```rust
// Em singularity/src/proxy.rs
async fn serve_plot(
    Path(id): Path<String>,
    State(table): State<HorizonTable>,
) -> Response {
    let home = /* resolve do horizon */;
    let bank = SporeBank::open(home).map_err(...)?;
    let cid = ContentId::from_str(&id).map_err(...)?;
    match bank.recall(&cid) {
        Some(plot) => Json(plot).into_response(),
        None => (StatusCode::NOT_FOUND, "plot ausente").into_response(),
    }
}
```

### Discovery

- Esporocarps anunciam `/plots/{id}` no DHT sob chave `spore/<ContentId>`
- Cliente faz `dht_get` → descobre qual esporocarp tem o Plot → `curl /plots/{id}`

### Expansão futura (Growth Zones)

**Growth Zones** são regiões orgânicas do espaço de endereços content-addressed
que expandem/contraem conforme a densidade de nós que custodiam determinado
prefixo de ContentId.

```
zone: Qma → nós A, B, C  (3 custodiantes)
zone: Qmb → nós D, E     (2 custodiantes, pode contrair)
zone: Qmc → apenas nó F  (pode expandir para incluir G)
```

Cada nó participa da zona que corresponde ao prefixo mais próximo do seu
NodeId (distância XOR). Isso forma um **DHT overlay** por zona.

#### Implementação simplificada

1. `GrowthZone` struct: `prefix: String, custodians: Vec<NodeId>`
2. Anúncio periódico no gossip: `ZoneAnnounce { zone: "Qma", custodian: NodeId }`
3. Ao servir `/plots/{id}`, o nó verifica se está na zona do prefixo do ContentId
4. Se não estiver, redireciona 302 para um custodiante da zona

### Roteiro

1. Adicionar rotas `/plots/{id}` e `/layers/{id}` no proxy do Horizon
2. Integrar com SporeBank e LayerStore
3. Anunciar Plots no DHT com flag `http-reachable`
4. Growth Zones como fase 2
