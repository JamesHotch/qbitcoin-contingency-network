use crate::{
    crypto::{HybridPublicKeys, SpendPolicyVersion},
    encoding::put_bytes,
    hash::Hash32,
    params::ChainParams,
};
use bech32::{Bech32m, Hrp, primitives::decode::CheckedHrpstring};
use thiserror::Error;

pub const ADDRESS_VERSION_HYBRID_V1: u8 = 1;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AddressError {
    #[error("invalid Bech32m address")]
    InvalidEncoding,
    #[error("address belongs to another network")]
    WrongNetwork,
    #[error("unsupported address version or payload length")]
    UnsupportedVersion,
}

#[must_use]
pub fn policy_commitment(keys: &HybridPublicKeys) -> Hash32 {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&(keys.policy_version as u16).to_le_bytes());
    put_bytes(&keys.schnorr, &mut bytes);
    put_bytes(&keys.ml_dsa, &mut bytes);
    match &keys.recovery {
        Some(recovery) => {
            bytes.push(1);
            bytes.extend_from_slice(&(recovery.policy_version as u16).to_le_bytes());
            bytes.extend_from_slice(&recovery.relative_timelock.to_le_bytes());
            put_bytes(&recovery.slh_dsa, &mut bytes);
        }
        None => bytes.push(0),
    }
    Hash32::tagged_sha3("QBTC/OUTPUT-POLICY/v1", &bytes)
}

pub fn encode_address(
    params: &ChainParams,
    keys: &HybridPublicKeys,
) -> Result<String, AddressError> {
    if keys.policy_version != SpendPolicyVersion::HybridSchnorrMlDsaV1 {
        return Err(AddressError::UnsupportedVersion);
    }
    let mut payload = Vec::with_capacity(33);
    payload.push(ADDRESS_VERSION_HYBRID_V1);
    payload.extend_from_slice(&policy_commitment(keys).0);
    let hrp = Hrp::parse(&params.bech32m_hrp).map_err(|_| AddressError::InvalidEncoding)?;
    bech32::encode::<Bech32m>(hrp, &payload).map_err(|_| AddressError::InvalidEncoding)
}

pub fn decode_address(params: &ChainParams, address: &str) -> Result<Hash32, AddressError> {
    let checked =
        CheckedHrpstring::new::<Bech32m>(address).map_err(|_| AddressError::InvalidEncoding)?;
    let expected_hrp =
        Hrp::parse(&params.bech32m_hrp).map_err(|_| AddressError::InvalidEncoding)?;
    if checked.hrp() != expected_hrp {
        return Err(AddressError::WrongNetwork);
    }
    let payload: Vec<u8> = checked.byte_iter().collect();
    if payload.len() != 33 || payload[0] != ADDRESS_VERSION_HYBRID_V1 {
        return Err(AddressError::UnsupportedVersion);
    }
    Ok(Hash32(
        payload[1..]
            .try_into()
            .map_err(|_| AddressError::UnsupportedVersion)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        crypto::HybridKeyPair,
        params::{ChainParams, Network},
    };

    #[test]
    fn bech32m_address_is_bound_to_network_and_policy() {
        let regtest = ChainParams::for_network(Network::Regtest);
        let testnet = ChainParams::for_network(Network::Testnet);
        let keys = HybridKeyPair::generate(None).public_keys();
        let address = encode_address(&regtest, &keys).unwrap();
        assert!(address.starts_with("rqbtc1"));
        assert_eq!(
            decode_address(&regtest, &address).unwrap(),
            policy_commitment(&keys)
        );
        assert_eq!(
            decode_address(&testnet, &address),
            Err(AddressError::WrongNetwork)
        );
    }
}
