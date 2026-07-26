use crate::hash::Hash32;
use primitive_types::U256;
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PowError {
    #[error("invalid compact proof-of-work target")]
    InvalidTarget,
    #[error("proof-of-work hash exceeds target")]
    InsufficientWork,
    #[error("chainwork arithmetic overflow")]
    ChainworkOverflow,
}

pub trait PowProvider: Send + Sync {
    fn version(&self) -> u16;
    fn hash(&self, header: &[u8]) -> Hash32;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Sha256dPowV1;

impl PowProvider for Sha256dPowV1 {
    fn version(&self) -> u16 {
        1
    }

    fn hash(&self, header: &[u8]) -> Hash32 {
        Hash32::sha256d(header)
    }
}

pub fn compact_to_target(bits: u32) -> Result<U256, PowError> {
    let exponent = bits >> 24;
    let mantissa = bits & 0x007f_ffff;
    if mantissa == 0 || bits & 0x0080_0000 != 0 || exponent > 32 {
        return Err(PowError::InvalidTarget);
    }

    let target = if exponent <= 3 {
        U256::from(mantissa >> (8 * (3 - exponent)))
    } else {
        U256::from(mantissa) << (8 * (exponent - 3))
    };
    if target.is_zero() {
        Err(PowError::InvalidTarget)
    } else {
        Ok(target)
    }
}

pub fn target_to_compact(target: U256) -> Result<u32, PowError> {
    if target.is_zero() {
        return Err(PowError::InvalidTarget);
    }
    let bytes = target.to_big_endian();
    let first = bytes
        .iter()
        .position(|byte| *byte != 0)
        .ok_or(PowError::InvalidTarget)?;
    let mut exponent = u32::try_from(32 - first).expect("target width is at most 32 bytes");
    let mut mantissa = if exponent <= 3 {
        target.low_u32() << (8 * (3 - exponent))
    } else {
        u32::from_be_bytes([
            bytes[first],
            *bytes.get(first + 1).unwrap_or(&0),
            *bytes.get(first + 2).unwrap_or(&0),
            0,
        ]) >> 8
    };
    if mantissa & 0x0080_0000 != 0 {
        mantissa >>= 8;
        exponent += 1;
    }
    Ok((exponent << 24) | (mantissa & 0x007f_ffff))
}

pub fn verify_pow(hash: Hash32, bits: u32) -> Result<(), PowError> {
    let target = compact_to_target(bits)?;
    let value = U256::from_big_endian(&hash.0);
    if value <= target {
        Ok(())
    } else {
        Err(PowError::InsufficientWork)
    }
}

pub fn work_for_bits(bits: u32) -> Result<U256, PowError> {
    let target = compact_to_target(bits)?;
    let denominator = target
        .checked_add(U256::one())
        .ok_or(PowError::InvalidTarget)?;
    Ok((U256::MAX - target) / denominator + U256::one())
}

pub fn adjusted_bits(
    previous_bits: u32,
    actual_timespan: u64,
    expected_timespan: u64,
    pow_limit_bits: u32,
) -> Result<u32, PowError> {
    if expected_timespan == 0 {
        return Err(PowError::InvalidTarget);
    }
    let clamped = actual_timespan.clamp(expected_timespan / 4, expected_timespan * 4);
    let previous = compact_to_target(previous_bits)?;
    let limit = compact_to_target(pow_limit_bits)?;
    let quotient = previous / U256::from(expected_timespan);
    let remainder = previous % U256::from(expected_timespan);
    let adjusted = quotient
        .checked_mul(U256::from(clamped))
        .and_then(|base| {
            base.checked_add(
                remainder.checked_mul(U256::from(clamped))? / U256::from(expected_timespan),
            )
        })
        .unwrap_or(limit)
        .min(limit);
    target_to_compact(adjusted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_target_round_trip_is_stable() {
        for bits in [0x1d00_ffff, 0x1f00_ffff, 0x2000_ffff, 0x207f_ffff] {
            let target = compact_to_target(bits).unwrap();
            assert_eq!(
                compact_to_target(target_to_compact(target).unwrap()).unwrap(),
                target
            );
        }
    }

    #[test]
    fn harder_targets_have_more_work() {
        assert!(work_for_bits(0x1f00_ffff).unwrap() > work_for_bits(0x207f_ffff).unwrap());
    }
}
