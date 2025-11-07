use std::{collections::HashMap, convert::TryInto, sync::Arc};

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::{rngs::OsRng, CryptoRng, RngCore};
use sha2::{Digest, Sha512};
use thiserror::Error;

use crate::config::{ParachainConfig, ParachainKeyConfig};

/// Errors produced by the cryptography subsystem.
#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("parachain {para_id} is not registered")]
    UnknownParachain { para_id: u32 },
    #[error("invalid signature bytes: {0}")]
    InvalidSignature(String),
    #[error("failed to construct keypair for parachain {para_id}: {source}")]
    InvalidKey {
        para_id: u32,
        #[source]
        source: KeypairBuildError,
    },
}

/// Holder for generated or configured keypairs keyed by parachain id.
#[derive(Clone)]
pub struct KeyRegistry {
    inner: Arc<HashMap<u32, ParachainKeypair>>,
}

impl KeyRegistry {
    /// Build the registry from configuration, generating keys where none are provided.
    pub fn from_config(config: &ParachainConfig) -> Result<Self, CryptoError> {
        let mut map = HashMap::new();
        let mut rng = OsRng;

        for para_id in config.parachain_ids() {
            let key_config = config.keys.iter().find(|entry| entry.para_id == para_id);
            let pair = match key_config {
                Some(entry) => ParachainKeypair::from_config_entry(para_id, entry)
                    .map_err(|source| CryptoError::InvalidKey { para_id, source })?,
                None => ParachainKeypair::generate(para_id, &mut rng),
            };
            map.insert(para_id, pair);
        }

        Ok(Self {
            inner: Arc::new(map),
        })
    }

    /// Retrieve a keypair for the given parachain id.
    pub fn get(&self, para_id: u32) -> Option<&ParachainKeypair> {
        self.inner.get(&para_id)
    }

    /// Verify a signature for a message emitted by a parachain.
    pub fn verify_signature(
        &self,
        para_id: u32,
        message: &[u8],
        signature_bytes: &[u8],
    ) -> Result<(), CryptoError> {
        let pair = self
            .get(para_id)
            .ok_or(CryptoError::UnknownParachain { para_id })?;

        let signature = signature_from_bytes(signature_bytes)
            .map_err(|err| CryptoError::InvalidSignature(err.to_string()))?;

        pair.verifying_key()
            .verify(message, &signature)
            .map_err(|err| {
                CryptoError::InvalidSignature(format!("signature verification failed: {err}"))
            })
    }

