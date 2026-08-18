use std::error::Error;
use std::fmt;

use mendrel_syntax::{SPACED_OPERATORS, SyntaxTree, Token, TokenKind};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormatError {
    MalformedTree,
}

impl fmt::Display for FormatError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedTree => {
                formatter.write_str("full formatting requires a CST without recovery elements")
            }
        }
    }
}

impl Error for FormatError {}

pub fn format(tree: &SyntaxTree) -> Result<String, FormatError> {
    if tree.has_recovery() {
        return Err(FormatError::MalformedTree);
    }

    let mut formatter = Formatter::default();
    let tokens = tree.tokens();
    for (index, token) in tokens.iter().enumerate() {
        formatter.token(
            token,
            has_following_comment_on_same_line(&tokens, index),
            next_code_text(&tokens, index),
        );
    }
    Ok(formatter.finish())
}

fn has_following_comment_on_same_line(tokens: &[&Token], index: usize) -> bool {
    for token in &tokens[index + 1..] {
        match token.kind {
            TokenKind::Whitespace if token.text.contains(['\n', '\r']) => return false,
            TokenKind::Whitespace => {}
            TokenKind::LineComment | TokenKind::BlockComment => return true,
            _ => return false,
        }
    }
    false
}

fn next_code_text<'a>(tokens: &'a [&Token], index: usize) -> Option<&'a str> {
    tokens[index + 1..]
        .iter()
        .find(|token| {
            !matches!(
                token.kind,
                TokenKind::Whitespace
                    | TokenKind::LineComment
                    | TokenKind::BlockComment
                    | TokenKind::Eof
                    | TokenKind::Missing
            )
        })
        .map(|token| token.text.as_str())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BraceLayout {
    Block,
    Inline,
}

#[derive(Default)]
struct Formatter {
    output: String,
    indent: usize,
    module_terminated: bool,
    in_import: bool,
    pending_top_level_separator: bool,
    brace_layouts: Vec<BraceLayout>,
    previous_significant: Option<(TokenKind, String)>,
}

impl Formatter {
    fn token(&mut self, token: &Token, has_following_comment: bool, next_code_text: Option<&str>) {
        match token.kind {
            TokenKind::Whitespace | TokenKind::Eof | TokenKind::Missing => {}
            TokenKind::LineComment => self.line_comment(&token.text),
            TokenKind::BlockComment => {
                self.block_comment(&token.text, has_following_comment);
            }
            TokenKind::Punctuation => {
                self.punctuation(&token.text, has_following_comment, next_code_text);
            }
            TokenKind::Identifier
            | TokenKind::Keyword
            | TokenKind::Integer
            | TokenKind::String
            | TokenKind::Invalid => self.word(token.kind, &token.text),
        }
    }

    fn line_comment(&mut self, text: &str) {
        if self.at_line_start() {
            self.write_indent();
        } else {
            self.ensure_space();
        }
        self.output.push_str(text);
        self.newline();
        self.complete_pending_top_level_separator();
    }

    fn block_comment(&mut self, text: &str, has_following_comment: bool) {
        if self.at_line_start() {
            self.write_indent();
        } else {
            self.ensure_space();
        }
        self.output.push_str(text);
        if self.in_import {
            self.ensure_space();
            return;
        }
        if self.pending_top_level_separator && has_following_comment {
            return;
        }
        self.newline();
        self.complete_pending_top_level_separator();
    }

    fn word(&mut self, kind: TokenKind, text: &str) {
        if self.indent == 0 && text == "import" {
            self.in_import = true;
        }
        self.write_indent();
        if self
            .previous_significant
            .as_ref()
            .is_some_and(|(previous, _)| is_word(*previous))
        {
            self.ensure_space();
        }
        self.output.push_str(text);
        self.previous_significant = Some((kind, text.to_owned()));
    }

