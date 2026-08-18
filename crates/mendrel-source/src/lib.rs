use std::error::Error;
use std::fmt;

mod digest;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ByteSpan {
    pub start: u32,
    pub end: u32,
}

impl ByteSpan {
    pub fn new(start: u32, end: u32) -> Result<Self, SourceError> {
        if start <= end {
            Ok(Self { start, end })
        } else {
            Err(SourceError::InvalidSpan { start, end })
        }
    }

    #[must_use]
    pub const fn len(self) -> u32 {
        self.end - self.start
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Position {
    pub byte: u32,
    pub line: u32,
    pub column_utf16: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceError {
    InvalidUtf8 { valid_up_to: usize },
    SourceTooLarge { length: usize },
    InvalidBoundary { byte: usize },
    InvalidSpan { start: u32, end: u32 },
    SpanOutOfBounds { byte: usize },
}

impl fmt::Display for SourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUtf8 { valid_up_to } => {
                write!(formatter, "source is not UTF-8 at byte {valid_up_to}")
            }
            Self::SourceTooLarge { length } => {
                write!(
                    formatter,
                    "source length {length} exceeds the u32 byte-span limit"
                )
            }
            Self::InvalidBoundary { byte } => {
                write!(formatter, "byte {byte} is not a UTF-8 boundary")
            }
            Self::InvalidSpan { start, end } => {
                write!(formatter, "span start {start} is after end {end}")
            }
            Self::SpanOutOfBounds { byte } => {
                write!(formatter, "byte {byte} is outside the source")
            }
        }
    }
}

impl Error for SourceError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceFile {
    path: String,
    text: String,
    line_starts: Vec<u32>,
    byte_len: u32,
    revision: String,
}

impl SourceFile {
    pub fn from_bytes(path: impl Into<String>, bytes: Vec<u8>) -> Result<Self, SourceError> {
        let byte_len = checked_byte_len(bytes.len())?;
        let revision = content_revision(&bytes);
        let text = String::from_utf8(bytes).map_err(|error| SourceError::InvalidUtf8 {
            valid_up_to: error.utf8_error().valid_up_to(),
        })?;
        let line_starts = line_starts(&text);
        Ok(Self {
            path: normalize_path(&path.into()),
            text,
            line_starts,
            byte_len,
            revision,
        })
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    #[must_use]
    pub fn byte_len(&self) -> u32 {
        self.byte_len
    }

    #[must_use]
    pub fn revision(&self) -> &str {
        &self.revision
    }

    pub fn position(&self, byte: u32) -> Result<Position, SourceError> {
        let byte = usize::try_from(byte).expect("u32 always fits usize on supported targets");
        if byte > self.text.len() {
            return Err(SourceError::SpanOutOfBounds { byte });
        }
        if !self.text.is_char_boundary(byte) {
            return Err(SourceError::InvalidBoundary { byte });
        }

        let line_index = self.line_starts.partition_point(|start| {
            usize::try_from(*start).expect("line offset fits usize") <= byte
        }) - 1;
        let line_start =
            usize::try_from(self.line_starts[line_index]).expect("line offset fits usize");
        let column_utf16 = self.text[line_start..byte].encode_utf16().count();

        Ok(Position {
            byte: u32::try_from(byte).expect("byte originated as u32"),
            line: u32::try_from(line_index).expect("line count is limited by source length"),
            column_utf16: u32::try_from(column_utf16)
                .expect("UTF-16 column is limited by source length"),
        })
    }
}

fn line_starts(text: &str) -> Vec<u32> {
    let mut starts = vec![0];
    for (index, byte) in text.bytes().enumerate() {
        if byte == b'\n' {
            starts.push(u32::try_from(index + 1).expect("source is limited to u32 byte spans"));
        }
    }
    starts
}

#[must_use]
pub fn normalize_path(path: &str) -> String {
    let replaced = path.replace('\\', "/");
    let absolute = replaced.starts_with('/');
    let mut segments: Vec<&str> = Vec::new();

    for segment in replaced.split('/') {
        match segment {
            "" | "." => {}
            ".." if segments.last().is_some_and(|last| *last != "..") => {
                segments.pop();
            }
            ".." if !absolute => segments.push(segment),
            ".." => {}
            _ => segments.push(segment),
        }
    }

    let normalized = segments.join("/");
    if absolute {
        format!("/{normalized}")
    } else if normalized.is_empty() {
        ".".to_owned()
    } else {
        normalized
    }
}

#[must_use]
pub fn content_revision(bytes: &[u8]) -> String {
    digest::revision(bytes)
}

pub fn position_in_valid_utf8_prefix(
    bytes: &[u8],
    valid_up_to: usize,
) -> Result<Position, SourceError> {
    if valid_up_to > bytes.len() {
        return Err(SourceError::SpanOutOfBounds { byte: valid_up_to });
    }
    let byte = checked_byte_len(valid_up_to)?;
    let prefix =
        std::str::from_utf8(&bytes[..valid_up_to]).map_err(|error| SourceError::InvalidUtf8 {
            valid_up_to: error.valid_up_to(),
        })?;
    let line_start = prefix.rfind('\n').map_or(0, |index| index + 1);
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count();
    let column_utf16 = prefix[line_start..].encode_utf16().count();

    Ok(Position {
        byte,
        line: u32::try_from(line).expect("line count cannot exceed validated source length"),
        column_utf16: u32::try_from(column_utf16)
            .expect("column cannot exceed validated source length"),
    })
}

fn checked_byte_len(length: usize) -> Result<u32, SourceError> {
    u32::try_from(length).map_err(|_| SourceError::SourceTooLarge { length })
}

#[cfg(all(test, target_pointer_width = "64"))]
mod tests {
    use super::{SourceError, checked_byte_len};

    #[test]
    fn rejects_source_lengths_larger_than_the_span_domain() {
        let length = usize::try_from(u64::from(u32::MAX) + 1).expect("64-bit usize");
        assert_eq!(
            checked_byte_len(length),
            Err(SourceError::SourceTooLarge { length })
        );
    }
}
