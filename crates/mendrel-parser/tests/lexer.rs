use mendrel_parser::lex;
use mendrel_source::SourceFile;
use mendrel_syntax::TokenKind;

fn source(path: &str, text: &str) -> SourceFile {
    SourceFile::from_bytes(path, text.as_bytes().to_vec()).expect("valid UTF-8 fixture")
}

#[test]
fn tokens_cover_the_first_slice_without_gaps_or_overlap() {
    let text = include_str!("fixtures/first_slice.mnd");
    let source = source("first_slice.mnd", text);
    let result = lex(&source);

    assert!(result.diagnostics.is_empty());
    let mut cursor = 0_u32;
    for token in result
        .tokens
        .iter()
        .filter(|token| token.kind != TokenKind::Eof)
    {
        assert_eq!(token.span.start, cursor, "gap before {token:?}");
        assert_eq!(
            &text[token.span.start as usize..token.span.end as usize],
            token.text
        );
        cursor = token.span.end;
    }
    assert_eq!(cursor as usize, text.len());
    assert_eq!(
        result.tokens.last().expect("EOF token").kind,
        TokenKind::Eof
    );
}

#[test]
fn preserves_nested_comments_as_one_lossless_token() {
    let source = source("comment.mnd", "/* outer /* nested */ done */module a;");
    let result = lex(&source);

    assert!(result.diagnostics.is_empty());
    let comments = result
        .tokens
        .iter()
        .filter(|token| token.kind == TokenKind::BlockComment)
        .collect::<Vec<_>>();
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].text, "/* outer /* nested */ done */");
}

#[test]
fn tab_outside_a_string_is_retained_and_diagnosed() {
    let source = source("tab.mnd", "module\tdemo;");
    let result = lex(&source);

    assert!(
        result
            .tokens
            .iter()
            .any(|token| token.kind == TokenKind::Invalid && token.text == "\t")
    );
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-INVALID-0001");
    assert_eq!(result.diagnostics[0].primary_span().start.byte, 6);
}

#[test]
fn arbitrary_unicode_never_panics_or_loses_text() {
    let corpus = [
        "",
        "😀",
        "αβγ",
        "module 日本語;",
        "\0\u{001f}",
        "\"unterminated",
    ];

    for (index, text) in corpus.into_iter().enumerate() {
        let source = source(&format!("unicode-{index}.mnd"), text);
        let result = lex(&source);
        let reconstructed = result
            .tokens
            .iter()
            .map(|token| token.text.as_str())
            .collect::<String>();
        assert_eq!(reconstructed, text);
    }
}