    fn punctuation(
        &mut self,
        text: &str,
        has_following_comment: bool,
        next_code_text: Option<&str>,
    ) {
        match text {
            "{" => {
                let layout = if self.in_import
                    && self
                        .previous_significant
                        .as_ref()
                        .is_some_and(|(kind, previous)| {
                            *kind == TokenKind::Punctuation && previous == "."
                        }) {
                    BraceLayout::Inline
                } else {
                    BraceLayout::Block
                };
                self.brace_layouts.push(layout);
                match layout {
                    BraceLayout::Inline => {
                        self.trim_spaces();
                        self.output.push('{');
                    }
                    BraceLayout::Block => {
                        self.ensure_space();
                        self.output.push('{');
                        self.newline();
                        self.indent += 1;
                    }
                }
            }
            "}" => match self.brace_layouts.pop().unwrap_or(BraceLayout::Block) {
                BraceLayout::Inline => {
                    self.trim_spaces();
                    self.output.push('}');
                }
                BraceLayout::Block => {
                    if !self.at_line_start() {
                        self.newline();
                    }
                    self.indent = self.indent.saturating_sub(1);
                    self.write_indent();
                    self.output.push('}');
                    if self.indent == 0 {
                        if has_following_comment {
                            self.pending_top_level_separator = true;
                        } else {
                            self.blank_line();
                        }
                    }
                }
            },
            ";" => {
                self.trim_spaces();
                self.output.push(';');
                if self.indent == 0 && !self.module_terminated {
                    self.module_terminated = true;
                    if has_following_comment {
                        self.pending_top_level_separator = true;
                    } else {
                        self.blank_line();
                    }
                } else if self.indent == 0 && self.in_import {
                    self.in_import = false;
                    let another_import_follows = next_code_text == Some("import");
                    if has_following_comment {
                        self.pending_top_level_separator = !another_import_follows;
                    } else if another_import_follows || next_code_text.is_none() {
                        self.newline();
                    } else {
                        self.blank_line();
                    }
                } else {
                    self.newline();
                }
            }
            "," => {
                self.trim_spaces();
                self.output.push(',');
                self.output.push(' ');
            }
            ":" => {
                self.trim_spaces();
                self.output.push(':');
                self.output.push(' ');
            }
            "(" => {
                self.write_indent();
                self.output.push_str(text);
            }
            "[" => {
                self.trim_spaces();
                self.output.push_str(text);
            }
            ")" | "]" => {
                self.trim_spaces();
                self.output.push_str(text);
            }
            "." | "::" | "?" => {
                self.trim_spaces();
                self.output.push_str(text);
            }
            _ if SPACED_OPERATORS.binary_search(&text).is_ok() => {
                self.ensure_space();
                self.output.push_str(text);
                self.output.push(' ');
            }
            _ => {
                self.trim_spaces();
                self.output.push_str(text);
            }
        }
        self.previous_significant = Some((TokenKind::Punctuation, text.to_owned()));
    }

    fn finish(mut self) -> String {
        self.trim_spaces();
        while self.output.ends_with("\n\n") {
            self.output.pop();
        }
        if !self.output.ends_with('\n') {
            self.output.push('\n');
        }
        self.output
    }

    fn write_indent(&mut self) {
        if self.at_line_start() {
            self.output.push_str(&" ".repeat(self.indent * 4));
        }
    }

    fn ensure_space(&mut self) {
        if !self.output.is_empty()
            && !self.at_line_start()
            && !self.output.ends_with(char::is_whitespace)
        {
            self.output.push(' ');
        }
    }

    fn trim_spaces(&mut self) {
        while self.output.ends_with(' ') {
            self.output.pop();
        }
    }

    fn newline(&mut self) {
        self.trim_spaces();
        if !self.output.ends_with('\n') {
            self.output.push('\n');
        }
    }

    fn blank_line(&mut self) {
        self.newline();
        if !self.output.ends_with("\n\n") {
            self.output.push('\n');
        }
    }

    fn complete_pending_top_level_separator(&mut self) {
        if self.pending_top_level_separator {
            self.blank_line();
            self.pending_top_level_separator = false;
        }
    }

    fn at_line_start(&self) -> bool {
        self.output.is_empty() || self.output.ends_with('\n')
    }
}

fn is_word(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Identifier | TokenKind::Keyword | TokenKind::Integer | TokenKind::String
    )
}
