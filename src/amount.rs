use core::fmt;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Number of atomic units in one QBTC.
pub const ATOMS_PER_QBTC: u64 = 100_000_000;
/// Consensus maximum monetary supply.
pub const MAX_QBTC: u64 = 21_000_000;
/// Consensus maximum in atomic units.
pub const MAX_MONEY_ATOMS: u64 = MAX_QBTC * ATOMS_PER_QBTC;
/// Original Bitcoin-style initial subsidy.
pub const INITIAL_SUBSIDY_ATOMS: u64 = 50 * ATOMS_PER_QBTC;
/// Number of blocks between subsidy halvings.
pub const HALVING_INTERVAL: u64 = 210_000;

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum AmountError {
    #[error("amount exceeds the 21 million QBTC consensus limit")]
    OutOfRange,
    #[error("amount arithmetic overflow or underflow")]
    Arithmetic,
}

/// A consensus amount represented exclusively as unsigned atomic units.
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Amount(u64);

impl Amount {
    pub const ZERO: Self = Self(0);
    pub const MAX_MONEY: Self = Self(MAX_MONEY_ATOMS);

    pub const fn from_atoms(atoms: u64) -> Result<Self, AmountError> {
        if atoms <= MAX_MONEY_ATOMS {
            Ok(Self(atoms))
        } else {
            Err(AmountError::OutOfRange)
        }
    }

    #[must_use]
    pub const fn atoms(self) -> u64 {
        self.0
    }

    pub fn checked_add(self, other: Self) -> Result<Self, AmountError> {
        let atoms = self.0.checked_add(other.0).ok_or(AmountError::Arithmetic)?;
        Self::from_atoms(atoms)
    }

    pub fn checked_sub(self, other: Self) -> Result<Self, AmountError> {
        let atoms = self.0.checked_sub(other.0).ok_or(AmountError::Arithmetic)?;
        Self::from_atoms(atoms)
    }
}

impl fmt::Debug for Amount {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}.{:08} QBTC",
            self.0 / ATOMS_PER_QBTC,
            self.0 % ATOMS_PER_QBTC
        )
    }
}

/// Maximum permitted block subsidy at a height.
///
/// Height zero may claim the nominal subsidy, but `QBitCoin`'s genesis
/// specification deliberately claims zero and creates no spendable allocation.
#[must_use]
pub const fn block_subsidy(height: u64) -> Amount {
    let halvings = height / HALVING_INTERVAL;
    if halvings >= u64::BITS as u64 {
        return Amount::ZERO;
    }
    Amount(INITIAL_SUBSIDY_ATOMS >> halvings)
}

/// Sum of all maximum subsidies from genesis through `height`, saturating at
/// the monetary cap. Consensus code also checks actual UTXO creation.
#[must_use]
pub fn valid_subsidy_total_at_height(height: u64) -> Amount {
    let mut remaining = height.saturating_add(1);
    let mut subsidy = INITIAL_SUBSIDY_ATOMS;
    let mut total = 0_u64;

    while remaining > 0 && subsidy > 0 {
        let blocks = remaining.min(HALVING_INTERVAL);
        let era_total = subsidy.checked_mul(blocks).unwrap_or(MAX_MONEY_ATOMS);
        total = total.saturating_add(era_total).min(MAX_MONEY_ATOMS);
        remaining -= blocks;
        subsidy >>= 1;
    }

    Amount(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subsidy_halves_without_floating_point() {
        assert_eq!(block_subsidy(0).atoms(), 50 * ATOMS_PER_QBTC);
        assert_eq!(
            block_subsidy(HALVING_INTERVAL - 1).atoms(),
            50 * ATOMS_PER_QBTC
        );
        assert_eq!(block_subsidy(HALVING_INTERVAL).atoms(), 25 * ATOMS_PER_QBTC);
        assert_eq!(block_subsidy(HALVING_INTERVAL * 64).atoms(), 0);
    }

    #[test]
    fn cumulative_subsidy_never_exceeds_cap() {
        assert!(valid_subsidy_total_at_height(u64::MAX).atoms() <= MAX_MONEY_ATOMS);
    }

    #[test]
    fn amount_arithmetic_is_checked() {
        assert!(
            Amount::MAX_MONEY
                .checked_add(Amount::from_atoms(1).unwrap())
                .is_err()
        );
        assert!(
            Amount::ZERO
                .checked_sub(Amount::from_atoms(1).unwrap())
                .is_err()
        );
    }
}
