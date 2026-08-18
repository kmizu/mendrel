use mendrel_diagnostics::{Diagnostic, DiagnosticSpan, INVALID_TOKEN};
use mendrel_source::{ByteSpan, SourceFile};
use mendrel_syntax::{KEYWORDS, PUNCTUATION, Token, TokenKind};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LexResult {
    pub tokens: Vec<Token>,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn lex(source: &SourceFile) -> LexResult {
    let text = source.text();
    let mut tokens = Vec::new();
    let mut diagnostics = Vec::new();
    let mut offset = 0;

    while offset < text.len() {
        let rest = &text[offset..];
        if rest.starts_with("//") {
            let length = rest.find('\n').unwrap_or(rest.len());
            push(
                &mut tokens,
                TokenKind::LineComment,
                text,
                offset,
                offset + length,
            );
            offset += length;
            continue;
        }
        if rest.starts_with("/*") {
            let (length, closed) = block_comment_length(rest);
            let kind = if closed {
                TokenKind::BlockComment
            } else {
                TokenKind::Invalid
            };
            push(&mut tokens, kind, text, offset, offset + length);
            if !closed {
                diagnostics.push(invalid(
                    source,
                    offset,
                    offset + length,
                    diagnostics.len(),
                    "unterminated block comment",
                ));
            }
            offset += length;
            continue;
        }

        let character = rest.chars().next().expect("offset is before end of source");
        if character == '\t' {
            let end = offset + character.len_utf8();
            push(&mut tokens, TokenKind::Invalid, text, offset, end);
            diagnostics.push(invalid(
                source,
                offset,
                end,
                diagnostics.len(),
                "tab is not allowed outside a string literal",
            ));
            offset = end;
        } else if character.is_whitespace() {
            let length = rest
                .char_indices()
                .take_while(|(_, character)| character.is_whitespace() && *character != '\t')
                .map(|(index, character)| index + character.len_utf8())
                .last()
                .expect("the first character is whitespace");
            push(
                &mut tokens,
                TokenKind::Whitespace,
                text,
                offset,
                offset + length,
            );
            offset += length;
        } else if is_identifier_start(character) {
            let length = rest
                .char_indices()
                .take_while(|(_, character)| is_identifier_continue(*character))
                .map(|(index, character)| index + character.len_utf8())
                .last()
                .expect("the first character starts an identifier");
            let end = offset + length;
            let word = &text[offset..end];
            let kind = if KEYWORDS.binary_search(&word).is_ok() {
                TokenKind::Keyword
            } else {
                TokenKind::Identifier
            };
            push(&mut tokens, kind, text, offset, end);
            offset = end;
        } else if character.is_ascii_digit() {
            let length = rest
                .char_indices()
                .take_while(|(_, character)| character.is_ascii_digit() || *character == '_')
                .map(|(index, character)| index + character.len_utf8())
                .last()
                .expect("the first character is a digit");
            push(
                &mut tokens,
                TokenKind::Integer,
                text,
                offset,
                offset + length,
            );
            offset += length;
        } else if character == '"' {
            let (length, closed) = string_length(rest);
            let kind = if closed {
                TokenKind::String
            } else {
                TokenKind::Invalid
            };
            push(&mut tokens, kind, text, offset, offset + length);
            if !closed {
                diagnostics.push(invalid(
                    source,
                    offset,
                    offset + length,
                    diagnostics.len(),
                    "unterminated string literal",
                ));
            }
            offset += length;
        } else if let Some(punctuation) = longest_punctuation(rest) {
            let end = offset + punctuation.len();
            push(&mut tokens, TokenKind::Punctuation, text, offset, end);
            offset = end;
        } else {
            let end = offset + character.len_utf8();
            push(&mut tokens, TokenKind::Invalid, text, offset, end);
            diagnostics.push(invalid(
                source,
                offset,
                end,
                diagnostics.len(),
                "character is not part of the Mendrel lexical grammar",
            ));
            offset = end;
        }
    }

    let end = source.byte_len();
    tokens.push(Token::new(
        TokenKind::Eof,
        "",
        ByteSpan::new(end, end).expect("EOF span is ordered"),
    ));
    LexResult {
        tokens,
        diagnostics,
    }
}

fn push(tokens: &mut Vec<Token>, kind: TokenKind, text: &str, start: usize, end: usize) {
    tokens.push(Token::new(
        kind,
        &text[start..end],
        ByteSpan::new(
            u32::try_from(start).expect("source offsets fit u32"),
            u32::try_from(end).expect("source offsets fit u32"),
        )
        .expect("lexer emits ordered spans"),
    ));
}

fn invalid(
    source: &SourceFile,
    start: usize,
    end: usize,
    index: usize,
    summary: &str,
) -> Diagnostic {
    let span = ByteSpan::new(
        u32::try_from(start).expect("source offsets fit u32"),
        u32::try_from(end).expect("source offsets fit u32"),
    )
    .expect("lexer emits ordered diagnostic spans");
    Diagnostic::from_catalog(
        &INVALID_TOKEN,
        DiagnosticSpan::from_source(source, span).expect("lexer spans are source boundaries"),
        source.revision(),
    )
    .with_id(format!("diag-{:04}", index + 1))
    .with_summary(summary)
}

fn is_identifier_start(character: char) -> bool {
    character == '_' || character.is_alphabetic()
}

fn is_identifier_continue(character: char) -> bool {
    character == '_' || character.is_alphanumeric()
}

fn longest_punctuation(source: &str) -> Option<&'static str> {
    PUNCTUATION
        .iter()
        .copied()
        .filter(|punctuation| source.starts_with(punctuation))
        .max_by_key(|punctuation| punctuation.len())
}

fn block_comment_length(source: &str) -> (usize, bool) {
    let bytes = source.as_bytes();
    let mut offset = 2;
    let mut depth = 1_u32;
    while offset < bytes.len() {
        if bytes[offset..].starts_with(b"/*") {
            depth += 1;
            offset += 2;
        } else if bytes[offset..].starts_with(b"*/") {
            depth -= 1;
            offset += 2;
            if depth == 0 {
                return (offset, true);
            }
        } else {
            let character = source[offset..]
                .chars()
                .next()
                .expect("offset remains inside source");
            offset += character.len_utf8();
        }
    }
    (source.len(), false)
}

fn string_length(source: &str) -> (usize, bool) {
    let mut escaped = false;
    for (offset, character) in source.char_indices().skip(1) {
        if escaped {
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == '"' {
            return (offset + character.len_utf8(), true);
        } else if character == '\n' || character == '\r' {
            return (offset, false);
        }
    }
    (source.len(), false)
}
