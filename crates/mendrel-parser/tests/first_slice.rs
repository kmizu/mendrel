use mendrel_parser::parse;
use mendrel_source::SourceFile;
use mendrel_syntax::{SyntaxElement, SyntaxKind, SyntaxNode, TokenKind, production_rule};

fn source(path: &str, text: &str) -> SourceFile {
    SourceFile::from_bytes(path, text.as_bytes().to_vec()).expect("valid UTF-8 fixture")
}

fn contains_kind(node: &SyntaxNode, expected: SyntaxKind) -> bool {
    node.kind == expected
        || node.children.iter().any(|child| match child {
            SyntaxElement::Node(child) => contains_kind(child, expected),
            SyntaxElement::Token(_) => false,
        })
}

#[test]
fn parses_the_first_slice_into_a_lossless_cst() {
    let text = include_str!("fixtures/first_slice.mnd");
    let result = parse(&source("first_slice.mnd", text));

    assert!(result.diagnostics.is_empty());
    assert_eq!(result.tree.source_text(), text);
    for kind in [
        SyntaxKind::ModuleDecl,
        SyntaxKind::FunctionDecl,
        SyntaxKind::FunctionHead,
        SyntaxKind::Parameter,
        SyntaxKind::Block,
        SyntaxKind::Expression,
        SyntaxKind::AdditiveExpression,
    ] {
        assert!(contains_kind(result.tree.root(), kind), "missing {kind:?}");
    }
    assert!(!result.tree.has_recovery());
}

#[test]
fn inserts_a_missing_semicolon_without_discarding_following_source() {
    let text = include_str!("fixtures/missing_semicolon.mnd");
    let source = source("missing_semicolon.mnd", text);
    let result = parse(&source);

    assert_eq!(result.tree.source_text(), text);
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-MISSING-0001");
    assert_eq!(result.diagnostics[0].expected(), Some(";"));
    assert_eq!(
        result.diagnostics[0].workspace_revision(),
        source.revision()
    );
    assert_eq!(result.diagnostics[0].fixes().len(), 1);
    assert_eq!(
        result.diagnostics[0].fixes()[0].text_edits()[0].replacement(),
        ";"
    );
    assert_eq!(
        result.diagnostics[0].fixes()[0].text_edits()[0].expected_source_digest(),
        source.revision()
    );
    assert!(result.tree.tokens().iter().any(|token| {
        token.kind == TokenKind::Missing && token.span.start == 16 && token.span.is_empty()
    }));
    assert!(result.tree.has_recovery());
    assert!(contains_kind(result.tree.root(), SyntaxKind::FunctionDecl));
}

#[test]
fn malformed_function_boundaries_recover_locally() {
    let cases = [
        (
            "missing-brace.mnd",
            "module demo.main;\npub fn add(left: I32) -> I32 { left + left",
            "}",
        ),
        (
            "missing-type.mnd",
            "module demo.main;\npub fn add(left: ) -> I32 { left }",
            "type",
        ),
        (
            "missing-name.mnd",
            "module demo.main;\npub fn (left: I32) -> I32 { left }",
            "identifier",
        ),
    ];

    for (path, text, expected) in cases {
        let result = parse(&source(path, text));
        assert_eq!(result.tree.source_text(), text, "source loss for {path}");
        assert_eq!(
            result.diagnostics.len(),
            1,
            "{path}: {:#?}",
            result.diagnostics
        );
        assert_eq!(
            result.diagnostics[0].expected(),
            Some(expected),
            "missing local {expected} diagnostic for {path}: {:?}",
            result.diagnostics,
        );
        assert!(result.tree.has_recovery());
    }
}

#[test]
fn unsupported_top_level_syntax_becomes_an_error_node() {
    let text = "module demo.main;\nrecord User {}\n";
    let result = parse(&source("unsupported.mnd", text));

    assert_eq!(result.tree.source_text(), text);
    assert!(contains_kind(result.tree.root(), SyntaxKind::Error));
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code() == "E-SYNTAX-UNSUPPORTED-0001")
    );
}

#[test]
fn block_recovery_stops_at_the_real_closing_brace() {
    let text = concat!(
        "module demo.main;\n",
        "pub fn broken(value: I32) -> I32 { return value; }\n",
        "pub fn intact(value: I32) -> I32 { value }\n",
    );
    let result = parse(&source("block-recovery.mnd", text));

    assert_eq!(result.tree.source_text(), text);
    assert_eq!(result.diagnostics.len(), 1, "{:#?}", result.diagnostics);
    assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-UNSUPPORTED-0001");
    assert_eq!(
        result
            .tree
            .tokens()
            .iter()
            .filter(|token| token.text == "intact")
            .count(),
        1,
    );
    assert_eq!(
        result
            .tree
            .root()
            .children
            .iter()
            .filter(|child| matches!(child, SyntaxElement::Node(node) if node.kind == SyntaxKind::FunctionDecl))
            .count(),
        2,
    );
}

#[test]
fn invalid_token_inside_a_block_does_not_cascade() {
    let text = "module demo.main;\npub fn broken(value: I32) -> I32 { value @ value }\n";
    let result = parse(&source("invalid-in-block.mnd", text));

    assert_eq!(result.tree.source_text(), text);
    assert_eq!(result.diagnostics.len(), 1, "{:#?}", result.diagnostics);
    assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-UNSUPPORTED-0001");
    assert!(contains_kind(result.tree.root(), SyntaxKind::Error));
}

