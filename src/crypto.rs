use crate::hash::Hash32;
use k256::{
    FieldBytes,
    schnorr::{
        Signature as SchnorrSignature, SigningKey as SchnorrSigningKey,
        VerifyingKey as SchnorrVerifyingKey,
        signature::{Signer as _, Verifier as _},
    },
};
use ml_dsa::{
    EncodedVerifyingKey, Generate as _, Keypair as _, MlDsa65, Signature as MlDsaSignature,
    SigningKey as MlDsaSigningKey, VerifyingKey as MlDsaVerifyingKey,
};
use serde::{Deserialize, Serialize};
use slh_dsa::{
    Shake128s, Signature as SlhDsaSignature, SigningKey as SlhDsaSigningKey,
    VerifyingKey as SlhDsaVerifyingKey,
};
use thiserror::Error;
use zeroize::{Zeroize, ZeroizeOnDrop};

pub const SCHNORR_PUBLIC_KEY_BYTES: usize = 32;
pub const SCHNORR_SIGNATURE_BYTES: usize = 64;
pub const ML_DSA_65_PUBLIC_KEY_BYTES: usize = 1_952;
pub const ML_DSA_65_SIGNATURE_BYTES: usize = 3_309;
pub const SLH_DSA_SHAKE_128S_PUBLIC_KEY_BYTES: usize = 32;
pub const SLH_DSA_SHAKE_128S_SIGNATURE_BYTES: usize = 7_856;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(u16)]
pub enum SignatureAlgorithm {
    SchnorrBip340V1 = 1,
    MlDsa65Fips204V1 = 2,
    SlhDsaShake128sFips205V1 = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u16)]
