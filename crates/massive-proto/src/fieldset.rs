//! Typed access to decoded fields, shared by the request and response decoders.

use thiserror::Error;

use crate::crypto::InvalidDeviceToken;
use crate::field::{Field, FieldError, Value};
use crate::frame::FrameError;
use crate::ids::InvalidText;
use crate::schema::Context;
use crate::tag::Tag;

#[derive(Debug, Clone, PartialEq, Error)]
pub enum DecodeError {
    #[error(transparent)]
    Frame(#[from] FrameError),
    #[error(transparent)]
    Field(#[from] FieldError),
    #[error("expected type byte 0x{expected:02X}, found 0x{found:02X}")]
    WrongType { expected: u8, found: u8 },
    #[error("type byte 0x{found:02X} is not valid here")]
    UnexpectedType { found: u8 },
    #[error("expected protocol version {expected}, found {found}")]
    WrongVersion { expected: u8, found: u8 },
    #[error("{context}: field {tag} is missing")]
    Missing { context: Context, tag: Tag },
    #[error("{context}: field {tag} at offset {offset} occurs more than once")]
    Duplicate {
        context: Context,
        tag: Tag,
        offset: usize,
    },
    #[error("{context}: field {tag} at offset {offset} is invalid: {reason}")]
    Invalid {
        context: Context,
        tag: Tag,
        offset: usize,
        reason: InvalidReason,
    },
    /// The retail parser counts some fields and requires an exact total.
    #[error("{context}: {found} counted fields where the client requires {required}")]
    FieldCount {
        context: Context,
        required: usize,
        found: usize,
    },
    #[error("{context}: count field {tag} is {declared} but {actual} records follow")]
    CountMismatch {
        context: Context,
        tag: Tag,
        declared: u16,
        actual: usize,
    },
    #[error("{context}: fields {present} and {absent} must occur together")]
    Incomplete {
        context: Context,
        present: Tag,
        absent: Tag,
    },
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum InvalidReason {
    #[error(transparent)]
    Text(#[from] InvalidText),
    #[error(transparent)]
    DeviceToken(#[from] InvalidDeviceToken),
    #[error("length {found}, expected {expected}")]
    Length { expected: usize, found: usize },
    #[error("length {found}, expected at least {minimum}")]
    TooShort { minimum: usize, found: usize },
    #[error("media type 0x{0:X} selects no object class")]
    MediaType(u32),
    #[error("value kind does not match the schema")]
    Kind,
}

pub(crate) type Result<T> = std::result::Result<T, DecodeError>;

#[derive(Debug, Clone, Copy)]
pub(crate) struct FieldSet<'a> {
    pub(crate) context: Context,
    pub(crate) fields: &'a [Field],
}

impl<'a> FieldSet<'a> {
    pub(crate) fn new(context: Context, fields: &'a [Field]) -> Self {
        Self { context, fields }
    }

    pub(crate) fn all(&self, tag: Tag) -> impl Iterator<Item = &'a Field> + use<'a> {
        self.fields.iter().filter(move |f| f.tag == tag)
    }

    pub(crate) fn count(&self, tags: &[Tag]) -> usize {
        self.fields.iter().filter(|f| tags.contains(&f.tag)).count()
    }

    pub(crate) fn optional(&self, tag: Tag) -> Result<Option<&'a Field>> {
        let mut it = self.all(tag);
        let first = it.next();
        if let Some(dup) = it.next() {
            return Err(DecodeError::Duplicate {
                context: self.context,
                tag,
                offset: dup.offset,
            });
        }
        Ok(first)
    }

    pub(crate) fn required(&self, tag: Tag) -> Result<&'a Field> {
        self.optional(tag)?.ok_or(DecodeError::Missing {
            context: self.context,
            tag,
        })
    }

    pub(crate) fn invalid(&self, field: &Field, reason: impl Into<InvalidReason>) -> DecodeError {
        DecodeError::Invalid {
            context: self.context,
            tag: field.tag,
            offset: field.offset,
            reason: reason.into(),
        }
    }

    pub(crate) fn opt<T>(
        &self,
        tag: Tag,
        convert: impl FnOnce(&Self, &'a Field) -> Result<T>,
    ) -> Result<Option<T>> {
        self.optional(tag)?.map(|f| convert(self, f)).transpose()
    }

    pub(crate) fn req<T>(
        &self,
        tag: Tag,
        convert: impl FnOnce(&Self, &'a Field) -> Result<T>,
    ) -> Result<T> {
        let field = self.required(tag)?;
        convert(self, field)
    }

    /// Requires `tags` to be all present or all absent.
    pub(crate) fn together(&self, tags: &[Tag]) -> Result<bool> {
        let present: Vec<Tag> = tags
            .iter()
            .copied()
            .filter(|t| self.all(*t).next().is_some())
            .collect();
        match (present.first(), tags.iter().find(|t| !present.contains(t))) {
            (Some(&p), Some(&a)) => Err(DecodeError::Incomplete {
                context: self.context,
                present: p,
                absent: a,
            }),
            (Some(_), None) => Ok(true),
            (None, _) => Ok(false),
        }
    }
}

pub(crate) fn u8_of(set: &FieldSet<'_>, f: &Field) -> Result<u8> {
    match f.value {
        Value::U8(v) => Ok(v),
        _ => Err(set.invalid(f, InvalidReason::Kind)),
    }
}

pub(crate) fn u16_of(set: &FieldSet<'_>, f: &Field) -> Result<u16> {
    match f.value {
        Value::U16(v) => Ok(v),
        _ => Err(set.invalid(f, InvalidReason::Kind)),
    }
}

pub(crate) fn u32_of(set: &FieldSet<'_>, f: &Field) -> Result<u32> {
    match f.value {
        Value::U32(v) => Ok(v),
        _ => Err(set.invalid(f, InvalidReason::Kind)),
    }
}

pub(crate) fn u64_of(set: &FieldSet<'_>, f: &Field) -> Result<u64> {
    match f.value {
        Value::U64(v) => Ok(v),
        _ => Err(set.invalid(f, InvalidReason::Kind)),
    }
}

pub(crate) fn f32_of(set: &FieldSet<'_>, f: &Field) -> Result<f32> {
    match f.value {
        Value::F32(v) => Ok(v),
        _ => Err(set.invalid(f, InvalidReason::Kind)),
    }
}

pub(crate) fn bytes_of<'a>(set: &FieldSet<'_>, f: &'a Field) -> Result<&'a [u8]> {
    match &f.value {
        Value::Bytes(v) => Ok(v),
        _ => Err(set.invalid(f, InvalidReason::Kind)),
    }
}

pub(crate) fn u32_array_of<'a>(set: &FieldSet<'_>, f: &'a Field) -> Result<&'a [u32]> {
    match &f.value {
        Value::U32Array(v) => Ok(v),
        _ => Err(set.invalid(f, InvalidReason::Kind)),
    }
}

pub(crate) fn block_of<'a>(set: &FieldSet<'_>, f: &'a Field) -> Result<&'a [Field]> {
    match &f.value {
        Value::Block(v) => Ok(v),
        _ => Err(set.invalid(f, InvalidReason::Kind)),
    }
}

pub(crate) fn str_of(set: &FieldSet<'_>, f: &Field) -> Result<String> {
    let Value::Str(bytes) = &f.value else {
        return Err(set.invalid(f, InvalidReason::Kind));
    };
    String::from_utf8(bytes.clone()).map_err(|e| {
        set.invalid(
            f,
            InvalidText {
                value: String::from_utf8_lossy(e.as_bytes()).into_owned(),
                max: crate::wire::STR_MAX,
            },
        )
    })
}

/// Converts a string field with a validating constructor.
pub(crate) fn text_of<T>(
    set: &FieldSet<'_>,
    f: &Field,
    ctor: impl FnOnce(String) -> std::result::Result<T, InvalidText>,
) -> Result<T> {
    ctor(str_of(set, f)?).map_err(|e| set.invalid(f, e))
}
