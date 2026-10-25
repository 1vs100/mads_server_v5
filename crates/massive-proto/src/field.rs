//! Schema-driven decoding of a field region into a tree.
//!
//! This layer knows value encodings but no message semantics. It is used by
//! the typed decoders and directly for analysis of captured traffic.

use std::fmt::{self, Write as _};

use thiserror::Error;

use crate::schema::{Context, FieldSpec, ValueKind};
use crate::tag::Tag;
use crate::wire::{Reader, Truncated};

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    F32(f32),
    Str(Vec<u8>),
    Bytes(Vec<u8>),
    U32Array(Vec<u32>),
    Block(Vec<Field>),
    /// Block tag not present in the schema, skipped the way the client skips it.
    Opaque(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    /// Absolute offset of the tag byte.
    pub offset: usize,
    pub tag: Tag,
    pub spec: Option<&'static FieldSpec>,
    pub value: Value,
}

impl Field {
    #[must_use]
    pub fn name(&self) -> &'static str {
        self.spec.map_or("unknown", |s| s.name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FieldError {
    #[error(transparent)]
    Truncated(#[from] Truncated),
    /// The retail parsers fail on this condition as well.
    #[error("unknown tag {tag} in {context} at offset {offset}")]
    UnknownTag {
        context: Context,
        tag: Tag,
        offset: usize,
    },
    #[error("string field {tag} in {context} at offset {offset} is {len} bytes, limit {max}")]
    StringTooLong {
        context: Context,
        tag: Tag,
        offset: usize,
        len: usize,
        max: usize,
    },
}

/// Fields decoded before the first error, and that error.
#[derive(Debug, Clone, PartialEq)]
pub struct PartialFields {
    pub fields: Vec<Field>,
    pub error: Option<FieldError>,
}

pub fn decode(reader: Reader<'_>, context: Context) -> Result<Vec<Field>, FieldError> {
    let partial = decode_partial(reader, context);
    match partial.error {
        Some(e) => Err(e),
        None => Ok(partial.fields),
    }
}

#[must_use]
pub fn decode_partial(mut reader: Reader<'_>, context: Context) -> PartialFields {
    let mut fields = Vec::new();
    while !reader.is_empty() {
        match decode_one(&mut reader, context) {
            Ok(field) => fields.push(field),
            Err(error) => {
                return PartialFields {
                    fields,
                    error: Some(error),
                };
            }
        }
    }
    PartialFields {
        fields,
        error: None,
    }
}

fn decode_one(reader: &mut Reader<'_>, context: Context) -> Result<Field, FieldError> {
    let offset = reader.offset();
    let tag = Tag(reader.u8()?);
    let Some(spec) = context.spec(tag) else {
        if tag.is_block() {
            let inner = reader.block()?;
            return Ok(Field {
                offset,
                tag,
                spec: None,
                value: Value::Opaque(inner.remaining().to_vec()),
            });
        }
        return Err(FieldError::UnknownTag {
            context,
            tag,
            offset,
        });
    };
    let value = match spec.kind {
        ValueKind::U8 => Value::U8(reader.u8()?),
        ValueKind::U16 => Value::U16(reader.u16()?),
        ValueKind::U32 => Value::U32(reader.u32()?),
        ValueKind::U64 => Value::U64(reader.u64()?),
        ValueKind::F32 => Value::F32(reader.f32()?),
        ValueKind::Str => {
            let bytes = reader.bytes16()?;
            if bytes.len() > crate::wire::STR_MAX {
                return Err(FieldError::StringTooLong {
                    context,
                    tag,
                    offset,
                    len: bytes.len(),
                    max: crate::wire::STR_MAX,
                });
            }
            Value::Str(bytes.to_vec())
        }
        ValueKind::Bytes => Value::Bytes(reader.bytes16()?.to_vec()),
        ValueKind::U32Array => Value::U32Array(reader.u32_array16()?),
        ValueKind::Block(inner_context) => Value::Block(decode(reader.block()?, inner_context)?),
    };
    Ok(Field {
        offset,
        tag,
        spec: Some(spec),
        value,
    })
}

/// Indented text rendering of a field tree, one field per line.
#[derive(Debug, Clone, Copy)]
pub struct Tree<'a>(pub &'a [Field]);

impl fmt::Display for Tree<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        render(self.0, 0, f)
    }
}