pub enum SpendPolicyVersion {
    HybridSchnorrMlDsaV1 = 0x0100,
    SlhDsaRecoveryV1 = 0x0101,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DomainTag {
    TransactionSignature,
    BlockSignature,
    CheckpointSignature,
    WalletRecovery,
    NetworkHandshake,
    GovernanceMessage,
}

impl DomainTag {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TransactionSignature => "QBTC/TX-SIGNATURE/v1",
            Self::BlockSignature => "QBTC/BLOCK-SIGNATURE/v1",
            Self::CheckpointSignature => "QBTC/CHECKPOINT-SIGNATURE/v1",
            Self::WalletRecovery => "QBTC/WALLET-RECOVERY/v1",
            Self::NetworkHandshake => "QBTC/NETWORK-HANDSHAKE/v1",
            Self::GovernanceMessage => "QBTC/GOVERNANCE-MESSAGE/v1",
        }
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CryptoError {
    #[error("unsupported signature algorithm or policy version")]
    UnsupportedVersion,
    #[error("missing Schnorr signature required by hybrid AND policy")]
    MissingSchnorrSignature,
    #[error("missing ML-DSA signature required by hybrid AND policy")]
    MissingMlDsaSignature,
    #[error("invalid public key encoding")]
    InvalidPublicKey,
    #[error("invalid signature encoding")]
    InvalidSignatureEncoding,
    #[error("signature verification failed")]
    VerificationFailed,
    #[error("recovery relative timelock has not matured")]
    RecoveryTimelock,
    #[error("signature generation failed")]
    SigningFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HybridPublicKeys {
    pub policy_version: SpendPolicyVersion,
    #[serde(with = "hex_bytes")]
    pub schnorr: Vec<u8>,
    #[serde(with = "hex_bytes")]
    pub ml_dsa: Vec<u8>,
    pub recovery: Option<RecoveryPublicKey>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryPublicKey {
    pub policy_version: SpendPolicyVersion,
    pub relative_timelock: u32,
    #[serde(with = "hex_bytes")]
    pub slh_dsa: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HybridSignature {
    pub policy_version: SpendPolicyVersion,
    #[serde(with = "optional_hex_bytes")]
    pub schnorr: Option<Vec<u8>>,
    #[serde(with = "optional_hex_bytes")]
    pub ml_dsa: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoverySignature {
    pub policy_version: SpendPolicyVersion,
    #[serde(with = "hex_bytes")]
    pub slh_dsa: Vec<u8>,
}

/// Narrow verification boundary around third-party cryptographic libraries.
pub trait SignatureProvider: Send + Sync {
    fn verify(
        &self,
        algorithm: SignatureAlgorithm,
        public_key: &[u8],
        signature: &[u8],
        digest: &Hash32,
        domain: DomainTag,
    ) -> Result<(), CryptoError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct RustCryptoProvider;

impl SignatureProvider for RustCryptoProvider {
    fn verify(
        &self,
        algorithm: SignatureAlgorithm,
        public_key: &[u8],
        signature: &[u8],
        digest: &Hash32,
        domain: DomainTag,
    ) -> Result<(), CryptoError> {
        let context = domain.as_str().as_bytes();
        match algorithm {
            SignatureAlgorithm::SchnorrBip340V1 => {
                let key_bytes: [u8; SCHNORR_PUBLIC_KEY_BYTES] = public_key
                    .try_into()
                    .map_err(|_| CryptoError::InvalidPublicKey)?;
                let key = SchnorrVerifyingKey::from_bytes(&FieldBytes::from(key_bytes))
                    .map_err(|_| CryptoError::InvalidPublicKey)?;
                let signature = SchnorrSignature::try_from(signature)
                    .map_err(|_| CryptoError::InvalidSignatureEncoding)?;
                key.verify(digest.0.as_slice(), &signature)
                    .map_err(|_| CryptoError::VerificationFailed)
            }
            SignatureAlgorithm::MlDsa65Fips204V1 => {
                let encoded = EncodedVerifyingKey::<MlDsa65>::try_from(public_key)
                    .map_err(|_| CryptoError::InvalidPublicKey)?;
                let key = MlDsaVerifyingKey::<MlDsa65>::decode(&encoded);
                let signature = MlDsaSignature::<MlDsa65>::try_from(signature)
                    .map_err(|_| CryptoError::InvalidSignatureEncoding)?;
                key.verify_with_context(digest.0.as_slice(), context, &signature)
                    .then_some(())
                    .ok_or(CryptoError::VerificationFailed)
            }
            SignatureAlgorithm::SlhDsaShake128sFips205V1 => {
                let key = SlhDsaVerifyingKey::<Shake128s>::try_from(public_key)
                    .map_err(|_| CryptoError::InvalidPublicKey)?;
                let signature = SlhDsaSignature::<Shake128s>::try_from(signature)
                    .map_err(|_| CryptoError::InvalidSignatureEncoding)?;
                key.try_verify_with_context(digest.0.as_slice(), context, &signature)
                    .map_err(|_| CryptoError::VerificationFailed)
            }
        }
    }
}

/// ML-DSA-only key material used by checkpoint signers. Like wallet key
/// material, it cannot be formatted or serialized accidentally.
pub struct MlDsaKeyPair {
    signing_key: MlDsaSigningKey<MlDsa65>,
}

impl MlDsaKeyPair {
    #[must_use]
    pub fn from_seed(seed: [u8; 32]) -> Self {
        Self {
            signing_key: MlDsaSigningKey::<MlDsa65>::from_seed(&seed.into()),
        }
    }

    #[must_use]
    pub fn generate() -> Self {
        Self {
            signing_key: MlDsaSigningKey::<MlDsa65>::generate(),
        }
    }

    #[must_use]
    pub fn public_key(&self) -> Vec<u8> {
        self.signing_key.verifying_key().encode().to_vec()
    }

    pub fn sign_digest(&self, digest: &Hash32, domain: DomainTag) -> Result<Vec<u8>, CryptoError> {
        self.signing_key
            .expanded_key()
            .sign_deterministic(digest.0.as_slice(), domain.as_str().as_bytes())
            .map(|signature| signature.encode().to_vec())
            .map_err(|_| CryptoError::SigningFailed)
    }
}

/// In-memory hybrid signing material. It intentionally implements neither
/// `Debug` nor serialization so secrets cannot be logged accidentally.
pub struct HybridKeyPair {
    schnorr: SchnorrSigningKey,
    ml_dsa: MlDsaSigningKey<MlDsa65>,
    recovery: Option<RecoveryKeyPair>,
}

#[derive(Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct HybridSecretMaterial {
    schnorr: [u8; 32],
    ml_dsa: [u8; 32],
    slh_dsa: Option<Vec<u8>>,
    recovery_relative_timelock: Option<u32>,
}

struct RecoveryKeyPair {
    slh_dsa: SlhDsaSigningKey<Shake128s>,
    relative_timelock: u32,
}

impl HybridKeyPair {
    #[must_use]
    pub fn generate(recovery_relative_timelock: Option<u32>) -> Self {
        let recovery = recovery_relative_timelock.map(|relative_timelock| {
            let mut rng = rand::rng();
            RecoveryKeyPair {
                slh_dsa: SlhDsaSigningKey::<Shake128s>::new(&mut rng),
                relative_timelock,
            }
        });

        Self {
            schnorr: SchnorrSigningKey::generate(),
            ml_dsa: MlDsaSigningKey::<MlDsa65>::generate(),
            recovery,
        }
    }

    pub fn from_secret_material(material: &HybridSecretMaterial) -> Result<Self, CryptoError> {
        let schnorr = SchnorrSigningKey::from_bytes(&FieldBytes::from(material.schnorr))
            .map_err(|_| CryptoError::InvalidPublicKey)?;
        let ml_dsa = MlDsaSigningKey::<MlDsa65>::from_seed(&material.ml_dsa.into());
        let recovery = match (
            material.slh_dsa.as_deref(),
            material.recovery_relative_timelock,
        ) {
            (Some(bytes), Some(relative_timelock)) => Some(RecoveryKeyPair {
                slh_dsa: SlhDsaSigningKey::<Shake128s>::try_from(bytes)
                    .map_err(|_| CryptoError::InvalidPublicKey)?,
                relative_timelock,
            }),
            (None, None) => None,
            _ => return Err(CryptoError::InvalidPublicKey),
        };
        Ok(Self {
            schnorr,
            ml_dsa,
            recovery,
        })
    }

    /// Explicit export used only immediately before encrypted wallet storage.
    /// The returned value zeroises itself on drop.
    #[must_use]
    pub fn export_secret_material(&self) -> HybridSecretMaterial {
        HybridSecretMaterial {
            schnorr: self.schnorr.to_bytes().into(),
            ml_dsa: self.ml_dsa.to_seed().into(),
            slh_dsa: self
                .recovery
                .as_ref()
                .map(|recovery| recovery.slh_dsa.to_vec()),
            recovery_relative_timelock: self
                .recovery
                .as_ref()
                .map(|recovery| recovery.relative_timelock),
        }
    }

    #[must_use]
    pub fn public_keys(&self) -> HybridPublicKeys {
        HybridPublicKeys {
            policy_version: SpendPolicyVersion::HybridSchnorrMlDsaV1,
            schnorr: self.schnorr.verifying_key().to_bytes().to_vec(),
            ml_dsa: self.ml_dsa.verifying_key().encode().to_vec(),
            recovery: self.recovery.as_ref().map(|recovery| RecoveryPublicKey {
                policy_version: SpendPolicyVersion::SlhDsaRecoveryV1,
                relative_timelock: recovery.relative_timelock,
                slh_dsa: recovery.slh_dsa.verifying_key().to_vec(),
            }),
        }
    }

    pub fn sign_hybrid(
        &self,
        chain_id: &Hash32,
        domain: DomainTag,
        message: &[u8],
    ) -> Result<HybridSignature, CryptoError> {
        let digest = signature_digest(chain_id, domain, message);
        let schnorr: SchnorrSignature = self.schnorr.sign(digest.0.as_slice());
        let ml_dsa = self
            .ml_dsa
            .expanded_key()
            .sign_deterministic(digest.0.as_slice(), domain.as_str().as_bytes())
            .map_err(|_| CryptoError::SigningFailed)?;

        Ok(HybridSignature {
            policy_version: SpendPolicyVersion::HybridSchnorrMlDsaV1,
            schnorr: Some(schnorr.to_bytes().to_vec()),
            ml_dsa: Some(ml_dsa.encode().to_vec()),
        })
    }

    pub fn sign_recovery(
        &self,
        chain_id: &Hash32,
        message: &[u8],
    ) -> Result<RecoverySignature, CryptoError> {
        let recovery = self
            .recovery
            .as_ref()
            .ok_or(CryptoError::UnsupportedVersion)?;
        let domain = DomainTag::WalletRecovery;
        let digest = signature_digest(chain_id, domain, message);
        let signature = recovery
            .slh_dsa
            .try_sign_with_context(digest.0.as_slice(), domain.as_str().as_bytes(), None)
            .map_err(|_| CryptoError::SigningFailed)?;
        Ok(RecoverySignature {
            policy_version: SpendPolicyVersion::SlhDsaRecoveryV1,
            slh_dsa: signature.to_vec(),
        })
    }
}

#[must_use]
pub fn signature_digest(chain_id: &Hash32, domain: DomainTag, message: &[u8]) -> Hash32 {
    let mut payload = Vec::with_capacity(chain_id.0.len() + message.len());
    payload.extend_from_slice(&chain_id.0);
    payload.extend_from_slice(message);
    Hash32::tagged_sha3(domain.as_str(), &payload)
}

pub fn verify_hybrid(
    provider: &dyn SignatureProvider,
    chain_id: &Hash32,
    domain: DomainTag,
    message: &[u8],
    public_keys: &HybridPublicKeys,
    signature: &HybridSignature,
) -> Result<(), CryptoError> {
    if public_keys.policy_version != SpendPolicyVersion::HybridSchnorrMlDsaV1
        || signature.policy_version != SpendPolicyVersion::HybridSchnorrMlDsaV1
    {
        return Err(CryptoError::UnsupportedVersion);
    }
    let schnorr = signature
        .schnorr
        .as_deref()
        .ok_or(CryptoError::MissingSchnorrSignature)?;
    let ml_dsa = signature
        .ml_dsa
        .as_deref()
        .ok_or(CryptoError::MissingMlDsaSignature)?;
    let digest = signature_digest(chain_id, domain, message);

    provider.verify(
        SignatureAlgorithm::SchnorrBip340V1,
        &public_keys.schnorr,
        schnorr,
        &digest,
        domain,
    )?;
    provider.verify(
        SignatureAlgorithm::MlDsa65Fips204V1,
        &public_keys.ml_dsa,
        ml_dsa,
        &digest,
        domain,
    )
}

pub fn verify_recovery(
    provider: &dyn SignatureProvider,
    chain_id: &Hash32,
    message: &[u8],
    public_key: &RecoveryPublicKey,
    signature: &RecoverySignature,
    input_age_blocks: u64,
) -> Result<(), CryptoError> {
    if public_key.policy_version != SpendPolicyVersion::SlhDsaRecoveryV1
        || signature.policy_version != SpendPolicyVersion::SlhDsaRecoveryV1
    {
        return Err(CryptoError::UnsupportedVersion);
    }
    if input_age_blocks < u64::from(public_key.relative_timelock) {
        return Err(CryptoError::RecoveryTimelock);
    }
    let domain = DomainTag::WalletRecovery;
    let digest = signature_digest(chain_id, domain, message);
    provider.verify(
        SignatureAlgorithm::SlhDsaShake128sFips205V1,
        &public_key.slh_dsa,
        &signature.slh_dsa,
        &digest,
        domain,
    )
}

mod hex_bytes {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&hex::encode(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
        let encoded = String::deserialize(deserializer)?;
        hex::decode(encoded).map_err(serde::de::Error::custom)
    }
}

mod optional_hex_bytes {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(
        bytes: &Option<Vec<u8>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match bytes {
            Some(bytes) => serializer.serialize_some(&hex::encode(bytes)),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<Vec<u8>>, D::Error> {
        let encoded = Option::<String>::deserialize(deserializer)?;
        encoded
            .map(hex::decode)
            .transpose()
            .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::{ChainParams, Network};

    #[test]
    fn normal_spend_requires_both_signatures() {
        let params = ChainParams::for_network(Network::Regtest);
        let keys = HybridKeyPair::generate(None);
        let public = keys.public_keys();
        let signature = keys
            .sign_hybrid(&params.chain_id, DomainTag::TransactionSignature, b"spend")
            .unwrap();
        let provider = RustCryptoProvider;

        verify_hybrid(
            &provider,
            &params.chain_id,
            DomainTag::TransactionSignature,
            b"spend",
            &public,
            &signature,
        )
        .unwrap();

        let mut missing_ml_dsa = signature.clone();
        missing_ml_dsa.ml_dsa = None;
        assert_eq!(
            verify_hybrid(
                &provider,
                &params.chain_id,
                DomainTag::TransactionSignature,
                b"spend",
                &public,
                &missing_ml_dsa,
            ),
            Err(CryptoError::MissingMlDsaSignature)
        );

        let mut missing_schnorr = signature;
        missing_schnorr.schnorr = None;
        assert_eq!(
            verify_hybrid(
                &provider,
                &params.chain_id,
                DomainTag::TransactionSignature,
                b"spend",
                &public,
                &missing_schnorr,
            ),
            Err(CryptoError::MissingSchnorrSignature)
        );
    }

    #[test]
    fn chain_id_and_domain_prevent_replay() {
        let regtest = ChainParams::for_network(Network::Regtest);
        let testnet = ChainParams::for_network(Network::Testnet);
        let keys = HybridKeyPair::generate(None);
        let public = keys.public_keys();
        let signature = keys
            .sign_hybrid(&regtest.chain_id, DomainTag::TransactionSignature, b"spend")
            .unwrap();
        let provider = RustCryptoProvider;

        assert_eq!(
            verify_hybrid(
                &provider,
                &testnet.chain_id,
                DomainTag::TransactionSignature,
                b"spend",
                &public,
                &signature,
            ),
            Err(CryptoError::VerificationFailed)
        );
        assert_eq!(
            verify_hybrid(
                &provider,
                &regtest.chain_id,
                DomainTag::CheckpointSignature,
                b"spend",
                &public,
                &signature,
            ),
            Err(CryptoError::VerificationFailed)
        );
    }

    #[test]
    fn serialized_public_material_round_trips_without_secrets() {
        let public = HybridKeyPair::generate(None).public_keys();
        let json = serde_json::to_string(&public).unwrap();
        let decoded: HybridPublicKeys = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, public);
        assert!(!json.contains("private"));
        assert!(!json.contains("secret"));
    }
}