    /// Sign a message with the parachain's key. Intended for tests.
    pub fn sign_message(&self, para_id: u32, message: &[u8]) -> Result<Signature, CryptoError> {
        let pair = self
            .get(para_id)
            .ok_or(CryptoError::UnknownParachain { para_id })?;
        Ok(pair.signing_key.sign(message))
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

/// Signing/verifying keypair for a parachain.
#[derive(Clone)]
pub struct ParachainKeypair {
    pub para_id: u32,
    signing_key: SigningKey,
}

impl ParachainKeypair {
    fn from_config_entry(
        para_id: u32,
        entry: &ParachainKeyConfig,
    ) -> Result<Self, KeypairBuildError> {
        if entry.secret_key.is_some() && entry.seed_phrase.is_some() {
            return Err(KeypairBuildError::ConflictingSources);
        }

        let signing_key = if let Some(secret) = &entry.secret_key {
            signing_from_secret(secret)?
        } else if let Some(seed) = &entry.seed_phrase {
            signing_from_seed_phrase(seed)?
        } else {
            return Err(KeypairBuildError::MissingSource);
        };

        Ok(Self {
            para_id,
            signing_key,
        })
    }

    fn generate<R>(para_id: u32, rng: &mut R) -> Self
    where
        R: RngCore + CryptoRng,
    {
        let mut secret = [0u8; 32];
        rng.fill_bytes(&mut secret);
        let signing_key = SigningKey::from_bytes(&secret);
        Self {
            para_id,
            signing_key,
        }
    }

    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    pub fn public_key_hex(&self) -> String {
        hex::encode(self.verifying_key().as_bytes())
    }
}

#[derive(Debug, Error)]
pub enum KeypairBuildError {
    #[error("secret key or seed phrase must be provided")]
    MissingSource,
    #[error("secret key and seed phrase are both provided; pick one source")]
    ConflictingSources,
    #[error("failed to parse secret key: {0}")]
    InvalidSecretKey(String),
    #[error("failed to derive key from seed phrase: {0}")]
    SeedPhrase(String),
    #[error("failed to construct signing key: {0}")]
    Signature(String),
}

fn signing_from_secret(secret: &str) -> Result<SigningKey, KeypairBuildError> {
    let decoded = decode_hex(secret).map_err(KeypairBuildError::InvalidSecretKey)?;
    match decoded.len() {
        32 => {
            let mut bytes = [0u8; 32];
            bytes.copy_from_slice(&decoded);
            Ok(SigningKey::from_bytes(&bytes))
        }
        64 => {
            let mut bytes = [0u8; 64];
            bytes.copy_from_slice(&decoded);
            SigningKey::from_keypair_bytes(&bytes)
                .map_err(|err| KeypairBuildError::Signature(err.to_string()))
        }
        other => Err(KeypairBuildError::InvalidSecretKey(format!(
            "expected 32 or 64 bytes, got {other}"
        ))),
    }
}

fn signing_from_seed_phrase(seed: &str) -> Result<SigningKey, KeypairBuildError> {
    if seed.trim().is_empty() {
        return Err(KeypairBuildError::SeedPhrase(
            "seed phrase cannot be empty".into(),
        ));
    }
    let digest = Sha512::digest(seed.as_bytes());
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&digest[..32]);
    Ok(SigningKey::from_bytes(&bytes))
}

fn decode_hex(input: &str) -> Result<Vec<u8>, String> {
    let normalized = input.trim().trim_start_matches("0x");
    hex::decode(normalized).map_err(|err| err.to_string())
}

fn signature_from_bytes(bytes: &[u8]) -> Result<Signature, String> {
    if bytes.len() != 64 {
        return Err(format!(
            "expected 64-byte signature, received {} bytes",
            bytes.len()
        ));
    }
    let arr: [u8; 64] = bytes
        .try_into()
        .map_err(|_| "invalid signature length".to_string())?;
    Ok(Signature::from_bytes(&arr))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_config_with_secret(secret: &str) -> ParachainConfig {
        ParachainConfig {
            count: 1,
            xcm_version: "V3".into(),
            keys: vec![ParachainKeyConfig {
                para_id: 1000,
                seed_phrase: None,
                secret_key: Some(secret.to_string()),
            }],
        }
    }

    #[test]
    fn generates_keys_when_not_provided() {
        let config = ParachainConfig {
            count: 2,
            xcm_version: "V3".into(),
            keys: Vec::new(),
        };
        let registry = KeyRegistry::from_config(&config).expect("registry");
        assert_eq!(registry.len(), 2);
    }

    #[test]
    fn loads_secret_key_from_hex() {
        let config = sample_config_with_secret(
            "d1a8f40f4f54a97756f0a3cbb8113de2a8e2b3ef85da24e9f6d6c9cbe6a3b0ab",
        );
        let registry = KeyRegistry::from_config(&config).expect("registry");
        let key = registry.get(1000).expect("key");
        assert_eq!(key.para_id, 1000);
    }

    #[test]
    fn derives_key_from_seed_phrase() {
        let config = ParachainConfig {
            count: 1,
            xcm_version: "V3".into(),
            keys: vec![ParachainKeyConfig {
                para_id: 1000,
                seed_phrase: Some("test seed phrase".into()),
                secret_key: None,
            }],
        };
        let registry = KeyRegistry::from_config(&config).expect("registry");
        assert!(registry.get(1000).is_some());
    }

    #[test]
    fn sign_and_verify_roundtrip() {
        let config = ParachainConfig {
            count: 1,
            xcm_version: "V3".into(),
            keys: Vec::new(),
        };
        let registry = KeyRegistry::from_config(&config).expect("registry");
        let message = b"hello world";
        let signature = registry.sign_message(1000, message).expect("signature");
        let signature_bytes = signature.to_bytes();

        assert!(registry
            .verify_signature(1000, message, &signature_bytes)
            .is_ok());
    }
}
