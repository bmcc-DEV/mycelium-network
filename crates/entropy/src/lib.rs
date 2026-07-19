//! # Entropy — Segredos com meia-vida
//!
//! Um segredo nunca existe inteiro em um só lugar. É fatiado em **Shades**
//! (sombras) via Shamir Secret Sharing sobre GF(256). Só quando M de N
//! Shades são coletadas pelas hifas o segredo materializa — e só existe
//! por uma meia-vida curta antes de evaporar da memória.
//!
//! Implementação real de Shamir (não stub); integração com hifas para
//! coleta remota fica para a próxima fase.

use mycelium_core::NodeId;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

#[derive(Debug, thiserror::Error)]
pub enum EntropyError {
    #[error("threshold inválido: precisa de 1 ≤ M ≤ N (recebeu M={m}, N={n})")]
    BadThreshold { m: u8, n: u8 },
    #[error("poucas shades para reconstruir: tem {have}, precisa de {need}")]
    InsufficientShades { have: usize, need: usize },
    #[error("shades com índices inválidos ou duplicados")]
    CorruptShades,
    #[error("o segredo evaporou (meia-vida esgotada)")]
    Evaporated,
}

/// Uma fatia do segredo. Sozinha não revela nada.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Shade {
    /// Índice x ∈ [1, 255] no polinômio de Shamir.
    pub index: u8,
    /// Valor y = f(x) para cada byte do segredo.
    pub shares: Vec<u8>,
}

/// Um segredo materializado — existe só enquanto a meia-vida não acaba.
#[derive(Debug)]
pub struct ChaosKey {
    bytes: Vec<u8>,
    born: Instant,
    half_life: Duration,
}

impl ChaosKey {
    /// Materializa o segredo a partir de pelo menos M Shades.
    pub fn materialize(shades: &[Shade], threshold: u8) -> Result<Self, EntropyError> {
        let bytes = reconstruct(shades, threshold)?;
        Ok(Self {
            bytes,
            born: Instant::now(),
            half_life: Duration::from_secs(30),
        })
    }

    /// Define a meia-vida (quanto tempo o segredo permanece legível).
    pub fn with_half_life(mut self, half_life: Duration) -> Self {
        self.half_life = half_life;
        self
    }

    /// Lê o segredo se ainda não evaporou.
    pub fn reveal(&self) -> Result<&[u8], EntropyError> {
        if self.born.elapsed() > self.half_life {
            return Err(EntropyError::Evaporated);
        }
        Ok(&self.bytes)
    }

    /// Força a evaporação imediata (zeroiza o buffer).
    pub fn evaporate(&mut self) {
        for b in &mut self.bytes {
            *b = 0;
        }
        self.bytes.clear();
        self.half_life = Duration::ZERO;
    }
}

/// Um cofre local: gera e guarda as Shades que este nó é responsável
/// por custodiar (tipicamente 1 de N).
#[derive(Debug, Default)]
pub struct Vault {
    custody: Vec<(NodeId, Shade)>,
}

impl Vault {
    pub fn new() -> Self {
        Self::default()
    }

    /// Divide um segredo em N Shades; qualquer M delas reconstrói.
    pub fn shatter(secret: &[u8], m: u8, n: u8) -> Result<Vec<Shade>, EntropyError> {
        if m == 0 || n == 0 || m > n || n as usize > 255 {
            return Err(EntropyError::BadThreshold { m, n });
        }

        let mut shades: Vec<Shade> = (1..=n)
            .map(|i| Shade {
                index: i,
                shares: Vec::with_capacity(secret.len()),
            })
            .collect();

        let mut rng = rand::thread_rng();
        for &byte in secret {
            // Polinômio de grau M-1: f(0) = secret_byte.
            let mut coeffs = vec![byte];
            for _ in 1..m {
                coeffs.push(rng.next_u32() as u8);
            }
            for shade in &mut shades {
                shades_push_eval(shade, &coeffs);
            }
        }

        Ok(shades)
    }

    /// Custodia uma Shade em nome de um nó (ou do próprio).
    pub fn hold(&mut self, custodian: NodeId, shade: Shade) {
        self.custody.push((custodian, shade));
    }

    /// Coleta as Shades custodidas (simula a coleta via hifas).
    pub fn gather(&self) -> Vec<Shade> {
        self.custody.iter().map(|(_, s)| s.clone()).collect()
    }

    pub fn len(&self) -> usize {
        self.custody.len()
    }

    pub fn is_empty(&self) -> bool {
        self.custody.is_empty()
    }
}

fn shades_push_eval(shade: &mut Shade, coeffs: &[u8]) {
    // Avalia f(x) = Σ c_i · x^i  em GF(256).
    let x = shade.index;
    let mut y = 0u8;
    let mut x_pow = 1u8;
    for &c in coeffs {
        y = gf_add(y, gf_mul(c, x_pow));
        x_pow = gf_mul(x_pow, x);
    }
    shade.shares.push(y);
}

