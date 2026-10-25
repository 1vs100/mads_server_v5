//! Big-endian primitives.
//!
//! Offsets in errors are absolute within the message that was handed to the
//! outermost [`Reader`], so they can be matched against a hex dump.

use thiserror::Error;

/// Longest string the retail writer emits (`sub_824FA640` uses
/// `strnlen(s, 2000)`) and the retail reader accepts (`sub_824FA168`).
pub const STR_MAX: usize = 2000;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("truncated at offset {offset}: {needed} bytes needed, {available} available")]
pub struct Truncated {
    pub offset: usize,
    pub needed: usize,
    pub available: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EncodeError {
    #[error("value of {len} bytes exceeds the limit of {max} bytes")]
    TooLong { len: usize, max: usize },
}

#[derive(Debug, Clone)]
pub struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
    origin: usize,
}

impl<'a> Reader<'a> {
    /// `origin` is the absolute offset of `buf[0]` in the enclosing message.
    #[must_use]
    pub fn new(buf: &'a [u8], origin: usize) -> Self {
        Self {
            buf,
            pos: 0,
            origin,
        }
    }

    #[must_use]
    pub fn offset(&self) -> usize {
        self.origin + self.pos
    }

    #[must_use]
    pub fn remaining(&self) -> &'a [u8] {
        &self.buf[self.pos..]
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pos == self.buf.len()
    }

    pub fn take(&mut self, n: usize) -> Result<&'a [u8], Truncated> {
        let available = self.buf.len() - self.pos;
        if n > available {
            return Err(Truncated {
                offset: self.offset(),
                needed: n,
                available,
            });
        }
        let out = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        Ok(out)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], Truncated> {
        let mut out = [0; N];
        out.copy_from_slice(self.take(N)?);
        Ok(out)
    }

    pub fn u8(&mut self) -> Result<u8, Truncated> {
        Ok(self.array::<1>()?[0])
    }

    pub fn u16(&mut self) -> Result<u16, Truncated> {
        self.array().map(u16::from_be_bytes)
    }

    pub fn u32(&mut self) -> Result<u32, Truncated> {
        self.array().map(u32::from_be_bytes)
    }

    pub fn u64(&mut self) -> Result<u64, Truncated> {
        self.array().map(u64::from_be_bytes)
    }

    pub fn f32(&mut self) -> Result<f32, Truncated> {
        self.array().map(f32::from_be_bytes)
    }

    /// `u16` length followed by that many bytes.
    pub fn bytes16(&mut self) -> Result<&'a [u8], Truncated> {
        let len = self.u16()?;
        self.take(usize::from(len))
    }

    /// `u16` count followed by that many `u32` values (`sub_824F9540`).
    pub fn u32_array16(&mut self) -> Result<Vec<u32>, Truncated> {
        let count = self.u16()?;
        (0..count).map(|_| self.u32()).collect()
    }

    /// `u32` length followed by a nested region.
    pub fn block(&mut self) -> Result<Reader<'a>, Truncated> {
        let len = self.u32()?;
        let len = usize::try_from(len).unwrap_or(usize::MAX);
        let origin = self.offset();
        self.take(len).map(|buf| Reader::new(buf, origin))
    }
}

#[derive(Debug, Clone, Default)]
pub struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn into_inner(self) -> Vec<u8> {
        self.buf
    }

    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.buf
    }

    pub fn raw(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    pub fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }

    pub fn u16(&mut self, v: u16) {
        self.raw(&v.to_be_bytes());
    }

    pub fn u32(&mut self, v: u32) {
        self.raw(&v.to_be_bytes());
    }

    pub fn u64(&mut self, v: u64) {
        self.raw(&v.to_be_bytes());
    }

    pub fn f32(&mut self, v: f32) {
        self.raw(&v.to_be_bytes());
    }

    pub fn bytes16(&mut self, bytes: &[u8]) -> Result<(), EncodeError> {
        let len = u16::try_from(bytes.len()).map_err(|_| EncodeError::TooLong {
            len: bytes.len(),
            max: usize::from(u16::MAX),
        })?;
        self.u16(len);
        self.raw(bytes);
        Ok(())
    }

    /// String field; enforces [`STR_MAX`] so the retail reader accepts it.
    pub fn str16(&mut self, s: &str) -> Result<(), EncodeError> {
        if s.len() > STR_MAX {
            return Err(EncodeError::TooLong {
                len: s.len(),
                max: STR_MAX,
            });
        }
        self.bytes16(s.as_bytes())
    }

    pub fn u32_array16(&mut self, values: &[u32]) -> Result<(), EncodeError> {
        let count = u16::try_from(values.len()).map_err(|_| EncodeError::TooLong {
            len: values.len(),
            max: usize::from(u16::MAX),
        })?;
        self.u16(count);
        for v in values {
            self.u32(*v);
        }
        Ok(())
    }

    /// Writes `tag | u32 length | body` where `body` is produced by `f`.
    pub fn block<E>(
        &mut self,
        tag: u8,
        f: impl FnOnce(&mut Writer) -> Result<(), E>,
    ) -> Result<(), E>
    where
        E: From<EncodeError>,
    {
        let mut inner = Writer::new();
        f(&mut inner)?;
        let len = u32::try_from(inner.buf.len()).map_err(|_| EncodeError::TooLong {
            len: inner.buf.len(),
            max: u32::MAX as usize,
        })?;
        self.u8(tag);
        self.u32(len);
        self.raw(&inner.buf);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncation_reports_absolute_offset() {
        let mut r = Reader::new(&[0x00, 0x05, b'a'], 10);
        let err = r.bytes16().unwrap_err();
        assert_eq!(
            err,
            Truncated {
                offset: 12,
                needed: 5,
                available: 1
            }
        );
    }

    #[test]
    fn block_origin_is_absolute() {
        let mut w = Writer::new();
        w.u8(0xAA);
        w.block::<EncodeError>(0xE4, |w| {
            w.u16(7);
            Ok(())
        })
        .unwrap();
        let bytes = w.into_inner();
        assert_eq!(bytes, [0xAA, 0xE4, 0, 0, 0, 2, 0, 7]);
        let mut r = Reader::new(&bytes, 0);
        r.take(2).unwrap();
        let inner = r.block().unwrap();
        assert_eq!(inner.offset(), 6);
    }

    #[test]
    fn str16_enforces_retail_limit() {
        let long = "x".repeat(STR_MAX + 1);
        assert!(Writer::new().str16(&long).is_err());
        assert!(Writer::new().str16(&long[..STR_MAX]).is_ok());
    }
}