fn render(fields: &[Field], depth: usize, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    for field in fields {
        let confidence = field.spec.map_or_else(
            || "[-/-]".to_owned(),
            |s| format!("[{}/{}]", s.layout, s.meaning),
        );
        write!(
            f,
            "{:05X} {:indent$}{} {:<24} {} ",
            field.offset,
            "",
            field.tag,
            field.name(),
            confidence,
            indent = depth * 2
        )?;
        match &field.value {
            Value::Block(children) => {
                writeln!(f, "block ({} fields)", children.len())?;
                render(children, depth + 1, f)?;
            }
            other => writeln!(f, "{}", ValueText(other))?,
        }
    }
    Ok(())
}

struct ValueText<'a>(&'a Value);

impl fmt::Display for ValueText<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Value::U8(v) => write!(f, "u8 {v} (0x{v:02X})"),
            Value::U16(v) => write!(f, "u16 {v} (0x{v:04X})"),
            Value::U32(v) => write!(f, "u32 {v} (0x{v:08X})"),
            Value::U64(v) => write!(f, "u64 {v} (0x{v:016X})"),
            Value::F32(v) => write!(f, "f32 {v}"),
            Value::Str(b) => write!(f, "str {}", Escaped(b)),
            Value::Bytes(b) => write!(f, "bytes[{}] {}", b.len(), Hex(b)),
            Value::U32Array(v) => write!(f, "u32[{}] {v:?}", v.len()),
            Value::Opaque(b) => write!(f, "opaque[{}] {}", b.len(), Hex(b)),
            Value::Block(_) => f.write_str("block"),
        }
    }
}

/// Lowercase hex without separators.
#[derive(Debug, Clone, Copy)]
pub struct Hex<'a>(pub &'a [u8]);

impl fmt::Display for Hex<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for b in self.0 {
            write!(f, "{b:02x}")?;
        }
        Ok(())
    }
}

/// Quoted ASCII with non-printable bytes as `\xNN`.
#[derive(Debug, Clone, Copy)]
pub struct Escaped<'a>(pub &'a [u8]);

impl fmt::Display for Escaped<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_char('"')?;
        for &b in self.0 {
            match b {
                b'"' => f.write_str("\\\"")?,
                b'\\' => f.write_str("\\\\")?,
                0x20..=0x7E => f.write_char(char::from(b))?,
                _ => write!(f, "\\x{b:02x}")?,
            }
        }
        f.write_char('"')
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_low_tag_fails_like_the_client() {
        let bytes = [0x3D, 0x00, 0x01, b'a', 0x77];
        let partial = decode_partial(Reader::new(&bytes, 6), Context::LocateServiceRequest);
        assert_eq!(partial.fields.len(), 1);
        assert_eq!(
            partial.error,
            Some(FieldError::UnknownTag {
                context: Context::LocateServiceRequest,
                tag: Tag(0x77),
                offset: 10
            })
        );
    }

    #[test]
    fn unknown_block_tag_is_skipped() {
        let bytes = [0xF0, 0, 0, 0, 2, 0xAB, 0xCD, 0x3D, 0x00, 0x00];
        let fields = decode(Reader::new(&bytes, 0), Context::LocateServiceRequest).unwrap();
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].value, Value::Opaque(vec![0xAB, 0xCD]));
        assert_eq!(fields[1].offset, 7);
    }

    #[test]
    fn nested_offsets_are_absolute() {
        let bytes = [0xE4, 0, 0, 0, 2, 0x57, 0x03];
        let fields = decode(Reader::new(&bytes, 100), Context::PlayerUpdateRequest).unwrap();
        let Value::Block(inner) = &fields[0].value else {
            panic!("expected block");
        };
        assert_eq!(inner[0].offset, 105);
        assert_eq!(inner[0].value, Value::U8(3));
    }
}
