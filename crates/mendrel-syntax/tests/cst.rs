use mendrel_source::ByteSpan;
use mendrel_syntax::{SyntaxElement, SyntaxKind, SyntaxNode, SyntaxTree, Token, TokenKind};

fn token(kind: TokenKind, text: &str, start: u32, end: u32) -> SyntaxElement {
    SyntaxElement::Token(Token::new(
        kind,
        text,
        ByteSpan::new(start, end).expect("ordered span"),
    ))
}

#[test]
fn cst_reconstructs_every_token_including_trivia() {
    let tree = SyntaxTree::new(SyntaxNode::new(
        SyntaxKind::SourceFile,
        vec![
            token(TokenKind::Keyword, "module", 0, 6),
            token(TokenKind::Whitespace, " ", 6, 7),
            token(TokenKind::Identifier, "demo", 7, 11),
            token(TokenKind::Punctuation, ";", 11, 12),
            token(TokenKind::Eof, "", 12, 12),
        ],
    ));

    assert_eq!(tree.source_text(), "module demo;");
    assert!(!tree.has_recovery());
}

#[test]
fn structural_fingerprint_ignores_trivia_and_spans() {
    let compact = SyntaxTree::new(SyntaxNode::new(
        SyntaxKind::SourceFile,
        vec![
            token(TokenKind::Keyword, "module", 0, 6),
            token(TokenKind::Whitespace, " ", 6, 7),
            token(TokenKind::Identifier, "demo", 7, 11),
        ],
    ));
    let spaced = SyntaxTree::new(SyntaxNode::new(
        SyntaxKind::SourceFile,
        vec![
            token(TokenKind::Keyword, "module", 20, 26),
            token(TokenKind::Whitespace, "\n    ", 26, 31),
            token(TokenKind::Identifier, "demo", 31, 35),
        ],
    ));

    assert_eq!(
        compact.structural_fingerprint(),
        spaced.structural_fingerprint()
    );
}

#[test]
fn missing_tokens_and_error_nodes_mark_recovery_without_changing_source() {
    let tree = SyntaxTree::new(SyntaxNode::new(
        SyntaxKind::SourceFile,
        vec![
            SyntaxElement::Node(SyntaxNode::new(
                SyntaxKind::Error,
                vec![token(TokenKind::Invalid, "@", 0, 1)],
            )),
            token(TokenKind::Missing, "", 1, 1),
        ],
    ));

    assert_eq!(tree.source_text(), "@");
    assert!(tree.has_recovery());
}

#[test]
#[should_panic(expected = "missing tokens must be empty zero-width insertions")]
fn missing_token_constructor_enforces_the_recovery_invariant() {
    let _ = Token::new(
        TokenKind::Missing,
        ";",
        ByteSpan::new(1, 2).expect("ordered span"),
    );
}
