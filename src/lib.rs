#![forbid(unsafe_code)]

//! Research implementation of the QBitCoin contingency-network protocol.
//!
//! This crate is testnet-only, unaudited, and unsuitable for real financial value.

pub mod amount;
pub mod block;
pub mod chain;
pub mod crypto;
pub mod encoding;
pub mod finality;
pub mod hash;
pub mod params;
pub mod pow;
pub mod transaction;
pub mod utxo;

/// The software and consensus format version.
pub const SOFTWARE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Returns whether the explicit, unsafe mainnet build gate was set at compile time.
#[must_use]
pub const fn unaudited_mainnet_build_enabled() -> bool {
    cfg!(enable_unaudited_mainnet)
}

#[cfg(test)]
mod tests {
    #[test]
    fn mainnet_is_disabled_in_normal_builds() {
        assert!(!super::unaudited_mainnet_build_enabled());
    }
}