#[test]
fn block_recovery_balances_nested_unsupported_braces() {
    let text = concat!(
        "module demo.main;\n",
        "pub fn broken(value: I32) -> I32 { if value { value } }\n",
        "pub fn intact(value: I32) -> I32 { value }\n",
    );
    let result = parse(&source("nested-block-recovery.mnd", text));

    assert_eq!(result.tree.source_text(), text);
    assert_eq!(result.diagnostics.len(), 1, "{:#?}", result.diagnostics);
    assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-UNSUPPORTED-0001");
    assert_eq!(
        result
            .tree
            .root()
            .children
            .iter()
            .filter(|child| matches!(child, SyntaxElement::Node(node) if node.kind == SyntaxKind::FunctionDecl))
            .count(),
        2,
    );
}

#[test]
fn rejects_function_modifiers_outside_the_phase_one_subset() {
    for (name, declaration, actual) in [
        ("bare", "fn f(value: I32) -> I32 { value }", "fn"),
        (
            "internal",
            "internal fn f(value: I32) -> I32 { value }",
            "internal",
        ),
        ("async", "async fn f(value: I32) -> I32 { value }", "async"),
        (
            "unsafe",
            "unsafe fn f(value: I32) -> I32 { value }",
            "unsafe",
        ),
        (
            "public-async",
            "pub async fn f(value: I32) -> I32 { value }",
            "async",
        ),
        (
            "public-unsafe",
            "pub unsafe fn f(value: I32) -> I32 { value }",
            "unsafe",
        ),
    ] {
        let text = format!("module demo.main;\n{declaration}\n");
        let result = parse(&source(&format!("{name}.mnd"), &text));

        assert_eq!(result.tree.source_text(), text);
        assert_eq!(
            result.diagnostics.len(),
            1,
            "{name}: {:#?}",
            result.diagnostics
        );
        assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-UNSUPPORTED-0001");
        assert_eq!(result.diagnostics[0].actual(), Some(actual));
        assert!(contains_kind(result.tree.root(), SyntaxKind::Error));
    }
}

#[test]
fn requires_an_explicit_return_type_annotation() {
    let text = "module demo.main;\npub fn f(value: I32) { value }\n";
    let result = parse(&source("missing-return-type.mnd", text));

    assert_eq!(result.tree.source_text(), text);
    assert_eq!(result.diagnostics.len(), 1, "{:#?}", result.diagnostics);
    assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-MISSING-0001");
    assert_eq!(
        result.diagnostics[0].expected(),
        Some("return type annotation")
    );
}

#[test]
fn rejects_move_parameters_without_resource_semantics() {
    let text = "module demo.main;\npub fn f(move value: I32) -> I32 { value }\n";
    let result = parse(&source("move-parameter.mnd", text));

    assert_eq!(result.tree.source_text(), text);
    assert_eq!(result.diagnostics.len(), 1, "{:#?}", result.diagnostics);
    assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-UNSUPPORTED-0001");
    assert!(contains_kind(result.tree.root(), SyntaxKind::Error));
}

#[test]
fn rejects_syntax_beyond_the_addition_only_vertical_slice() {
    for (name, declaration) in [
        ("empty-parameters", "pub fn value() -> I32 { value }"),
        (
            "trailing-parameter-comma",
            "pub fn value(input: I32,) -> I32 { input }",
        ),
        ("empty-body", "pub fn value(input: I32) -> I32 {}"),
        (
            "qualified-expression",
            "pub fn value(input: I32) -> I32 { math.input }",
        ),
        (
            "parenthesized-expression",
            "pub fn value(input: I32) -> I32 { (input) }",
        ),
        (
            "unary-expression",
            "pub fn value(input: I32) -> I32 { -input }",
        ),
        (
            "subtraction",
            "pub fn value(input: I32) -> I32 { input - input }",
        ),
        (
            "multiplication",
            "pub fn value(input: I32) -> I32 { input * input }",
        ),
        (
            "division",
            "pub fn value(input: I32) -> I32 { input / input }",
        ),
        (
            "remainder",
            "pub fn value(input: I32) -> I32 { input % input }",
        ),
    ] {
        let text = format!("module demo.main;\n{declaration}\n");
        let result = parse(&source(&format!("{name}.mnd"), &text));

        assert_eq!(result.tree.source_text(), text);
        assert_eq!(
            result.diagnostics.len(),
            1,
            "{name}: {:#?}",
            result.diagnostics
        );
        assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-UNSUPPORTED-0001");
        assert!(result.tree.has_recovery());
    }
}

#[test]
fn implemented_production_shapes_are_pinned_to_the_normative_grammar() {
    for (production, expected_rule) in [
        ("module_decl", "\"module\", qualified_name, \";\""),
        ("qualified_name", "identifier, { \".\", identifier }"),
        ("function_decl", "function_head, [ contract_clause ], block"),
        ("block", "\"{\", { statement }, [ expression ], \"}\""),
        (
            "additive_expression",
            "multiplicative_expression, { ( \"+\" | \"-\" ), multiplicative_expression }",
        ),
        (
            "multiplicative_expression",
            "unary_expression, { ( \"*\" | \"/\" | \"%\" ), unary_expression }",
        ),
    ] {
        assert_eq!(
            production_rule(production),
            Some(expected_rule),
            "{production}"
        );
    }
}
