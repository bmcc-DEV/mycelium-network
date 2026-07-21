//! # mycelium-pqc
//!
//! ML-KEM-1024 (FIPS 203) nativo — port de ET-COSMIC `void_core/pqc.rs` sem WASM.
//! ML-DSA fica para quando a crate `ml-dsa` estabilizar no workspace (honestidade: sem stub falso).

use ml_kem::kem::{Decapsulate, Encapsulate};
use ml_kem::{EncodedSizeUser, KemCore, MlKem1024};
use rand_core::OsRng;
use zeroize::{Zeroize, ZeroizeOnDrop};

#[derive(Debug, thiserror::Error)]
pub enum PqcError {
    #[error("tamanho inválido: {0}")]
    BadLength(String),
    #[error("operação PQC falhou: {0}")]
    Crypto(String),
}

#[derive(ZeroizeOnDrop)]
pub struct KemKeyPair {
    pub public_key: Vec<u8>,
    dk_bytes: Vec<u8>,
}

impl KemKeyPair {
    pub fn private_bytes(&self) -> &[u8] {
        &self.dk_bytes
    }
}

pub struct KemEncap {
    pub ciphertext: Vec<u8>,
    pub shared_secret: Vec<u8>,
}

impl Drop for KemEncap {
    fn drop(&mut self) {
        self.shared_secret.zeroize();
    }
}

pub fn mlkem_keygen() -> KemKeyPair {
    let (dk, ek) = MlKem1024::generate(&mut OsRng);
    KemKeyPair {
        public_key: ek.as_bytes().to_vec(),
        dk_bytes: dk.as_bytes().to_vec(),
    }
}

pub fn mlkem_encapsulate(public_key: &[u8]) -> Result<KemEncap, PqcError> {
    type Ek = <MlKem1024 as KemCore>::EncapsulationKey;
    let ek_arr = public_key
        .try_into()
        .map_err(|_| PqcError::BadLength("ML-KEM ek".into()))?;
    let ek = Ek::from_bytes(ek_arr);
    let (ct, ss) = ek
        .encapsulate(&mut OsRng)
        .map_err(|_| PqcError::Crypto("encapsulate".into()))?;
    Ok(KemEncap {
        ciphertext: ct.as_slice().to_vec(),
        shared_secret: ss.as_slice().to_vec(),
    })
}

pub fn mlkem_decapsulate(private_key: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>, PqcError> {
    type Dk = <MlKem1024 as KemCore>::DecapsulationKey;
    let dk_arr = private_key
        .try_into()
        .map_err(|_| PqcError::BadLength("ML-KEM dk".into()))?;
    let dk = Dk::from_bytes(dk_arr);
    let ct = ml_kem::Ciphertext::<MlKem1024>::try_from(ciphertext)
        .map_err(|_| PqcError::BadLength("ML-KEM ct".into()))?;
    let ss = dk
        .decapsulate(&ct)
        .map_err(|_| PqcError::Crypto("decapsulate".into()))?;
    Ok(ss.as_slice().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mlkem_roundtrip() {
        let kp = mlkem_keygen();
        let enc = mlkem_encapsulate(&kp.public_key).expect("encap");
        let ss = mlkem_decapsulate(kp.private_bytes(), &enc.ciphertext).expect("decap");
        assert_eq!(ss, enc.shared_secret);
    }
}