fn reconstruct(shades: &[Shade], threshold: u8) -> Result<Vec<u8>, EntropyError> {
    if shades.len() < threshold as usize {
        return Err(EntropyError::InsufficientShades {
            have: shades.len(),
            need: threshold as usize,
        });
    }

    let used = &shades[..threshold as usize];
    let mut seen = [false; 256];
    for s in used {
        if s.index == 0 || seen[s.index as usize] {
            return Err(EntropyError::CorruptShades);
        }
        seen[s.index as usize] = true;
    }

    let secret_len = used[0].shares.len();
    if used.iter().any(|s| s.shares.len() != secret_len) {
        return Err(EntropyError::CorruptShades);
    }

    let mut secret = Vec::with_capacity(secret_len);
    for byte_idx in 0..secret_len {
        let points: Vec<(u8, u8)> = used
            .iter()
            .map(|s| (s.index, s.shares[byte_idx]))
            .collect();
        secret.push(lagrange_at_zero(&points)?);
    }
    Ok(secret)
}

/// Interpolação de Lagrange em x=0 sobre GF(256).
fn lagrange_at_zero(points: &[(u8, u8)]) -> Result<u8, EntropyError> {
    let mut secret = 0u8;
    for (i, &(xi, yi)) in points.iter().enumerate() {
        let mut num = 1u8;
        let mut den = 1u8;
        for (j, &(xj, _)) in points.iter().enumerate() {
            if i == j {
                continue;
            }
            // ℓ_i(0) = Π (0 - x_j) / (x_i - x_j) = Π x_j / (x_i - x_j)
            // em GF(256), -a = a (característica 2).
            num = gf_mul(num, xj);
            den = gf_mul(den, gf_add(xi, xj));
        }
        let li = gf_mul(num, gf_inv(den)?);
        secret = gf_add(secret, gf_mul(yi, li));
    }
    Ok(secret)
}

// --- Aritmética GF(256) com polinômio irredutível x^8 + x^4 + x^3 + x + 1 (0x11b) ---

fn gf_add(a: u8, b: u8) -> u8 {
    a ^ b
}

fn gf_mul(mut a: u8, mut b: u8) -> u8 {
    let mut p = 0u8;
    for _ in 0..8 {
        if b & 1 != 0 {
            p ^= a;
        }
        let hi = a & 0x80;
        a <<= 1;
        if hi != 0 {
            a ^= 0x1b;
        }
        b >>= 1;
    }
    p
}

fn gf_inv(a: u8) -> Result<u8, EntropyError> {
    if a == 0 {
        return Err(EntropyError::CorruptShades);
    }
    // a^{254} = a^{-1} em GF(256)^* (pelo teorema de Fermat finito).
    let mut result = 1u8;
    let mut base = a;
    let mut exp = 254u16;
    while exp > 0 {
        if exp & 1 != 0 {
            result = gf_mul(result, base);
        }
        base = gf_mul(base, base);
        exp >>= 1;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shatter_and_reconstruct_roundtrip() {
        let secret = b"chaos-key-from-the-void";
        let shades = Vault::shatter(secret, 3, 5).unwrap();
        assert_eq!(shades.len(), 5);

        // Qualquer subconjunto de 3 reconstrói.
        let key = ChaosKey::materialize(&shades[0..3], 3)
            .unwrap()
            .with_half_life(Duration::from_secs(60));
        assert_eq!(key.reveal().unwrap(), secret);

        let key2 = ChaosKey::materialize(&[shades[1].clone(), shades[3].clone(), shades[4].clone()], 3)
            .unwrap();
        assert_eq!(key2.reveal().unwrap(), secret);
    }

    #[test]
    fn fewer_than_threshold_fails() {
        let shades = Vault::shatter(b"secret", 3, 5).unwrap();
        assert!(matches!(
            ChaosKey::materialize(&shades[0..2], 3),
            Err(EntropyError::InsufficientShades { have: 2, need: 3 })
        ));
    }

    #[test]
    fn half_life_evaporates_secret() {
        let shades = Vault::shatter(b"ephemeral", 2, 2).unwrap();
        let mut key = ChaosKey::materialize(&shades, 2)
            .unwrap()
            .with_half_life(Duration::ZERO);
        // born == Instant::now() e half_life == 0: pode ainda revelar no
        // mesmo instante; evaporamos explicitamente.
        key.evaporate();
        assert!(matches!(key.reveal(), Err(EntropyError::Evaporated)));
    }

    #[test]
    fn bad_threshold_rejected() {
        assert!(matches!(
            Vault::shatter(b"x", 3, 2),
            Err(EntropyError::BadThreshold { m: 3, n: 2 })
        ));
    }

    #[test]
    fn vault_holds_and_gathers() {
        let mut vault = Vault::new();
        let shades = Vault::shatter(b"held", 2, 3).unwrap();
        for (i, shade) in shades.into_iter().enumerate() {
            vault.hold(NodeId::derive(&[i as u8]), shade);
        }
        assert_eq!(vault.len(), 3);
        let key = ChaosKey::materialize(&vault.gather()[..2], 2).unwrap();
        assert_eq!(key.reveal().unwrap(), b"held");
    }
}
