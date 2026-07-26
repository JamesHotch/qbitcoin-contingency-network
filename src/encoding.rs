use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum EncodingError {
    #[error("unexpected end of input")]
    UnexpectedEnd,
    #[error("non-canonical compact integer")]
    NonCanonicalCompact,
    #[error("length exceeds consensus limit")]
    LengthLimit,
    #[error("unknown enum tag or version")]
    UnknownTag,
    #[error("trailing bytes after canonical object")]
    TrailingBytes,
    #[error("invalid encoded value")]
    InvalidValue,
}

pub fn put_compact(value: u64, output: &mut Vec<u8>) {
    match value {
        0..=0xfc => output.push(u8::try_from(value).expect("match arm bounds the value")),
        0xfd..=0xffff => {
            output.push(0xfd);
            output.extend_from_slice(
                &u16::try_from(value)
                    .expect("match arm bounds the value")
                    .to_le_bytes(),
            );
        }
        0x1_0000..=0xffff_ffff => {
            output.push(0xfe);
            output.extend_from_slice(
                &u32::try_from(value)
                    .expect("match arm bounds the value")
                    .to_le_bytes(),
            );
        }
        _ => {
            output.push(0xff);
            output.extend_from_slice(&value.to_le_bytes());
        }
    }
}

pub fn put_bytes(bytes: &[u8], output: &mut Vec<u8>) {
    put_compact(bytes.len() as u64, output);
    output.extend_from_slice(bytes);
}

pub struct Decoder<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> Decoder<'a> {
    #[must_use]
    pub const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    pub fn read_u8(&mut self) -> Result<u8, EncodingError> {
        Ok(self.read_array::<1>()?[0])
    }

    pub fn read_u16(&mut self) -> Result<u16, EncodingError> {
        Ok(u16::from_le_bytes(self.read_array()?))
    }

    pub fn read_u32(&mut self) -> Result<u32, EncodingError> {
        Ok(u32::from_le_bytes(self.read_array()?))
    }

    pub fn read_u64(&mut self) -> Result<u64, EncodingError> {
        Ok(u64::from_le_bytes(self.read_array()?))
    }

    pub fn read_array<const N: usize>(&mut self) -> Result<[u8; N], EncodingError> {
        let end = self
            .cursor
            .checked_add(N)
            .ok_or(EncodingError::UnexpectedEnd)?;
        let bytes = self
            .bytes
            .get(self.cursor..end)
            .ok_or(EncodingError::UnexpectedEnd)?;
        self.cursor = end;
        bytes.try_into().map_err(|_| EncodingError::UnexpectedEnd)
    }

    pub fn read_compact(&mut self) -> Result<u64, EncodingError> {
        match self.read_u8()? {
            value @ 0x00..=0xfc => Ok(u64::from(value)),
            0xfd => {
                let value = u64::from(self.read_u16()?);
                (value >= 0xfd)
                    .then_some(value)
                    .ok_or(EncodingError::NonCanonicalCompact)
            }
            0xfe => {
                let value = u64::from(self.read_u32()?);
                (value > 0xffff)
                    .then_some(value)
                    .ok_or(EncodingError::NonCanonicalCompact)
            }
            0xff => {
                let value = self.read_u64()?;
                (value > u64::from(u32::MAX))
                    .then_some(value)
                    .ok_or(EncodingError::NonCanonicalCompact)
            }
        }
    }

    pub fn read_bytes(&mut self, maximum: usize) -> Result<Vec<u8>, EncodingError> {
        let length =
            usize::try_from(self.read_compact()?).map_err(|_| EncodingError::LengthLimit)?;
        if length > maximum {
            return Err(EncodingError::LengthLimit);
        }
        let end = self
            .cursor
            .checked_add(length)
            .ok_or(EncodingError::UnexpectedEnd)?;
        let bytes = self
            .bytes
            .get(self.cursor..end)
            .ok_or(EncodingError::UnexpectedEnd)?;
        self.cursor = end;
        Ok(bytes.to_vec())
    }

    pub fn finish(self) -> Result<(), EncodingError> {
        if self.cursor == self.bytes.len() {
            Ok(())
        } else {
            Err(EncodingError::TrailingBytes)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_integer_boundaries_round_trip() {
        for value in [
            0,
            0xfc,
            0xfd,
            0xffff,
            0x1_0000,
            u64::from(u32::MAX),
            u64::from(u32::MAX) + 1,
            u64::MAX,
        ] {
            let mut bytes = Vec::new();
            put_compact(value, &mut bytes);
            let mut decoder = Decoder::new(&bytes);
            assert_eq!(decoder.read_compact().unwrap(), value);
            decoder.finish().unwrap();
        }
    }

    #[test]
    fn non_minimal_integer_is_rejected() {
        assert_eq!(
            Decoder::new(&[0xfd, 0xfc, 0x00]).read_compact(),
            Err(EncodingError::NonCanonicalCompact)
        );
    }
}
