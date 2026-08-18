use std::fmt::Write;

use mendrel_source::ByteSpan;

use crate::SyntaxKind;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TokenKind {
    Whitespace,
    LineComment,
    BlockComment,
    Identifier,
    Keyword,
    Integer,
    String,
    Punctuation,
    Invalid,
    Missing,
    Eof,
}

impl TokenKind {
    #[must_use]
    pub const fn is_trivia(self) -> bool {
        matches!(
            self,
            Self::Whitespace | Self::LineComment | Self::BlockComment
        )
    }

    #[must_use]
    pub const fn is_recovery(self) -> bool {
        matches!(self, Self::Invalid | Self::Missing)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub text: String,
    pub span: ByteSpan,
}

impl Token {
    #[must_use]
    pub fn new(kind: TokenKind, text: impl Into<String>, span: ByteSpan) -> Self {
        let text = text.into();
        assert!(
            kind != TokenKind::Missing || (text.is_empty() && span.is_empty()),
            "missing tokens must be empty zero-width insertions"
        );
        Self { kind, text, span }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxElement {
    Node(SyntaxNode),
    Token(Token),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxNode {
    pub kind: SyntaxKind,
    pub children: Vec<SyntaxElement>,
}

impl SyntaxNode {
    #[must_use]
    pub fn new(kind: SyntaxKind, children: Vec<SyntaxElement>) -> Self {
        Self { kind, children }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxTree {
    root: SyntaxNode,
}

impl SyntaxTree {
    #[must_use]
    pub const fn new(root: SyntaxNode) -> Self {
        Self { root }
    }

    #[must_use]
    pub const fn root(&self) -> &SyntaxNode {
        &self.root
    }

    #[must_use]
    pub fn source_text(&self) -> String {
        let mut source = String::new();
        visit_tokens(&self.root, &mut |token| source.push_str(&token.text));
        source
    }

    #[must_use]
    pub fn tokens(&self) -> Vec<&Token> {
        let mut tokens = Vec::new();
        visit_tokens(&self.root, &mut |token| tokens.push(token));
        tokens
    }

    #[must_use]
    pub fn has_recovery(&self) -> bool {
        node_has_recovery(&self.root)
    }

    #[must_use]
    pub fn structural_fingerprint(&self) -> String {
        let mut fingerprint = String::new();
        fingerprint_node(&self.root, &mut fingerprint);
        fingerprint
    }

    #[must_use]
    pub fn dump(&self) -> String {
        let mut output = String::new();
        dump_node(&self.root, 0, &mut output);
        output
    }
}

fn visit_tokens<'tree>(node: &'tree SyntaxNode, visitor: &mut impl FnMut(&'tree Token)) {
    for child in &node.children {
        match child {
            SyntaxElement::Node(node) => visit_tokens(node, visitor),
            SyntaxElement::Token(token) => visitor(token),
        }
    }
}

fn node_has_recovery(node: &SyntaxNode) -> bool {
    node.kind == SyntaxKind::Error
        || node.children.iter().any(|child| match child {
            SyntaxElement::Node(node) => node_has_recovery(node),
            SyntaxElement::Token(token) => token.kind.is_recovery(),
        })
}

fn fingerprint_node(node: &SyntaxNode, output: &mut String) {
    write!(output, "({}", node.kind.as_str()).expect("writing to String cannot fail");
    for child in &node.children {
        match child {
            SyntaxElement::Node(node) => fingerprint_node(node, output),
            SyntaxElement::Token(token) if !token.kind.is_trivia() => {
                write!(output, " {:?}:{:?}", token.kind, token.text)
                    .expect("writing to String cannot fail");
            }
            SyntaxElement::Token(_) => {}
        }
    }
    output.push(')');
}

fn dump_node(node: &SyntaxNode, depth: usize, output: &mut String) {
    writeln!(output, "{}{}", "  ".repeat(depth), node.kind.as_str())
        .expect("writing to String cannot fail");
    for child in &node.children {
        match child {
            SyntaxElement::Node(node) => dump_node(node, depth + 1, output),
            SyntaxElement::Token(token) => {
                writeln!(
                    output,
                    "{}{:?}@{}..{} {:?}",
                    "  ".repeat(depth + 1),
                    token.kind,
                    token.span.start,
                    token.span.end,
                    token.text
                )
                .expect("writing to String cannot fail");
            }
        }
    }
}
