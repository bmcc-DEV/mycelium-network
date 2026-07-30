# PQC — Pós-Quantum Cryptography

## ML-KEM-1024 (FIPS 203)

Implementação puramente em Rust (port do ET-COSMIC `void_core/pqc.rs` sem WASM).

**Crate:** `mycelium-pqc` em `crates/mycelium-pqc/`

```rust
pub fn mlkem_keygen() -> KemKeyPair
pub fn mlkem_encapsulate(public_key: &[u8]) -> Result<KemEncap, PqcError>
pub fn mlkem_decapsulate(private_key: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>, PqcError>
```

- Chave pública: 1568 bytes
- Chave privada: 1568 bytes
- Ciphertext: 1568 bytes
- Shared secret: 32 bytes

## Handshake híbrido (🟡 existente)

**Módulo:** `mycelium-hyphae/src/pqc.rs` (feature `pqc-transport`)

Após o handshake Noise padrão, peers trocam chaves ML-KEM sobre o
canal seguro e derivam um **segredo combinado**:

```
combined = blake3(noise_secret || pqc_shared_secret)
```

```rust
// Cliente: encapsula para a chave pública do servidor
let (hybrid, ciphertext) = pqc::client_handshake(&server_pk, &noise_secret)?;

// Servidor: decapsula o ciphertext do cliente
let hybrid = pqc::server_handshake(&private_key, &ciphertext, &noise_secret)?;

assert_eq!(hybrid.combined, client_hybrid.combined);
```

### Multiaddr

`/unix/mycelium-pqc/<pk_hex>` — mesmo padrão do Nostr transport.

### PeerId

Derivado deterministicamente da chave pública ML-KEM via
ed25519 seed (BLAKE3 → seed → Keypair → PeerId).

### Testes

```
cargo test --features pqc-transport -p mycelium-hyphae
```

- `hybrid_handshake_roundtrip` — ambos os lados chegam no mesmo combined
- `different_noise_gives_different_combined` — noise_secret diferente → combined diferente
- `pqc_multiaddr_roundtrip` — encode/decode de multiaddr

## Transporte real (❌ falta)

O que **não** foi implementado ainda:

1. **Implementar `Transport` trait completo** (igual `NostrTransport`)
   - TCP listen/dial
   - Handshake KEM (cliente encapsula, servidor decapsula)
   - Yamux multiplexing
   - Retornar `(PeerId, StreamMuxerBox)`
2. **Registrar com `with_other_transport`** na germinação das hifas
3. **Rota de dial/listen funcional**

```rust
// Planejado:
let addr = "/tcp/4004/pqc/<pk_hex>";
builder
    .with_other_transport(|key| pqc::build(key))
    .map_err(|e| ...)?;
```

## Próximos passos

1. Completar `PqcTransport` struct com `Transport` trait
2. Adicionar `build()` function que retorna `Boxed<(PeerId, StreamMuxerBox)>`
3. Wire no `germinate_with()` do `mycelium-hyphae`
4. Teste de integração: dois nós se conectam via PQC-only
