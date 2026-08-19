use mendrel_parser::parse;
use mendrel_source::SourceFile;
use mendrel_syntax::{
    SPACED_OPERATORS, SyntaxElement, SyntaxKind, SyntaxNode, TokenKind, production_rule,
};

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

fn count_kind(node: &SyntaxNode, expected: SyntaxKind) -> usize {
    usize::from(node.kind == expected)
        + node
            .children
            .iter()
            .map(|child| match child {
                SyntaxElement::Node(child) => count_kind(child, expected),
                SyntaxElement::Token(_) => 0,
            })
            .sum::<usize>()
}

fn find_kind(node: &SyntaxNode, expected: SyntaxKind) -> Option<&SyntaxNode> {
    if node.kind == expected {
        return Some(node);
    }
    node.children.iter().find_map(|child| match child {
        SyntaxElement::Node(child) => find_kind(child, expected),
        SyntaxElement::Token(_) => None,
    })
}

fn contains_zero_width_token(node: &SyntaxNode, expected: TokenKind) -> bool {
    node.children.iter().any(|child| match child {
        SyntaxElement::Node(child) => contains_zero_width_token(child, expected),
        SyntaxElement::Token(token) => token.kind == expected && token.span.is_empty(),
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
fn parses_direct_grouped_and_aliased_imports_losslessly() {
    let text = concat!(
        "module demo.main;\n",
        "import billing.money.Money;\n",
        "import billing.tax.{TaxRate, calculate_tax as compute_tax,};\n",
        "import billing.clock.Clock as BillingClock;\n",
        "pub fn value(input: I32) -> I32 { input }\n",
    );
    let result = parse(&source("imports.mnd", text));

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert_eq!(result.tree.source_text(), text);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::ImportDecl), 3);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::ImportPath), 3);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::ImportItem), 2);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::FunctionDecl), 1);
    assert!(!result.tree.has_recovery());
}

#[test]
fn inserts_a_missing_import_semicolon_without_consuming_following_items() {
    let text = concat!(
        "module demo.main;\n",
        "import billing.money.Money\n",
        "import billing.clock.Clock;\n",
        "pub fn value(input: I32) -> I32 { input }\n",
    );
    let result = parse(&source("missing-import-semicolon.mnd", text));

    assert_eq!(result.tree.source_text(), text);
    assert_eq!(result.diagnostics.len(), 1, "{:#?}", result.diagnostics);
    assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-MISSING-0001");
    assert_eq!(result.diagnostics[0].expected(), Some(";"));
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::ImportDecl), 2);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::FunctionDecl), 1);
    let first_import =
        find_kind(result.tree.root(), SyntaxKind::ImportDecl).expect("first import declaration");
    assert!(contains_zero_width_token(first_import, TokenKind::Missing));
    assert!(result.tree.has_recovery());
}

#[test]
fn inserts_a_missing_grouped_import_brace_without_consuming_the_function() {
    let text = concat!(
        "module demo.main;\n",
        "import billing.tax.{TaxRate, calculate_tax;\n",
        "pub fn value(input: I32) -> I32 { input }\n",
    );
    let result = parse(&source("missing-import-brace.mnd", text));

    assert_eq!(result.tree.source_text(), text);
    assert_eq!(result.diagnostics.len(), 1, "{:#?}", result.diagnostics);
    assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-MISSING-0001");
    assert_eq!(result.diagnostics[0].expected(), Some("}"));
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::ImportDecl), 1);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::ImportItem), 2);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::FunctionDecl), 1);
    let import =
        find_kind(result.tree.root(), SyntaxKind::ImportDecl).expect("grouped import declaration");
    assert!(contains_zero_width_token(import, TokenKind::Missing));
    assert!(result.tree.has_recovery());
}

#[test]
fn rejects_imports_after_top_level_declarations_without_losing_following_declarations() {
    let text = concat!(
        "module demo.main;\n",
        "pub fn first(input: I32) -> I32 { input }\n",
        "import billing.money.Money;\n",
        "pub fn second(input: I32) -> I32 { input }\n",
    );
    let result = parse(&source("late-import.mnd", text));

    assert_eq!(result.tree.source_text(), text);
    assert_eq!(result.diagnostics.len(), 1, "{:#?}", result.diagnostics);
    assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-UNSUPPORTED-0001");
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::ImportDecl), 0);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::FunctionDecl), 2);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::Error), 1);
    assert!(result.tree.has_recovery());
}

#[test]
fn unterminated_late_grouped_import_does_not_consume_following_declarations() {
    let text = concat!(
        "module demo.main;\n",
        "pub fn first(input: I32) -> I32 { input }\n",
        "import billing.tax.{TaxRate,\n",
        "pub fn second(input: I32) -> I32 { input }\n",
        "pub fn third(input: I32) -> I32 { input }\n",
    );
    let result = parse(&source("unterminated-late-import.mnd", text));

    assert_eq!(result.tree.source_text(), text);
    assert_eq!(result.diagnostics.len(), 1, "{:#?}", result.diagnostics);
    assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-UNSUPPORTED-0001");
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::ImportDecl), 0);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::FunctionDecl), 3);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::Error), 1);
    assert!(result.tree.has_recovery());
}

#[test]
fn parses_visible_record_declarations_and_fields_losslessly() {
    let text = concat!(
        "module demo.main;\n",
        "pub record Customer {\n",
        "    pub id: CustomerId,\n",
        "    internal name: Text,\n",
        "    email: EmailAddress,\n",
        "}\n",
        "record Empty {}\n",
        "pub fn value(input: I32) -> I32 { input }\n",
    );
    let result = parse(&source("records.mnd", text));

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert_eq!(result.tree.source_text(), text);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::RecordDecl), 2);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::RecordBody), 2);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::RecordField), 3);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::Visibility), 3);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::FunctionDecl), 1);
    assert!(!result.tree.has_recovery());
}

#[test]
fn parses_self_typed_record_fields_without_stalling() {
    let text = concat!(
        "module demo.main;\n",
        "record Link { next: Self, }\n",
        "pub fn value(input: I32) -> I32 { input }\n",
    );
    let result = parse(&source("self-record-field.mnd", text));

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert_eq!(result.tree.source_text(), text);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::RecordField), 1);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::FunctionDecl), 1);
    assert!(!result.tree.has_recovery());
}

#[test]
fn parses_visible_enum_declarations_and_payload_fields_losslessly() {
    let text = concat!(
        "module demo.main;\n",
        "pub enum PaymentState {\n",
        "    Pending,\n",
        "    Authorized { authorization_id: AuthorizationId, },\n",
        "    Declined { internal decline_reason: DeclineReason, },\n",
        "}\n",
        "enum Empty {}\n",
        "record Marker {}\n",
        "pub fn value(input: I32) -> I32 { input }\n",
    );
    let result = parse(&source("enums.mnd", text));

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert_eq!(result.tree.source_text(), text);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::EnumDecl), 2);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::EnumBody), 2);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::EnumVariant), 3);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::RecordField), 2);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::Visibility), 2);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::RecordDecl), 1);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::FunctionDecl), 1);
    assert!(!result.tree.has_recovery());
}

#[test]
fn inserts_a_missing_enum_variant_name_without_losing_following_variants() {
    let text = concat!(
        "module demo.main;\n",
        "enum Broken { , Good, }\n",
        "record Intact {}\n",
        "pub fn intact(input: I32) -> I32 { input }\n",
    );
    let result = parse(&source("missing-enum-variant-name.mnd", text));

    assert_eq!(result.tree.source_text(), text);
    assert_eq!(result.diagnostics.len(), 1, "{:#?}", result.diagnostics);
    assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-MISSING-0001");
    assert_eq!(result.diagnostics[0].expected(), Some("identifier"));
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::EnumDecl), 1);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::EnumVariant), 2);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::RecordDecl), 1);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::FunctionDecl), 1);
    let declaration = find_kind(result.tree.root(), SyntaxKind::EnumDecl).expect("enum");
    assert!(contains_zero_width_token(declaration, TokenKind::Missing));
    assert!(result.tree.has_recovery());
}

#[test]
fn missing_enum_payload_variant_name_preserves_its_fields_and_following_variant() {
    let text = concat!(
        "module demo.main;\n",
        "enum Broken { { value: I32, }, Good, }\n",
        "record Intact {}\n",
    );
    let result = parse(&source("missing-enum-payload-variant-name.mnd", text));

    assert_eq!(result.tree.source_text(), text);
    assert_eq!(result.diagnostics.len(), 1, "{:#?}", result.diagnostics);
    assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-MISSING-0001");
    assert_eq!(result.diagnostics[0].expected(), Some("identifier"));
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::EnumVariant), 2);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::RecordField), 1);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::RecordDecl), 1);
    assert!(result.tree.has_recovery());
}

#[test]
fn missing_enum_payload_boundary_does_not_consume_the_next_variant() {
    let text = concat!(
        "module demo.main;\n",
        "enum Broken { First { value: I32, Second, }\n",
        "record Intact {}\n",
        "pub fn intact(input: I32) -> I32 { input }\n",
    );
    let result = parse(&source("missing-enum-payload-boundary.mnd", text));

    assert_eq!(result.tree.source_text(), text);
    assert_eq!(
        result
            .diagnostics
            .iter()
            .map(|diagnostic| (diagnostic.code(), diagnostic.expected()))
            .collect::<Vec<_>>(),
        [
            ("E-SYNTAX-MISSING-0001", Some("}")),
            ("E-SYNTAX-MISSING-0001", Some(",")),
        ],
        "{:#?}",
        result.diagnostics,
    );
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::EnumVariant), 2);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::RecordField), 1);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::RecordDecl), 1);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::FunctionDecl), 1);
    assert!(result.tree.has_recovery());
}

#[test]
fn missing_enum_payload_opening_brace_preserves_fields_and_following_variants() {
    let text = concat!(
        "module demo.main;\n",
        "enum Broken { First value: I32, }, Second, }\n",
        "record Intact {}\n",
        "pub fn intact(input: I32) -> I32 { input }\n",
    );
    let result = parse(&source("missing-enum-payload-opening-brace.mnd", text));

    assert_eq!(result.tree.source_text(), text);
    assert_eq!(result.diagnostics.len(), 1, "{:#?}", result.diagnostics);
    assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-MISSING-0001");
    assert_eq!(result.diagnostics[0].expected(), Some("{"));
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::EnumVariant), 2);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::RecordField), 1);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::RecordDecl), 1);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::FunctionDecl), 1);
    let declaration = find_kind(result.tree.root(), SyntaxKind::EnumDecl).expect("enum");
    assert!(contains_zero_width_token(declaration, TokenKind::Missing));
    assert!(result.tree.has_recovery());
}

#[test]
fn unsupported_enum_payload_type_does_not_consume_the_next_variant() {
    let text = concat!(
        "module demo.main;\n",
        "enum Broken { First { bad: Box<T> Second, }\n",
        "record Intact {}\n",
        "pub fn intact(input: I32) -> I32 { input }\n",
    );
    let result = parse(&source("unsupported-enum-payload-type.mnd", text));

    assert_eq!(result.tree.source_text(), text);
    assert_eq!(
        result
            .diagnostics
            .iter()
            .map(|diagnostic| (diagnostic.code(), diagnostic.expected()))
            .collect::<Vec<_>>(),
        [
            ("E-SYNTAX-UNSUPPORTED-0001", None),
            ("E-SYNTAX-MISSING-0001", Some(",")),
            ("E-SYNTAX-MISSING-0001", Some("}")),
            ("E-SYNTAX-MISSING-0001", Some(",")),
        ],
        "{:#?}",
        result.diagnostics,
    );
    assert_eq!(result.diagnostics[0].actual(), Some("<"));
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::EnumVariant), 2);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::RecordField), 1);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::RecordDecl), 1);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::FunctionDecl), 1);
    assert!(result.tree.has_recovery());
}

#[test]
fn rejects_deferred_enum_header_syntax_without_cascading() {
    let cases = [
        (
            "enum-generics.mnd",
            concat!(
                "module demo.main;\n",
                "pub enum Box<T> { Value { value: T, }, }\n",
                "record Intact {}\n",
                "pub fn intact(input: I32) -> I32 { input }\n",
            ),
            "<",
        ),
        (
            "enum-where.mnd",
            concat!(
                "module demo.main;\n",
                "enum Ordered where { T: Ord } { Value, }\n",
                "record Intact {}\n",
                "pub fn intact(input: I32) -> I32 { input }\n",
            ),
            "where",
        ),
    ];

    for (path, text, actual) in cases {
        let result = parse(&source(path, text));

        assert_eq!(result.tree.source_text(), text, "source loss for {path}");
        assert_eq!(
            result.diagnostics.len(),
            1,
            "{path}: {:#?}",
            result.diagnostics
        );
        assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-UNSUPPORTED-0001");
        assert_eq!(result.diagnostics[0].actual(), Some(actual));
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::EnumDecl), 1);
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::RecordDecl), 1);
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::FunctionDecl), 1);
        let declaration = find_kind(result.tree.root(), SyntaxKind::EnumDecl).expect("enum");
        assert!(contains_kind(declaration, SyntaxKind::Error));
        assert!(!contains_zero_width_token(declaration, TokenKind::Missing));
        assert!(result.tree.has_recovery());
    }
}

#[test]
fn unsupported_enum_variant_attribute_recovers_inside_the_enum() {
    let text = concat!(
        "module demo.main;\n",
        "enum Annotated { @tag(level: 1, nested: cfg.value()) First, Second, }\n",
        "record Intact {}\n",
        "pub fn intact(input: I32) -> I32 { input }\n",
    );
    let result = parse(&source("unsupported-enum-variant-attribute.mnd", text));

    assert_eq!(result.tree.source_text(), text);
    assert_eq!(result.diagnostics.len(), 1, "{:#?}", result.diagnostics);
    assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-UNSUPPORTED-0001");
    assert_eq!(result.diagnostics[0].actual(), Some("@"));
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::EnumVariant), 2);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::RecordDecl), 1);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::FunctionDecl), 1);
    let declaration = find_kind(result.tree.root(), SyntaxKind::EnumDecl).expect("enum");
    assert!(contains_kind(declaration, SyntaxKind::Error));
    assert!(!contains_zero_width_token(declaration, TokenKind::Missing));
    assert!(result.tree.has_recovery());
}

#[test]
fn unsupported_enum_payload_field_attribute_recovers_inside_the_payload() {
    let text = concat!(
        "module demo.main;\n",
        "enum Annotated { First { @tag value: I32, }, Second, }\n",
        "record Intact {}\n",
        "pub fn intact(input: I32) -> I32 { input }\n",
    );
    let result = parse(&source("unsupported-enum-field-attribute.mnd", text));

    assert_eq!(result.tree.source_text(), text);
    assert_eq!(result.diagnostics.len(), 1, "{:#?}", result.diagnostics);
    assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-UNSUPPORTED-0001");
    assert_eq!(result.diagnostics[0].actual(), Some("@"));
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::EnumVariant), 2);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::RecordField), 1);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::RecordDecl), 1);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::FunctionDecl), 1);
    let declaration = find_kind(result.tree.root(), SyntaxKind::EnumDecl).expect("enum");
    assert!(contains_kind(declaration, SyntaxKind::Error));
    assert!(!contains_zero_width_token(declaration, TokenKind::Missing));
    assert!(result.tree.has_recovery());
}

#[test]
fn unsupported_tuple_enum_payload_recovers_inside_the_variant() {
    let text = concat!(
        "module demo.main;\n",
        "enum Unsupported { First(I32), Second, }\n",
        "record Intact {}\n",
        "pub fn intact(input: I32) -> I32 { input }\n",
    );
    let result = parse(&source("unsupported-tuple-enum-payload.mnd", text));

    assert_eq!(result.tree.source_text(), text);
    assert_eq!(result.diagnostics.len(), 1, "{:#?}", result.diagnostics);
    assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-UNSUPPORTED-0001");
    assert_eq!(result.diagnostics[0].actual(), Some("("));
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::EnumVariant), 2);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::RecordDecl), 1);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::FunctionDecl), 1);
    let declaration = find_kind(result.tree.root(), SyntaxKind::EnumDecl).expect("enum");
    assert!(contains_kind(declaration, SyntaxKind::Error));
    assert!(!contains_zero_width_token(declaration, TokenKind::Missing));
    assert!(result.tree.has_recovery());
}

#[test]
fn missing_payload_close_preserves_a_following_tuple_style_variant() {
    let text = concat!(
        "module demo.main;\n",
        "enum Broken { First { value: I32, Second(I32), Third, }\n",
        "record Intact {}\n",
        "pub fn intact(input: I32) -> I32 { input }\n",
    );
    let result = parse(&source(
        "missing-payload-close-before-tuple-variant.mnd",
        text,
    ));

    assert_eq!(result.tree.source_text(), text);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::EnumVariant), 3);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::RecordField), 1);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::RecordDecl), 1);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::FunctionDecl), 1);
    assert!(result.diagnostics.iter().any(|diagnostic| {
        diagnostic.code() == "E-SYNTAX-UNSUPPORTED-0001" && diagnostic.actual() == Some("(")
    }));
    assert!(result.tree.has_recovery());
}

#[test]
fn unsupported_field_type_preserves_a_following_missing_name_payload_variant() {
    let text = concat!(
        "module demo.main;\n",
        "enum Broken { First { bad: Box<T> { value: I32, }, Third, }\n",
        "record Intact {}\n",
        "pub fn intact(input: I32) -> I32 { input }\n",
    );
    let result = parse(&source(
        "unsupported-type-before-missing-name-payload-variant.mnd",
        text,
    ));

    assert_eq!(result.tree.source_text(), text);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::EnumVariant), 3);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::RecordField), 2);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::RecordDecl), 1);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::FunctionDecl), 1);
    assert!(result.diagnostics.iter().any(|diagnostic| {
        diagnostic.code() == "E-SYNTAX-UNSUPPORTED-0001" && diagnostic.actual() == Some("<")
    }));
    assert!(result.diagnostics.iter().any(|diagnostic| {
        diagnostic.code() == "E-SYNTAX-MISSING-0001" && diagnostic.expected() == Some("identifier")
    }));
    assert!(result.tree.has_recovery());
}

#[test]
fn effect_row_braces_remain_part_of_an_unsupported_enum_payload_field_type() {
    let text = concat!(
        "module demo.main;\n",
        "enum Broken { First { bad: fn(I32) -> I32 uses { io: Unit }, good: Text, }, Second, }\n",
        "record Intact {}\n",
        "pub fn intact(input: I32) -> I32 { input }\n",
    );
    let result = parse(&source("effect-row-braces-in-enum-payload.mnd", text));

    assert_eq!(result.tree.source_text(), text);
    assert_eq!(result.diagnostics.len(), 1, "{:#?}", result.diagnostics);
    assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-UNSUPPORTED-0001");
    assert_eq!(result.diagnostics[0].actual(), Some("fn"));
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::EnumVariant), 2);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::RecordField), 2);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::RecordDecl), 1);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::FunctionDecl), 1);
    let declaration = find_kind(result.tree.root(), SyntaxKind::EnumDecl).expect("enum");
    assert!(!contains_zero_width_token(declaration, TokenKind::Missing));
    assert!(result.tree.has_recovery());
}

#[test]
fn unsupported_enum_body_region_recovers_before_the_next_variant() {
    let text = concat!(
        "module demo.main;\n",
        "enum Unsupported { (I32), Second, }\n",
        "record Intact {}\n",
        "pub fn intact(input: I32) -> I32 { input }\n",
    );
    let result = parse(&source("unsupported-enum-body-region.mnd", text));

    assert_eq!(result.tree.source_text(), text);
    assert_eq!(result.diagnostics.len(), 1, "{:#?}", result.diagnostics);
    assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-UNSUPPORTED-0001");
    assert_eq!(result.diagnostics[0].actual(), Some("("));
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::EnumVariant), 1);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::RecordDecl), 1);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::FunctionDecl), 1);
    let declaration = find_kind(result.tree.root(), SyntaxKind::EnumDecl).expect("enum");
    assert!(contains_kind(declaration, SyntaxKind::Error));
    assert!(!contains_zero_width_token(declaration, TokenKind::Missing));
    assert!(result.tree.has_recovery());
}

#[test]
fn unsupported_enum_payload_region_recovers_before_the_next_field() {
    let text = concat!(
        "module demo.main;\n",
        "enum Unsupported { First { (I32), value: Text, }, Second, }\n",
        "record Intact {}\n",
        "pub fn intact(input: I32) -> I32 { input }\n",
    );
    let result = parse(&source("unsupported-enum-payload-region.mnd", text));

    assert_eq!(result.tree.source_text(), text);
    assert_eq!(result.diagnostics.len(), 1, "{:#?}", result.diagnostics);
    assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-UNSUPPORTED-0001");
    assert_eq!(result.diagnostics[0].actual(), Some("("));
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::EnumVariant), 2);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::RecordField), 1);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::RecordDecl), 1);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::FunctionDecl), 1);
    let declaration = find_kind(result.tree.root(), SyntaxKind::EnumDecl).expect("enum");
    assert!(contains_kind(declaration, SyntaxKind::Error));
    assert!(!contains_zero_width_token(declaration, TokenKind::Missing));
    assert!(result.tree.has_recovery());
}

#[test]
fn unterminated_unsupported_enum_region_stops_at_the_enum_boundary() {
    let text = concat!(
        "module demo.main;\n",
        "enum Broken { (I32, }\n",
        "record Intact {}\n",
        "pub fn intact(input: I32) -> I32 { input }\n",
    );
    let result = parse(&source("unterminated-unsupported-enum-region.mnd", text));

    assert_eq!(result.tree.source_text(), text);
    assert_eq!(result.diagnostics.len(), 1, "{:#?}", result.diagnostics);
    assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-UNSUPPORTED-0001");
    assert_eq!(result.diagnostics[0].actual(), Some("("));
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::EnumDecl), 1);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::RecordDecl), 1);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::FunctionDecl), 1);
    let declaration = find_kind(result.tree.root(), SyntaxKind::EnumDecl).expect("enum");
    assert!(contains_kind(declaration, SyntaxKind::Error));
    assert!(!contains_zero_width_token(declaration, TokenKind::Missing));
    assert!(result.tree.has_recovery());
}

#[test]
fn unterminated_unsupported_payload_region_stops_at_the_payload_boundary() {
    let text = concat!(
        "module demo.main;\n",
        "enum Broken { First { (I32, }, Second, }\n",
        "record Intact {}\n",
        "pub fn intact(input: I32) -> I32 { input }\n",
    );
    let result = parse(&source(
        "unterminated-unsupported-enum-payload-region.mnd",
        text,
    ));

    assert_eq!(result.tree.source_text(), text);
    assert_eq!(result.diagnostics.len(), 1, "{:#?}", result.diagnostics);
    assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-UNSUPPORTED-0001");
    assert_eq!(result.diagnostics[0].actual(), Some("("));
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::EnumVariant), 2);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::RecordDecl), 1);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::FunctionDecl), 1);
    let declaration = find_kind(result.tree.root(), SyntaxKind::EnumDecl).expect("enum");
    assert!(contains_kind(declaration, SyntaxKind::Error));
    assert!(!contains_zero_width_token(declaration, TokenKind::Missing));
    assert!(result.tree.has_recovery());
}

#[test]
fn malformed_enum_boundaries_recover_locally() {
    let cases = [
        (
            "missing-enum-name.mnd",
            concat!(
                "module demo.main;\n",
                "enum { First, }\n",
                "record Intact {}\n",
                "pub fn intact(input: I32) -> I32 { input }\n",
            ),
            "identifier",
            1,
        ),
        (
            "missing-enum-body-open.mnd",
            concat!(
                "module demo.main;\n",
                "enum Broken First, }\n",
                "record Intact {}\n",
                "pub fn intact(input: I32) -> I32 { input }\n",
            ),
            "{",
            1,
        ),
        (
            "missing-enum-variant-comma.mnd",
            concat!(
                "module demo.main;\n",
                "enum Broken { First Second, }\n",
                "record Intact {}\n",
                "pub fn intact(input: I32) -> I32 { input }\n",
            ),
            ",",
            2,
        ),
        (
            "missing-enum-body-close.mnd",
            concat!(
                "module demo.main;\n",
                "enum Broken { First,\n",
                "record Intact {}\n",
                "pub fn intact(input: I32) -> I32 { input }\n",
            ),
            "}",
            1,
        ),
    ];

    for (path, text, expected, variant_count) in cases {
        let result = parse(&source(path, text));

        assert_eq!(result.tree.source_text(), text, "source loss for {path}");
        assert_eq!(
            result.diagnostics.len(),
            1,
            "{path}: {:#?}",
            result.diagnostics
        );
        assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-MISSING-0001");
        assert_eq!(result.diagnostics[0].expected(), Some(expected));
        assert_eq!(
            count_kind(result.tree.root(), SyntaxKind::EnumVariant),
            variant_count,
            "variant loss for {path}",
        );
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::RecordDecl), 1);
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::FunctionDecl), 1);
        let declaration = find_kind(result.tree.root(), SyntaxKind::EnumDecl).expect("enum");
        assert!(contains_zero_width_token(declaration, TokenKind::Missing));
        assert!(result.tree.has_recovery());
    }
}

#[test]
fn malformed_enum_payload_fields_recover_locally() {
    let cases = [
        (
            "missing-enum-field-name.mnd",
            "enum Broken { First { : I32, good: Text, }, Second, }",
            "identifier",
        ),
        (
            "missing-enum-field-type.mnd",
            "enum Broken { First { missing: , good: Text, }, Second, }",
            "type",
        ),
        (
            "missing-enum-field-comma.mnd",
            "enum Broken { First { first: I32 second: I32, }, Second, }",
            ",",
        ),
    ];

    for (path, declaration, expected) in cases {
        let text = format!(
            "module demo.main;\n{declaration}\nrecord Intact {{}}\npub fn intact(input: I32) -> I32 {{ input }}\n"
        );
        let result = parse(&source(path, &text));

        assert_eq!(result.tree.source_text(), text, "source loss for {path}");
        assert_eq!(
            result.diagnostics.len(),
            1,
            "{path}: {:#?}",
            result.diagnostics
        );
        assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-MISSING-0001");
        assert_eq!(result.diagnostics[0].expected(), Some(expected));
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::EnumVariant), 2);
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::RecordField), 2);
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::RecordDecl), 1);
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::FunctionDecl), 1);
        let declaration = find_kind(result.tree.root(), SyntaxKind::EnumDecl).expect("enum");
        assert!(contains_zero_width_token(declaration, TokenKind::Missing));
        assert!(result.tree.has_recovery());
    }
}

#[test]
fn rejects_deferred_record_header_syntax_without_cascading() {
    let cases = [
        (
            "record-generics.mnd",
            concat!(
                "module demo.main;\n",
                "pub record Box<T> { value: T, }\n",
                "pub fn value(input: I32) -> I32 { input }\n",
            ),
            "<",
        ),
        (
            "record-where.mnd",
            concat!(
                "module demo.main;\n",
                "record Ordered where { T: Ord } { value: T, }\n",
                "pub fn value(input: I32) -> I32 { input }\n",
            ),
            "where",
        ),
    ];

    for (path, text, actual) in cases {
        let result = parse(&source(path, text));

        assert_eq!(result.tree.source_text(), text, "source loss for {path}");
        assert_eq!(
            result.diagnostics.len(),
            1,
            "{path}: {:#?}",
            result.diagnostics
        );
        assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-UNSUPPORTED-0001");
        assert_eq!(result.diagnostics[0].actual(), Some(actual));
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::RecordDecl), 1);
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::FunctionDecl), 1);
        let record = find_kind(result.tree.root(), SyntaxKind::RecordDecl).expect("record");
        assert!(contains_kind(record, SyntaxKind::Error));
        assert!(
            !contains_zero_width_token(record, TokenKind::Missing),
            "unexpected missing token for {path}",
        );
        assert!(result.tree.has_recovery());
    }
}

#[test]
fn rejects_deferred_record_field_types_without_cascading() {
    let cases = [
        ("generic", "Box<T>", "<"),
        ("nested-generic", "Outer<Inner<T>>", "<"),
        ("tuple", "(I32, I32)", "("),
        ("reference", "&T", "&"),
        ("function", "fn(I32) -> I32", "fn"),
        (
            "effectful-function",
            "fn(I32) -> I32 uses { io: Unit }",
            "fn",
        ),
    ];

    for (case, unsupported_type, actual) in cases {
        let text = format!(
            concat!(
                "module demo.main;\n",
                "record Broken {{ value: {}, good: Text, }}\n",
                "record Intact {{}}\n",
                "pub fn intact(input: I32) -> I32 {{ input }}\n",
            ),
            unsupported_type,
        );
        let result = parse(&source(&format!("unsupported-{case}-type.mnd"), &text));

        assert_eq!(result.tree.source_text(), text, "source loss for {case}");
        assert_eq!(
            result.diagnostics.len(),
            1,
            "{case}: {:#?}",
            result.diagnostics
        );
        assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-UNSUPPORTED-0001");
        assert_eq!(result.diagnostics[0].actual(), Some(actual));
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::RecordDecl), 2);
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::RecordField), 2);
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::FunctionDecl), 1);
        let record = find_kind(result.tree.root(), SyntaxKind::RecordDecl).expect("record");
        assert!(contains_kind(record, SyntaxKind::Error));
        assert!(
            !contains_zero_width_token(record, TokenKind::Missing),
            "unexpected missing token for {case}",
        );
        assert!(result.tree.has_recovery());
    }
}

#[test]
fn preserves_the_next_field_after_an_unsupported_type_and_missing_comma() {
    let cases = [
        ("generic", "Box<T>", "good: Text"),
        ("tuple", "(I32, I32)", "pub good: Text"),
    ];

    for (case, unsupported_type, next_field) in cases {
        let text = format!(
            concat!(
                "module demo.main;\n",
                "record Broken {{ bad: {} {}, }}\n",
                "record Intact {{}}\n",
                "pub fn intact(input: I32) -> I32 {{ input }}\n",
            ),
            unsupported_type, next_field,
        );
        let result = parse(&source(
            &format!("unsupported-{case}-type-missing-comma.mnd"),
            &text,
        ));

        assert_eq!(result.tree.source_text(), text, "source loss for {case}");
        assert_eq!(
            result
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code())
                .collect::<Vec<_>>(),
            ["E-SYNTAX-UNSUPPORTED-0001", "E-SYNTAX-MISSING-0001"],
            "{case}: {:#?}",
            result.diagnostics,
        );
        assert_eq!(result.diagnostics[1].expected(), Some(","));
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::RecordDecl), 2);
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::RecordField), 2);
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::FunctionDecl), 1);
        let record = find_kind(result.tree.root(), SyntaxKind::RecordDecl).expect("record");
        assert!(contains_kind(record, SyntaxKind::Error));
        assert!(contains_zero_width_token(record, TokenKind::Missing));
        assert!(result.tree.has_recovery());
    }
}

#[test]
fn unterminated_unsupported_field_type_stops_at_the_record_boundary() {
    let text = concat!(
        "module demo.main;\n",
        "record Broken { bad: Box<T, }\n",
        "record Intact {}\n",
        "pub fn intact(input: I32) -> I32 { input }\n",
    );
    let result = parse(&source("unterminated-record-field-type.mnd", text));

    assert_eq!(result.tree.source_text(), text);
    assert_eq!(
        result
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code())
            .collect::<Vec<_>>(),
        ["E-SYNTAX-UNSUPPORTED-0001", "E-SYNTAX-MISSING-0001"],
        "{:#?}",
        result.diagnostics,
    );
    assert_eq!(result.diagnostics[1].expected(), Some(","));
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::RecordDecl), 2);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::RecordField), 1);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::FunctionDecl), 1);
    assert!(result.tree.has_recovery());
}

#[test]
fn malformed_record_boundaries_recover_locally() {
    let cases = [
        (
            "missing-record-field-name.mnd",
            concat!(
                "module demo.main;\n",
                "pub record Broken { : CustomerId, good: Text, }\n",
                "pub fn intact(input: I32) -> I32 { input }\n",
            ),
            "identifier",
            2,
        ),
        (
            "missing-record-field-type.mnd",
            concat!(
                "module demo.main;\n",
                "pub record Broken { missing: , good: Text, }\n",
                "pub fn intact(input: I32) -> I32 { input }\n",
            ),
            "type",
            2,
        ),
        (
            "missing-record-field-comma.mnd",
            concat!(
                "module demo.main;\n",
                "pub record Broken { first: I32 second: I32, }\n",
                "pub fn intact(input: I32) -> I32 { input }\n",
            ),
            ",",
            2,
        ),
        (
            "missing-record-brace.mnd",
            concat!(
                "module demo.main;\n",
                "pub record Broken { first: I32,\n",
                "pub fn intact(input: I32) -> I32 { input }\n",
            ),
            "}",
            1,
        ),
    ];

    for (path, text, expected, field_count) in cases {
        let result = parse(&source(path, text));

        assert_eq!(result.tree.source_text(), text, "source loss for {path}");
        assert_eq!(
            result.diagnostics.len(),
            1,
            "{path}: {:#?}",
            result.diagnostics
        );
        assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-MISSING-0001");
        assert_eq!(result.diagnostics[0].expected(), Some(expected));
        assert_eq!(
            count_kind(result.tree.root(), SyntaxKind::RecordField),
            field_count,
            "field loss for {path}",
        );
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::FunctionDecl), 1);
        let record = find_kind(result.tree.root(), SyntaxKind::RecordDecl).expect("record");
        assert!(contains_zero_width_token(record, TokenKind::Missing));
        assert!(result.tree.has_recovery());
    }
}

#[test]
fn parses_nested_parenthesized_expressions_without_losing_source() {
    let text = concat!(
        "module demo.main;\n",
        "pub fn grouped(left: I32, right: I32) -> I32 { (left + (right)) }\n",
    );
    let result = parse(&source("parenthesized.mnd", text));

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert_eq!(result.tree.source_text(), text);
    assert_eq!(
        count_kind(result.tree.root(), SyntaxKind::ParenthesizedExpression),
        2,
    );
    assert!(!result.tree.has_recovery());
}

#[test]
fn parses_specified_literal_primary_expressions() {
    let text = concat!(
        "module demo.main;\n",
        "pub fn integer(value: I32) -> I32 { 1 }\n",
        "pub fn string(value: I32) -> I32 { \"value\" }\n",
        "pub fn boolean_true(value: I32) -> I32 { true }\n",
        "pub fn boolean_false(value: I32) -> I32 { false }\n",
        "pub fn unit(value: I32) -> I32 { Unit }\n",
        "pub fn none(value: I32) -> I32 { None }\n",
    );
    let result = parse(&source("literals.mnd", text));

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert_eq!(result.tree.source_text(), text);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::Literal), 6);
    assert_eq!(
        count_kind(result.tree.root(), SyntaxKind::IntegerLiteral),
        1
    );
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::StringLiteral), 1);
    assert!(!result.tree.has_recovery());
}

#[test]
fn rejects_string_escapes_until_the_escape_set_is_specified() {
    let text = concat!(
        "module demo.main;\n",
        "pub fn string(value: I32) -> I32 { \"line\\\\n\" }\n",
    );
    let result = parse(&source("string-escape.mnd", text));

    assert_eq!(result.tree.source_text(), text);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::Literal), 1);
    let literal = find_kind(result.tree.root(), SyntaxKind::Literal).expect("literal node");
    assert!(contains_kind(literal, SyntaxKind::Error));
    assert!(
        !result
            .tree
            .tokens()
            .iter()
            .any(|token| token.kind == TokenKind::Missing)
    );
    assert_eq!(result.diagnostics.len(), 1, "{:#?}", result.diagnostics);
    assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-UNSUPPORTED-0001");
    assert_eq!(result.diagnostics[0].actual(), Some("\"line\\\\n\""));
    assert!(result.tree.has_recovery());
}

#[test]
fn parses_chained_calls_with_named_arguments_and_a_trailing_comma() {
    let text = concat!(
        "module demo.main;\n",
        "pub fn invoke(value: I32) -> I32 { apply()(value, named: (value + value),) }\n",
    );
    let result = parse(&source("calls.mnd", text));

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert_eq!(result.tree.source_text(), text);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::CallSuffix), 2);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::PostfixSuffix), 2);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::Argument), 2);
    assert!(!result.tree.has_recovery());
}

#[test]
fn parses_multiplicative_operators_more_tightly_than_addition() {
    let text = concat!(
        "module demo.main;\n",
        "pub fn calculate(left: I32, right: I32, value: I32) -> I32 { ",
        "left + right * value / left % right }\n",
    );
    let result = parse(&source("multiplicative.mnd", text));

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert_eq!(result.tree.source_text(), text);
    let additive =
        find_kind(result.tree.root(), SyntaxKind::AdditiveExpression).expect("additive expression");
    let multiplicative = additive
        .children
        .iter()
        .filter_map(|child| match child {
            SyntaxElement::Node(node) if node.kind == SyntaxKind::MultiplicativeExpression => {
                Some(node)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(multiplicative.len(), 2);
    assert_eq!(
        multiplicative[1]
            .children
            .iter()
            .filter(|child| matches!(child, SyntaxElement::Node(node) if node.kind == SyntaxKind::UnaryExpression))
            .count(),
        4,
    );
    assert_eq!(
        multiplicative[1]
            .children
            .iter()
            .filter_map(|child| match child {
                SyntaxElement::Token(token) if token.kind == TokenKind::Punctuation => {
                    Some(token.text.as_str())
                }
                _ => None,
            })
            .collect::<Vec<_>>(),
        ["*", "/", "%"],
    );
    assert!(!result.tree.has_recovery());
}

#[test]
fn parses_addition_and_subtraction_in_source_order() {
    let text = concat!(
        "module demo.main;\n",
        "pub fn calculate(left: I32, right: I32, value: I32) -> I32 { ",
        "left - right * value + left - right }\n",
    );
    let result = parse(&source("additive.mnd", text));

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert_eq!(result.tree.source_text(), text);
    let additive =
        find_kind(result.tree.root(), SyntaxKind::AdditiveExpression).expect("additive expression");
    let operands = additive
        .children
        .iter()
        .filter_map(|child| match child {
            SyntaxElement::Node(node) if node.kind == SyntaxKind::MultiplicativeExpression => {
                Some(node)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(operands.len(), 4);
    assert_eq!(
        additive
            .children
            .iter()
            .filter_map(|child| match child {
                SyntaxElement::Token(token) if token.kind == TokenKind::Punctuation => {
                    Some(token.text.as_str())
                }
                _ => None,
            })
            .collect::<Vec<_>>(),
        ["-", "+", "-"],
    );
    assert_eq!(
        operands[1]
            .children
            .iter()
            .filter_map(|child| match child {
                SyntaxElement::Token(token) if token.kind == TokenKind::Punctuation => {
                    Some(token.text.as_str())
                }
                _ => None,
            })
            .collect::<Vec<_>>(),
        ["*"],
    );
    assert!(!result.tree.has_recovery());
}

#[test]
fn retains_unsupported_unary_minus_after_a_binary_operator() {
    for (name, expression) in [
        ("subtraction", "left - -right"),
        ("addition", "left + -right"),
    ] {
        let text = format!(
            "module demo.main;\npub fn calculate(left: I32, right: I32) -> I32 {{ {expression} }}\n"
        );
        let result = parse(&source(&format!("{name}-unary-minus.mnd"), &text));

        assert_eq!(result.tree.source_text(), text);
        assert_eq!(
            result.diagnostics.len(),
            1,
            "{name}: {:#?}",
            result.diagnostics
        );
        assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-UNSUPPORTED-0001");
        assert_eq!(result.diagnostics[0].actual(), Some("-"));
        let additive = find_kind(result.tree.root(), SyntaxKind::AdditiveExpression)
            .expect("additive expression");
        let operands = additive
            .children
            .iter()
            .filter_map(|child| match child {
                SyntaxElement::Node(node) if node.kind == SyntaxKind::MultiplicativeExpression => {
                    Some(node)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(operands.len(), 2);
        assert!(contains_kind(operands[1], SyntaxKind::Error));
        assert!(!contains_zero_width_token(operands[1], TokenKind::Missing));
        assert!(result.tree.has_recovery());
    }
}

#[test]
fn inserts_a_missing_subtraction_operand_without_consuming_the_block_boundary() {
    let text = concat!(
        "module demo.main;\n",
        "pub fn broken(value: I32) -> I32 { value - }\n",
        "pub fn intact(value: I32) -> I32 { value }\n",
    );
    let result = parse(&source("missing-subtraction-operand.mnd", text));

    assert_eq!(result.tree.source_text(), text);
    assert_eq!(result.diagnostics.len(), 1, "{:#?}", result.diagnostics);
    assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-MISSING-0001");
    assert_eq!(result.diagnostics[0].expected(), Some("expression"));
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::FunctionDecl), 2);
    let additive = find_kind(result.tree.root(), SyntaxKind::AdditiveExpression)
        .expect("broken additive expression");
    assert!(contains_zero_width_token(additive, TokenKind::Missing));
    assert!(result.tree.has_recovery());
}

#[test]
fn inserts_a_missing_multiplicative_operand_without_consuming_the_block_boundary() {
    let text = concat!(
        "module demo.main;\n",
        "pub fn broken(value: I32) -> I32 { value * }\n",
        "pub fn intact(value: I32) -> I32 { value }\n",
    );
    let result = parse(&source("missing-multiplicative-operand.mnd", text));

    assert_eq!(result.tree.source_text(), text);
    assert_eq!(result.diagnostics.len(), 1, "{:#?}", result.diagnostics);
    assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-MISSING-0001");
    assert_eq!(result.diagnostics[0].expected(), Some("expression"));
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::FunctionDecl), 2);
    let multiplicative = find_kind(result.tree.root(), SyntaxKind::MultiplicativeExpression)
        .expect("broken multiplicative expression");
    assert!(contains_zero_width_token(
        multiplicative,
        TokenKind::Missing,
    ));
    assert!(result.tree.has_recovery());
}

#[test]
fn inserts_a_missing_call_parenthesis_without_consuming_the_block_boundary() {
    let text = concat!(
        "module demo.main;\n",
        "pub fn broken(value: I32) -> I32 { apply(value }\n",
        "pub fn intact(value: I32) -> I32 { value }\n",
    );
    let result = parse(&source("missing-call-parenthesis.mnd", text));

    assert_eq!(result.tree.source_text(), text);
    assert_eq!(result.diagnostics.len(), 1, "{:#?}", result.diagnostics);
    assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-MISSING-0001");
    assert_eq!(result.diagnostics[0].expected(), Some(")"));
    assert_eq!(result.diagnostics[0].fixes().len(), 1);
    assert_eq!(
        result.diagnostics[0].fixes()[0].text_edits()[0].replacement(),
        ")",
    );
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::FunctionDecl), 2);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::CallSuffix), 1);
    assert!(result.tree.has_recovery());
}

#[test]
fn inserts_a_missing_closing_parenthesis_without_consuming_the_block_boundary() {
    let text = concat!(
        "module demo.main;\n",
        "pub fn broken(value: I32) -> I32 { (value + value }\n",
        "pub fn intact(value: I32) -> I32 { value }\n",
    );
    let result = parse(&source("missing-parenthesis.mnd", text));

    assert_eq!(result.tree.source_text(), text);
    assert_eq!(result.diagnostics.len(), 1, "{:#?}", result.diagnostics);
    assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-MISSING-0001");
    assert_eq!(result.diagnostics[0].expected(), Some(")"));
    assert_eq!(result.diagnostics[0].fixes().len(), 1);
    assert_eq!(
        result.diagnostics[0].fixes()[0].text_edits()[0].replacement(),
        ")",
    );
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::FunctionDecl), 2);
    assert_eq!(
        count_kind(result.tree.root(), SyntaxKind::ParenthesizedExpression),
        1,
    );
    assert!(result.tree.has_recovery());
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
    let text = "module demo.main;\ntrait User {}\n";
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
        "pub fn broken(value: I32) -> I32 { while value { value } }\n",
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
fn parses_return_statements_with_and_without_values() {
    let text = concat!(
        "module demo.main;\n",
        "pub fn with_value(input: I32) -> I32 {\n",
        "    return input;\n",
        "}\n",
        "pub fn without_value(input: I32) -> I32 {\n",
        "    return;\n",
        "}\n",
    );
    let result = parse(&source("return-statements.mnd", text));

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert_eq!(result.tree.source_text(), text);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::Statement), 2);
    assert_eq!(
        count_kind(result.tree.root(), SyntaxKind::ReturnStatement),
        2
    );
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::Expression), 1);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::FunctionDecl), 2);
    assert!(!result.tree.has_recovery());
}

#[test]
fn parses_let_and_return_statements_in_source_order_before_a_tail_expression() {
    let text = concat!(
        "module demo.main;\n",
        "pub fn value(input: I32) -> I32 {\n",
        "    let first = input;\n",
        "    return first;\n",
        "    let last = input;\n",
        "    last\n",
        "}\n",
    );
    let result = parse(&source("mixed-statements.mnd", text));

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert_eq!(result.tree.source_text(), text);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::Statement), 3);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::LetStatement), 2);
    assert_eq!(
        count_kind(result.tree.root(), SyntaxKind::ReturnStatement),
        1
    );
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::Expression), 4);
    assert!(!result.tree.has_recovery());
}

#[test]
fn unsupported_return_expression_recovers_before_following_statements() {
    let text = concat!(
        "module demo.main;\n",
        "pub fn value(input: I32) -> I32 {\n",
        "    return -input;\n",
        "    let next = input;\n",
        "    next\n",
        "}\n",
    );
    let result = parse(&source("unsupported-return-expression.mnd", text));

    assert_eq!(result.tree.source_text(), text);
    assert_eq!(result.diagnostics.len(), 1, "{:#?}", result.diagnostics);
    assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-UNSUPPORTED-0001");
    assert_eq!(result.diagnostics[0].actual(), Some("-"));
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::Statement), 2);
    assert_eq!(
        count_kind(result.tree.root(), SyntaxKind::ReturnStatement),
        1
    );
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::LetStatement), 1);
    let return_statement =
        find_kind(result.tree.root(), SyntaxKind::ReturnStatement).expect("return statement");
    assert!(contains_kind(return_statement, SyntaxKind::Error));
    assert!(!contains_zero_width_token(
        result.tree.root(),
        TokenKind::Missing
    ));
    assert!(result.tree.has_recovery());
}

#[test]
fn unsupported_return_expression_suffix_recovers_before_following_statements() {
    let text = concat!(
        "module demo.main;\n",
        "pub fn value(input: I32) -> I32 {\n",
        "    return input.member;\n",
        "    let next = input;\n",
        "    next\n",
        "}\n",
    );
    let result = parse(&source("unsupported-return-expression-suffix.mnd", text));

    assert_eq!(result.tree.source_text(), text);
    assert_eq!(result.diagnostics.len(), 1, "{:#?}", result.diagnostics);
    assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-UNSUPPORTED-0001");
    assert_eq!(result.diagnostics[0].actual(), Some("."));
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::Statement), 2);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::LetStatement), 1);
    let return_statement =
        find_kind(result.tree.root(), SyntaxKind::ReturnStatement).expect("return statement");
    assert!(contains_kind(return_statement, SyntaxKind::Expression));
    assert!(contains_kind(return_statement, SyntaxKind::Error));
    assert!(!contains_zero_width_token(
        result.tree.root(),
        TokenKind::Missing
    ));
    assert!(result.tree.has_recovery());
}

#[test]
fn unsupported_tokens_inside_return_expressions_recover_without_cascading() {
    for (case, expression) in [
        ("call-argument", "call(while)"),
        ("binary-operand", "input + while"),
    ] {
        let text = format!(
            concat!(
                "module demo.main;\n",
                "pub fn value(input: I32) -> I32 {{\n",
                "    return {};\n",
                "    let next = input;\n",
                "    next\n",
                "}}\n",
            ),
            expression,
        );
        let result = parse(&source(&format!("unsupported-return-{case}.mnd"), &text));

        assert_eq!(result.tree.source_text(), text, "source loss for {case}");
        assert_eq!(
            result.diagnostics.len(),
            1,
            "{case}: {:#?}",
            result.diagnostics
        );
        assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-UNSUPPORTED-0001");
        assert_eq!(result.diagnostics[0].actual(), Some("while"));
        assert_eq!(
            count_kind(result.tree.root(), SyntaxKind::Statement),
            2,
            "statement loss for {case}",
        );
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::LetStatement), 1);
        let return_statement =
            find_kind(result.tree.root(), SyntaxKind::ReturnStatement).expect("return statement");
        assert!(contains_kind(return_statement, SyntaxKind::Error));
        assert!(!contains_zero_width_token(
            result.tree.root(),
            TokenKind::Missing
        ));
        assert!(result.tree.has_recovery());
    }
}

#[test]
fn adjacent_return_expression_suffixes_recover_through_their_semicolons() {
    for (case, suffix, actual) in [("identifier", "value", "value"), ("literal", "42", "42")] {
        let text = format!(
            concat!(
                "module demo.main;\n",
                "pub fn value(input: I32) -> I32 {{\n",
                "    return input {};\n",
                "    let next = input;\n",
                "    next\n",
                "}}\n",
            ),
            suffix,
        );
        let result = parse(&source(
            &format!("adjacent-return-expression-suffix-{case}.mnd"),
            &text,
        ));

        assert_eq!(result.tree.source_text(), text, "source loss for {case}");
        assert_eq!(
            result.diagnostics.len(),
            1,
            "{case}: {:#?}",
            result.diagnostics
        );
        assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-UNSUPPORTED-0001");
        assert_eq!(result.diagnostics[0].actual(), Some(actual));
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::Statement), 2);
        assert_eq!(
            count_kind(result.tree.root(), SyntaxKind::ReturnStatement),
            1
        );
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::LetStatement), 1);
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::Expression), 3);
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::Error), 1);
        assert!(!contains_zero_width_token(
            result.tree.root(),
            TokenKind::Missing
        ));
        assert!(result.tree.has_recovery());
    }
}

#[test]
fn multiline_call_suffixes_stay_inside_return_recovery() {
    for (case, return_expression, actual, expression_count) in [
        (
            "supported-prefix",
            "input value\n        (input)",
            "value",
            3,
        ),
        (
            "unsupported-start",
            "while value\n        (input)",
            "while",
            2,
        ),
    ] {
        let text = format!(
            concat!(
                "module demo.main;\n",
                "pub fn value(input: I32) -> I32 {{\n",
                "    return {};\n",
                "    let next = input;\n",
                "    next\n",
                "}}\n",
            ),
            return_expression,
        );
        let result = parse(&source(
            &format!("multiline-call-return-recovery-{case}.mnd"),
            &text,
        ));

        assert_eq!(result.tree.source_text(), text, "source loss for {case}");
        assert_eq!(
            result.diagnostics.len(),
            1,
            "{case}: {:#?}",
            result.diagnostics
        );
        assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-UNSUPPORTED-0001");
        assert_eq!(result.diagnostics[0].actual(), Some(actual));
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::Statement), 2);
        assert_eq!(
            count_kind(result.tree.root(), SyntaxKind::ReturnStatement),
            1
        );
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::LetStatement), 1);
        assert_eq!(
            count_kind(result.tree.root(), SyntaxKind::Expression),
            expression_count,
            "expression loss for {case}",
        );
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::Error), 1);
        assert!(!contains_zero_width_token(
            result.tree.root(),
            TokenKind::Missing
        ));
        assert!(result.tree.has_recovery());
    }
}

#[test]
fn multiline_unsupported_groups_with_semicolons_stay_inside_return_recovery() {
    for (case, return_expression, expression_count) in [
        ("supported-prefix", "input while\n        (input)", 3),
        ("unsupported-start", "while\n        (input)", 2),
    ] {
        let text = format!(
            concat!(
                "module demo.main;\n",
                "pub fn value(input: I32) -> I32 {{\n",
                "    return {};\n",
                "    let next = input;\n",
                "    next\n",
                "}}\n",
            ),
            return_expression,
        );
        let result = parse(&source(
            &format!("multiline-unsupported-return-group-{case}.mnd"),
            &text,
        ));

        assert_eq!(result.tree.source_text(), text, "source loss for {case}");
        assert_eq!(
            result.diagnostics.len(),
            1,
            "{case}: {:#?}",
            result.diagnostics
        );
        assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-UNSUPPORTED-0001");
        assert_eq!(result.diagnostics[0].actual(), Some("while"));
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::Statement), 2);
        assert_eq!(
            count_kind(result.tree.root(), SyntaxKind::ReturnStatement),
            1
        );
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::LetStatement), 1);
        assert_eq!(
            count_kind(result.tree.root(), SyntaxKind::Expression),
            expression_count,
            "expression loss for {case}",
        );
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::Error), 1);
        assert!(!contains_zero_width_token(
            result.tree.root(),
            TokenKind::Missing
        ));
        assert!(result.tree.has_recovery());
    }
}

#[test]
fn multiline_group_without_a_return_semicolon_remains_the_block_tail() {
    let text = concat!(
        "module demo.main;\n",
        "pub fn value(input: I32) -> I32 {\n",
        "    return @\n",
        "        (input)\n",
        "}\n",
    );
    let result = parse(&source(
        "missing-return-semicolon-before-group-tail.mnd",
        text,
    ));

    assert_eq!(result.tree.source_text(), text);
    assert_eq!(
        result
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code())
            .collect::<Vec<_>>(),
        ["E-SYNTAX-UNSUPPORTED-0001", "E-SYNTAX-MISSING-0001"],
        "{:#?}",
        result.diagnostics,
    );
    assert_eq!(result.diagnostics[0].actual(), Some("@"));
    assert_eq!(result.diagnostics[1].expected(), Some(";"));
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::Statement), 1);
    assert_eq!(
        count_kind(result.tree.root(), SyntaxKind::ReturnStatement),
        1
    );
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::Expression), 2);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::Error), 1);
    assert!(contains_zero_width_token(
        result.tree.root(),
        TokenKind::Missing
    ));
    assert!(result.tree.has_recovery());
}

#[test]
fn unsupported_return_expression_start_recovers_before_following_statements() {
    let text = concat!(
        "module demo.main;\n",
        "pub fn value(input: I32) -> I32 {\n",
        "    return while input;\n",
        "    let next = input;\n",
        "    next\n",
        "}\n",
    );
    let result = parse(&source("unsupported-return-expression-start.mnd", text));

    assert_eq!(result.tree.source_text(), text);
    assert_eq!(result.diagnostics.len(), 1, "{:#?}", result.diagnostics);
    assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-UNSUPPORTED-0001");
    assert_eq!(result.diagnostics[0].actual(), Some("while"));
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::Statement), 2);
    assert_eq!(
        count_kind(result.tree.root(), SyntaxKind::ReturnStatement),
        1
    );
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::LetStatement), 1);
    let return_statement =
        find_kind(result.tree.root(), SyntaxKind::ReturnStatement).expect("return statement");
    assert!(contains_kind(return_statement, SyntaxKind::Error));
    assert!(!contains_zero_width_token(
        result.tree.root(),
        TokenKind::Missing
    ));
    assert!(result.tree.has_recovery());
}

#[test]
fn unsupported_return_expression_ignores_statement_keywords_inside_delimiters() {
    let text = concat!(
        "module demo.main;\n",
        "pub fn value(input: I32) -> I32 {\n",
        "    return while(return);\n",
        "    let next = input;\n",
        "    next\n",
        "}\n",
    );
    let result = parse(&source("nested-unsupported-return-expression.mnd", text));

    assert_eq!(result.tree.source_text(), text);
    assert_eq!(result.diagnostics.len(), 1, "{:#?}", result.diagnostics);
    assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-UNSUPPORTED-0001");
    assert_eq!(result.diagnostics[0].actual(), Some("while"));
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::Statement), 2);
    assert_eq!(
        count_kind(result.tree.root(), SyntaxKind::ReturnStatement),
        1
    );
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::LetStatement), 1);
    let return_statement =
        find_kind(result.tree.root(), SyntaxKind::ReturnStatement).expect("return statement");
    assert!(contains_kind(return_statement, SyntaxKind::Error));
    assert!(!contains_zero_width_token(
        result.tree.root(),
        TokenKind::Missing
    ));
    assert!(result.tree.has_recovery());
}

#[test]
fn unsupported_return_expression_ignores_declaration_shapes_inside_delimiters() {
    let text = concat!(
        "module demo.main;\n",
        "pub fn value(input: I32) -> I32 {\n",
        "    return while({ pub fn inner(x: I32) -> I32 { x } });\n",
        "    let next = input;\n",
        "    next\n",
        "}\n",
    );
    let result = parse(&source(
        "nested-declaration-shaped-return-expression.mnd",
        text,
    ));

    assert_eq!(result.tree.source_text(), text);
    assert_eq!(result.diagnostics.len(), 1, "{:#?}", result.diagnostics);
    assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-UNSUPPORTED-0001");
    assert_eq!(result.diagnostics[0].actual(), Some("while"));
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::Statement), 2);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::LetStatement), 1);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::FunctionDecl), 1);
    let return_statement =
        find_kind(result.tree.root(), SyntaxKind::ReturnStatement).expect("return statement");
    assert!(contains_kind(return_statement, SyntaxKind::Error));
    assert!(!contains_zero_width_token(
        result.tree.root(),
        TokenKind::Missing
    ));
    assert!(result.tree.has_recovery());
}

#[test]
fn unterminated_unsupported_return_expression_stops_at_the_block_boundary() {
    let text = concat!(
        "module demo.main;\n",
        "pub fn broken(input: I32) -> I32 { return while(input; }\n",
        "pub fn intact(input: I32) -> I32 { input }\n",
    );
    let result = parse(&source("unterminated-return-expression.mnd", text));

    assert_eq!(result.tree.source_text(), text);
    assert_eq!(result.diagnostics.len(), 1, "{:#?}", result.diagnostics);
    assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-UNSUPPORTED-0001");
    assert_eq!(result.diagnostics[0].actual(), Some("while"));
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::FunctionDecl), 2);
    assert_eq!(
        count_kind(result.tree.root(), SyntaxKind::ReturnStatement),
        1
    );
    let return_statement =
        find_kind(result.tree.root(), SyntaxKind::ReturnStatement).expect("return statement");
    assert!(contains_kind(return_statement, SyntaxKind::Error));
    assert!(!contains_zero_width_token(
        result.tree.root(),
        TokenKind::Missing
    ));
    assert!(result.tree.has_recovery());
}

#[test]
fn missing_block_close_after_return_preserves_the_following_declaration() {
    let text = concat!(
        "module demo.main;\n",
        "pub fn broken(input: I32) -> I32 {\n",
        "    return while(input;\n",
        "pub fn intact(input: I32) -> I32 { input }\n",
    );
    let result = parse(&source("missing-block-close-after-return.mnd", text));

    assert_eq!(result.tree.source_text(), text);
    assert_eq!(result.diagnostics.len(), 2, "{:#?}", result.diagnostics);
    assert_eq!(
        result
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code())
            .collect::<Vec<_>>(),
        ["E-SYNTAX-UNSUPPORTED-0001", "E-SYNTAX-MISSING-0001"],
    );
    assert_eq!(result.diagnostics[0].actual(), Some("while"));
    assert_eq!(result.diagnostics[1].expected(), Some("}"));
    assert_eq!(result.diagnostics[1].actual(), Some("pub"));
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::FunctionDecl), 2);
    assert_eq!(
        count_kind(result.tree.root(), SyntaxKind::ReturnStatement),
        1
    );
    assert!(contains_zero_width_token(
        result.tree.root(),
        TokenKind::Missing
    ));
    assert!(result.tree.has_recovery());
}

#[test]
fn unterminated_return_delimiters_resynchronize_before_following_block_elements() {
    let cases = [
        ("before-let", "let next = input; next", 2, 1, 1, 2),
        ("before-return", "return input;", 2, 2, 0, 1),
        ("before-tail", "next", 1, 1, 0, 1),
    ];

    for (case, following, statement_count, return_count, let_count, expression_count) in cases {
        let text = format!(
            concat!(
                "module demo.main;\n",
                "pub fn value(input: I32) -> I32 {{\n",
                "    return while(input; {}\n",
                "}}\n",
            ),
            following,
        );
        let result = parse(&source(
            &format!("unterminated-return-delimiter-{case}.mnd"),
            &text,
        ));

        assert_eq!(result.tree.source_text(), text, "source loss for {case}");
        assert_eq!(
            result.diagnostics.len(),
            1,
            "{case}: {:#?}",
            result.diagnostics
        );
        assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-UNSUPPORTED-0001");
        assert_eq!(result.diagnostics[0].actual(), Some("while"));
        assert_eq!(
            count_kind(result.tree.root(), SyntaxKind::Statement),
            statement_count,
            "statement loss for {case}",
        );
        assert_eq!(
            count_kind(result.tree.root(), SyntaxKind::ReturnStatement),
            return_count,
            "return loss for {case}",
        );
        assert_eq!(
            count_kind(result.tree.root(), SyntaxKind::LetStatement),
            let_count,
            "let loss for {case}",
        );
        assert_eq!(
            count_kind(result.tree.root(), SyntaxKind::Expression),
            expression_count,
            "expression loss for {case}",
        );
        assert!(!contains_zero_width_token(
            result.tree.root(),
            TokenKind::Missing
        ));
        assert!(result.tree.has_recovery());
    }
}

#[test]
fn unterminated_return_delimiters_keep_line_separated_operands_before_their_semicolons() {
    let cases = [
        ("paren", "while(", "input", "while", 2),
        ("angle", "input::<", "input", "::", 3),
        ("bracket", "while[", "input", "while", 2),
        (
            "paren-continuation",
            "while(",
            "input +\n        value",
            "while",
            2,
        ),
        (
            "angle-continuation",
            "input::<",
            "input +\n        value",
            "::",
            3,
        ),
        (
            "bracket-continuation",
            "while[",
            "input +\n        value",
            "while",
            2,
        ),
        (
            "nested-call-comma-continuation",
            "while(call(",
            "input,\n        value",
            "while",
            2,
        ),
    ];

    for (case, expression_start, operand, actual, expression_count) in cases {
        let text = format!(
            concat!(
                "module demo.main;\n",
                "pub fn value(input: I32) -> I32 {{\n",
                "    return {}\n",
                "        {};\n",
                "    let next = input;\n",
                "    next\n",
                "}}\n",
            ),
            expression_start, operand,
        );
        let result = parse(&source(
            &format!("unterminated-return-before-operand-{case}.mnd"),
            &text,
        ));

        assert_eq!(result.tree.source_text(), text, "source loss for {case}");
        assert_eq!(
            result.diagnostics.len(),
            1,
            "{case}: {:#?}",
            result.diagnostics
        );
        assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-UNSUPPORTED-0001");
        assert_eq!(result.diagnostics[0].actual(), Some(actual));
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::Statement), 2);
        assert_eq!(
            count_kind(result.tree.root(), SyntaxKind::ReturnStatement),
            1
        );
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::LetStatement), 1);
        assert_eq!(
            count_kind(result.tree.root(), SyntaxKind::Expression),
            expression_count,
            "expression loss for {case}",
        );
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::Error), 1);
        assert!(!contains_zero_width_token(
            result.tree.root(),
            TokenKind::Missing
        ));
        assert!(result.tree.has_recovery());
    }
}

#[test]
fn unsupported_grammar_operators_continue_returns_across_line_breaks() {
    for operator in SPACED_OPERATORS
        .iter()
        .copied()
        .filter(|operator| !matches!(*operator, "+" | "-" | "*" | "/" | "%"))
    {
        let text = format!(
            concat!(
                "module demo.main;\n",
                "pub fn value(input: I32) -> I32 {{\n",
                "    return input {}\n",
                "        value;\n",
                "    let next = input;\n",
                "    next\n",
                "}}\n",
            ),
            operator,
        );
        let result = parse(&source(
            &format!("multiline-unsupported-return-operator-{operator}.mnd"),
            &text,
        ));

        assert_eq!(
            result.tree.source_text(),
            text,
            "source loss for {operator}",
        );
        assert_eq!(
            result.diagnostics.len(),
            1,
            "{operator}: {:#?}",
            result.diagnostics
        );
        assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-UNSUPPORTED-0001");
        assert_eq!(result.diagnostics[0].actual(), Some(operator));
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::Statement), 2);
        assert_eq!(
            count_kind(result.tree.root(), SyntaxKind::ReturnStatement),
            1
        );
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::LetStatement), 1);
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::Expression), 3);
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::Error), 1);
        assert!(!contains_zero_width_token(
            result.tree.root(),
            TokenKind::Missing
        ));
        assert!(result.tree.has_recovery());
    }
}

#[test]
fn grammar_defined_unary_operators_continue_returns_across_line_breaks() {
    for (case, expression_start, actual) in [
        ("outer", "!", "!"),
        ("unterminated-paren", "while(!", "while"),
    ] {
        let text = format!(
            concat!(
                "module demo.main;\n",
                "pub fn value(input: I32) -> I32 {{\n",
                "    return {}\n",
                "        value;\n",
                "    let next = input;\n",
                "    next\n",
                "}}\n",
            ),
            expression_start,
        );
        let result = parse(&source(
            &format!("multiline-unary-return-{case}.mnd"),
            &text,
        ));

        assert_eq!(result.tree.source_text(), text, "source loss for {case}");
        assert_eq!(
            result.diagnostics.len(),
            1,
            "{case}: {:#?}",
            result.diagnostics
        );
        assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-UNSUPPORTED-0001");
        assert_eq!(result.diagnostics[0].actual(), Some(actual));
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::Statement), 2);
        assert_eq!(
            count_kind(result.tree.root(), SyntaxKind::ReturnStatement),
            1
        );
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::LetStatement), 1);
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::Expression), 2);
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::Error), 1);
        assert!(!contains_zero_width_token(
            result.tree.root(),
            TokenKind::Missing
        ));
        assert!(result.tree.has_recovery());
    }
}

#[test]
fn balanced_return_angle_closers_do_not_hide_a_missing_semicolon_tail() {
    for (case, expression) in [
        ("single", "input::<value>"),
        ("nested", "input::<Outer<value>>"),
    ] {
        let text = format!(
            concat!(
                "module demo.main;\n",
                "pub fn value(input: I32) -> I32 {{\n",
                "    return {}\n",
                "    next\n",
                "}}\n",
            ),
            expression,
        );
        let result = parse(&source(
            &format!("missing-return-semicolon-after-angle-{case}.mnd"),
            &text,
        ));

        assert_eq!(result.tree.source_text(), text, "source loss for {case}");
        assert_eq!(
            result
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code())
                .collect::<Vec<_>>(),
            ["E-SYNTAX-UNSUPPORTED-0001", "E-SYNTAX-MISSING-0001"],
            "{case}: {:#?}",
            result.diagnostics,
        );
        assert_eq!(result.diagnostics[0].actual(), Some("::"));
        assert_eq!(result.diagnostics[1].expected(), Some(";"));
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::Statement), 1);
        assert_eq!(
            count_kind(result.tree.root(), SyntaxKind::ReturnStatement),
            1
        );
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::Expression), 2);
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::Error), 1);
        assert!(contains_zero_width_token(
            result.tree.root(),
            TokenKind::Missing
        ));
        assert!(result.tree.has_recovery());
    }
}

#[test]
fn semicolons_inside_balanced_inner_return_delimiters_do_not_end_the_return() {
    let cases = [
        ("paren", "while(call(input; input)", "while"),
        ("angle", "while<call<input; input>", "while"),
        ("bracket", "while[call[input; input]", "while"),
    ];

    for (case, expression, actual) in cases {
        let text = format!(
            concat!(
                "module demo.main;\n",
                "pub fn value(input: I32) -> I32 {{\n",
                "    return {}\n",
                "    let next = input;\n",
                "    next\n",
                "}}\n",
            ),
            expression,
        );
        let result = parse(&source(
            &format!("balanced-inner-return-semicolon-{case}.mnd"),
            &text,
        ));

        assert_eq!(result.tree.source_text(), text, "source loss for {case}");
        assert_eq!(
            result
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code())
                .collect::<Vec<_>>(),
            ["E-SYNTAX-UNSUPPORTED-0001", "E-SYNTAX-MISSING-0001"],
            "{case}: {:#?}",
            result.diagnostics,
        );
        assert_eq!(result.diagnostics[0].actual(), Some(actual));
        assert_eq!(result.diagnostics[1].expected(), Some(";"));
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::Statement), 2);
        assert_eq!(
            count_kind(result.tree.root(), SyntaxKind::ReturnStatement),
            1
        );
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::LetStatement), 1);
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::Expression), 2);
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::Error), 1);
        assert!(contains_zero_width_token(
            result.tree.root(),
            TokenKind::Missing
        ));
        assert!(result.tree.has_recovery());
    }
}

#[test]
fn unsupported_return_expression_keeps_semicolons_inside_balanced_delimiters() {
    let text = concat!(
        "module demo.main;\n",
        "pub fn value(input: I32) -> I32 {\n",
        "    return while([input; input]);\n",
        "    let next = input;\n",
        "    next\n",
        "}\n",
    );
    let result = parse(&source("balanced-return-delimiters.mnd", text));

    assert_eq!(result.tree.source_text(), text);
    assert_eq!(result.diagnostics.len(), 1, "{:#?}", result.diagnostics);
    assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-UNSUPPORTED-0001");
    assert_eq!(result.diagnostics[0].actual(), Some("while"));
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::Statement), 2);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::LetStatement), 1);
    let return_statement =
        find_kind(result.tree.root(), SyntaxKind::ReturnStatement).expect("return statement");
    assert!(contains_kind(return_statement, SyntaxKind::Error));
    assert!(!contains_zero_width_token(
        result.tree.root(),
        TokenKind::Missing
    ));
    assert!(result.tree.has_recovery());
}

#[test]
fn unsupported_return_expression_keeps_line_separated_tokens_inside_balanced_delimiters() {
    for (case, expression, actual) in [
        ("paren", "while(\n        input\n    )", "while"),
        ("angle", "input::<\n        value\n    >", "::"),
    ] {
        let text = format!(
            concat!(
                "module demo.main;\n",
                "pub fn value(input: I32) -> I32 {{\n",
                "    return {};\n",
                "    let next = input;\n",
                "    next\n",
                "}}\n",
            ),
            expression,
        );
        let result = parse(&source(
            &format!("balanced-multiline-return-{case}.mnd"),
            &text,
        ));

        assert_eq!(result.tree.source_text(), text, "source loss for {case}");
        assert_eq!(
            result.diagnostics.len(),
            1,
            "{case}: {:#?}",
            result.diagnostics
        );
        assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-UNSUPPORTED-0001");
        assert_eq!(result.diagnostics[0].actual(), Some(actual));
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::Statement), 2);
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::LetStatement), 1);
        let return_statement =
            find_kind(result.tree.root(), SyntaxKind::ReturnStatement).expect("return statement");
        assert!(contains_kind(return_statement, SyntaxKind::Error));
        assert!(!contains_zero_width_token(
            result.tree.root(),
            TokenKind::Missing
        ));
        assert!(result.tree.has_recovery());
    }
}

#[test]
fn unsupported_return_suffix_tracks_balanced_angle_delimiters() {
    for (case, expression) in [
        ("keyword", "input::<return>"),
        ("nested", "input::<Outer<return>>"),
        ("semicolon", "input::<value; value>"),
    ] {
        let text = format!(
            concat!(
                "module demo.main;\n",
                "pub fn value(input: I32) -> I32 {{\n",
                "    return {};\n",
                "    let next = input;\n",
                "    next\n",
                "}}\n",
            ),
            expression,
        );
        let result = parse(&source(&format!("balanced-angle-return-{case}.mnd"), &text));

        assert_eq!(result.tree.source_text(), text, "source loss for {case}");
        assert_eq!(
            result.diagnostics.len(),
            1,
            "{case}: {:#?}",
            result.diagnostics
        );
        assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-UNSUPPORTED-0001");
        assert_eq!(result.diagnostics[0].actual(), Some("::"));
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::Statement), 2);
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::LetStatement), 1);
        let return_statement =
            find_kind(result.tree.root(), SyntaxKind::ReturnStatement).expect("return statement");
        assert!(contains_kind(return_statement, SyntaxKind::Error));
        assert!(!contains_zero_width_token(
            result.tree.root(),
            TokenKind::Missing
        ));
        assert!(result.tree.has_recovery());
    }
}

#[test]
fn unterminated_return_angle_delimiters_resynchronize_before_following_block_elements() {
    let cases = [
        ("before-let", "let next = input; next", 2, 1, 1, 3),
        ("before-return", "return input;", 2, 2, 0, 2),
        ("before-tail", "next", 1, 1, 0, 2),
    ];

    for (case, following, statement_count, return_count, let_count, expression_count) in cases {
        let text = format!(
            concat!(
                "module demo.main;\n",
                "pub fn value(input: I32) -> I32 {{\n",
                "    return input::<value; {}\n",
                "}}\n",
            ),
            following,
        );
        let result = parse(&source(
            &format!("unterminated-angle-return-{case}.mnd"),
            &text,
        ));

        assert_eq!(result.tree.source_text(), text, "source loss for {case}");
        assert_eq!(
            result.diagnostics.len(),
            1,
            "{case}: {:#?}",
            result.diagnostics
        );
        assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-UNSUPPORTED-0001");
        assert_eq!(result.diagnostics[0].actual(), Some("::"));
        assert_eq!(
            count_kind(result.tree.root(), SyntaxKind::Statement),
            statement_count,
            "statement loss for {case}",
        );
        assert_eq!(
            count_kind(result.tree.root(), SyntaxKind::ReturnStatement),
            return_count,
            "return loss for {case}",
        );
        assert_eq!(
            count_kind(result.tree.root(), SyntaxKind::LetStatement),
            let_count,
            "let loss for {case}",
        );
        assert_eq!(
            count_kind(result.tree.root(), SyntaxKind::Expression),
            expression_count,
            "expression loss for {case}",
        );
        assert!(!contains_zero_width_token(
            result.tree.root(),
            TokenKind::Missing
        ));
        assert!(result.tree.has_recovery());
    }
}

#[test]
fn unterminated_return_delimiters_without_semicolons_preserve_following_block_elements() {
    let cases = [
        (
            "paren-before-let",
            "while(input",
            "let next = input; next",
            "while",
            2,
            1,
            1,
            2,
        ),
        (
            "angle-before-return",
            "input::<value",
            "return input;",
            "::",
            2,
            2,
            0,
            2,
        ),
        (
            "paren-before-tail",
            "while(input",
            "next",
            "while",
            1,
            1,
            0,
            1,
        ),
        (
            "multiline-paren-before-tail",
            "while(\n        input",
            "next",
            "while",
            1,
            1,
            0,
            1,
        ),
    ];

    for (
        case,
        expression,
        following,
        actual,
        statement_count,
        return_count,
        let_count,
        expression_count,
    ) in cases
    {
        let text = format!(
            concat!(
                "module demo.main;\n",
                "pub fn value(input: I32) -> I32 {{\n",
                "    return {}\n",
                "    {}\n",
                "}}\n",
            ),
            expression, following,
        );
        let result = parse(&source(
            &format!("missing-return-semicolon-{case}.mnd"),
            &text,
        ));

        assert_eq!(result.tree.source_text(), text, "source loss for {case}");
        assert_eq!(
            result
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code())
                .collect::<Vec<_>>(),
            ["E-SYNTAX-UNSUPPORTED-0001", "E-SYNTAX-MISSING-0001"],
            "{case}: {:#?}",
            result.diagnostics,
        );
        assert_eq!(result.diagnostics[0].actual(), Some(actual));
        assert_eq!(result.diagnostics[1].expected(), Some(";"));
        assert_eq!(
            count_kind(result.tree.root(), SyntaxKind::Statement),
            statement_count,
            "statement loss for {case}",
        );
        assert_eq!(
            count_kind(result.tree.root(), SyntaxKind::ReturnStatement),
            return_count,
            "return loss for {case}",
        );
        assert_eq!(
            count_kind(result.tree.root(), SyntaxKind::LetStatement),
            let_count,
            "let loss for {case}",
        );
        assert_eq!(
            count_kind(result.tree.root(), SyntaxKind::Expression),
            expression_count,
            "expression loss for {case}",
        );
        assert!(contains_zero_width_token(
            result.tree.root(),
            TokenKind::Missing
        ));
        assert!(result.tree.has_recovery());
    }
}

#[test]
fn missing_return_boundaries_preserve_following_top_level_declarations() {
    let cases = [
        (
            "function",
            "pub fn intact(input: I32) -> I32 { input }",
            SyntaxKind::FunctionDecl,
            2,
        ),
        (
            "record",
            "record Intact { value: I32, }",
            SyntaxKind::RecordDecl,
            1,
        ),
        ("enum", "enum Intact { Value, }", SyntaxKind::EnumDecl, 1),
    ];

    for (case, declaration, declaration_kind, declaration_count) in cases {
        let text = format!(
            concat!(
                "module demo.main;\n",
                "pub fn broken(input: I32) -> I32 {{\n",
                "    return input::<value\n",
                "{}\n",
            ),
            declaration,
        );
        let result = parse(&source(
            &format!("missing-return-boundary-before-{case}.mnd"),
            &text,
        ));

        assert_eq!(result.tree.source_text(), text, "source loss for {case}");
        assert_eq!(
            result
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code())
                .collect::<Vec<_>>(),
            [
                "E-SYNTAX-UNSUPPORTED-0001",
                "E-SYNTAX-MISSING-0001",
                "E-SYNTAX-MISSING-0001",
            ],
            "{case}: {:#?}",
            result.diagnostics,
        );
        assert_eq!(result.diagnostics[0].actual(), Some("::"));
        assert_eq!(result.diagnostics[1].expected(), Some(";"));
        assert_eq!(result.diagnostics[2].expected(), Some("}"));
        assert_eq!(
            count_kind(result.tree.root(), declaration_kind),
            declaration_count,
            "declaration loss for {case}",
        );
        assert_eq!(
            count_kind(result.tree.root(), SyntaxKind::ReturnStatement),
            1
        );
        assert!(contains_zero_width_token(
            result.tree.root(),
            TokenKind::Missing
        ));
        assert!(result.tree.has_recovery());
    }
}

#[test]
fn unsupported_return_punctuation_recovers_without_cascading() {
    let text = concat!(
        "module demo.main;\n",
        "pub fn value(input: I32) -> I32 {\n",
        "    return @;\n",
        "    let next = input;\n",
        "    next\n",
        "}\n",
    );
    let result = parse(&source("invalid-return-expression.mnd", text));

    assert_eq!(result.tree.source_text(), text);
    assert_eq!(result.diagnostics.len(), 1, "{:#?}", result.diagnostics);
    assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-UNSUPPORTED-0001");
    assert_eq!(result.diagnostics[0].actual(), Some("@"));
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::Statement), 2);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::LetStatement), 1);
    let return_statement =
        find_kind(result.tree.root(), SyntaxKind::ReturnStatement).expect("return statement");
    assert!(contains_kind(return_statement, SyntaxKind::Error));
    assert!(!contains_zero_width_token(
        result.tree.root(),
        TokenKind::Missing
    ));
    assert!(result.tree.has_recovery());
}

#[test]
fn lexer_invalid_tokens_inside_returns_do_not_gain_parser_diagnostics() {
    for (case, expression) in [("start", "\t"), ("call-argument", "call(\t)")] {
        let text = format!(
            concat!(
                "module demo.main;\n",
                "pub fn value(input: I32) -> I32 {{\n",
                "    return {};\n",
                "    let next = input;\n",
                "    next\n",
                "}}\n",
            ),
            expression,
        );
        let result = parse(&source(&format!("invalid-return-{case}.mnd"), &text));

        assert_eq!(result.tree.source_text(), text, "source loss for {case}");
        assert_eq!(
            result.diagnostics.len(),
            1,
            "{case}: {:#?}",
            result.diagnostics
        );
        assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-INVALID-0001");
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::Statement), 2);
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::LetStatement), 1);
        let return_statement =
            find_kind(result.tree.root(), SyntaxKind::ReturnStatement).expect("return statement");
        assert!(contains_kind(return_statement, SyntaxKind::Error));
        assert!(!contains_zero_width_token(
            result.tree.root(),
            TokenKind::Missing
        ));
        assert!(result.tree.has_recovery());
    }
}

#[test]
fn unsupported_returns_without_semicolons_preserve_line_separated_tails() {
    let cases = [
        ("unsupported-suffix", "input.member", 2, 1),
        ("unsupported-keyword", "while input", 1, 0),
        ("unsupported-punctuation", "@ value", 1, 0),
    ];

    for (case, return_expression, expression_count, return_expression_count) in cases {
        let text = format!(
            concat!(
                "module demo.main;\n",
                "pub fn value(input: I32) -> I32 {{\n",
                "    return {}\n",
                "    next\n",
                "}}\n",
            ),
            return_expression,
        );
        let result = parse(&source(
            &format!("missing-unsupported-return-semicolon-{case}.mnd"),
            &text,
        ));

        assert_eq!(result.tree.source_text(), text, "source loss for {case}");
        assert_eq!(
            result.diagnostics.len(),
            2,
            "{case}: {:#?}",
            result.diagnostics
        );
        assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-UNSUPPORTED-0001");
        assert_eq!(result.diagnostics[1].code(), "E-SYNTAX-MISSING-0001");
        assert_eq!(result.diagnostics[1].expected(), Some(";"));
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::Statement), 1);
        assert_eq!(
            count_kind(result.tree.root(), SyntaxKind::ReturnStatement),
            1
        );
        assert_eq!(
            count_kind(result.tree.root(), SyntaxKind::Expression),
            expression_count,
            "tail expression loss for {case}",
        );
        let return_statement =
            find_kind(result.tree.root(), SyntaxKind::ReturnStatement).expect("return statement");
        assert_eq!(
            count_kind(return_statement, SyntaxKind::Expression),
            return_expression_count,
            "return expression mismatch for {case}",
        );
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::Error), 1);
        assert!(contains_zero_width_token(
            result.tree.root(),
            TokenKind::Missing
        ));
        assert!(result.tree.has_recovery());
    }
}

#[test]
fn missing_return_semicolon_preserves_following_block_elements() {
    let cases = [
        ("before-return", "return input return;", 2, 2, 0, 1),
        (
            "before-let",
            "return input let next = input; next",
            2,
            1,
            1,
            3,
        ),
        ("before-tail", "return input next", 1, 1, 0, 2),
        ("before-close", "return input", 1, 1, 0, 1),
    ];

    for (case, body, statement_count, return_count, let_count, expression_count) in cases {
        let text = format!("module demo.main;\npub fn value(input: I32) -> I32 {{ {body} }}\n");
        let result = parse(&source(
            &format!("missing-return-semicolon-{case}.mnd"),
            &text,
        ));

        assert_eq!(result.tree.source_text(), text, "source loss for {case}");
        assert_eq!(
            result.diagnostics.len(),
            1,
            "{case}: {:#?}",
            result.diagnostics
        );
        assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-MISSING-0001");
        assert_eq!(result.diagnostics[0].expected(), Some(";"));
        assert_eq!(
            count_kind(result.tree.root(), SyntaxKind::Statement),
            statement_count,
            "statement loss for {case}",
        );
        assert_eq!(
            count_kind(result.tree.root(), SyntaxKind::ReturnStatement),
            return_count,
            "return loss for {case}",
        );
        assert_eq!(
            count_kind(result.tree.root(), SyntaxKind::LetStatement),
            let_count,
            "let loss for {case}",
        );
        assert_eq!(
            count_kind(result.tree.root(), SyntaxKind::Expression),
            expression_count,
            "expression loss for {case}",
        );
        assert!(contains_zero_width_token(
            result.tree.root(),
            TokenKind::Missing
        ));
        assert!(!contains_kind(result.tree.root(), SyntaxKind::Error));
        assert!(result.tree.has_recovery());
    }
}

#[test]
fn parses_identifier_let_statements_before_a_trailing_expression() {
    let text = concat!(
        "module demo.main;\n",
        "pub fn total(input: I32) -> I32 {\n",
        "    let doubled = input * 2;\n",
        "    let adjusted: I32 = doubled + 1;\n",
        "    adjusted\n",
        "}\n",
    );
    let result = parse(&source("let-statements.mnd", text));

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert_eq!(result.tree.source_text(), text);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::Statement), 2);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::LetStatement), 2);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::Pattern), 2);
    assert_eq!(
        count_kind(result.tree.root(), SyntaxKind::BindingPattern),
        2
    );
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::FunctionDecl), 1);
    assert!(!result.tree.has_recovery());
}

#[test]
fn parses_a_let_only_block_without_requiring_a_trailing_expression() {
    let text = concat!(
        "module demo.main;\n",
        "pub fn remember(input: I32) -> I32 {\n",
        "    let remembered = input;\n",
        "}\n",
    );
    let result = parse(&source("let-only-block.mnd", text));

    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert_eq!(result.tree.source_text(), text);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::Statement), 1);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::LetStatement), 1);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::Expression), 1);
    assert!(!result.tree.has_recovery());
}

#[test]
fn unsupported_let_pattern_recovers_before_following_statements() {
    let text = concat!(
        "module demo.main;\n",
        "pub fn value(input: I32) -> I32 {\n",
        "    let _ = input;\n",
        "    let value = input;\n",
        "    value\n",
        "}\n",
    );
    let result = parse(&source("unsupported-let-pattern.mnd", text));

    assert_eq!(result.tree.source_text(), text);
    assert_eq!(result.diagnostics.len(), 1, "{:#?}", result.diagnostics);
    assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-UNSUPPORTED-0001");
    assert_eq!(result.diagnostics[0].actual(), Some("_"));
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::Statement), 2);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::LetStatement), 2);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::Pattern), 2);
    assert_eq!(
        count_kind(result.tree.root(), SyntaxKind::BindingPattern),
        1
    );
    assert!(contains_kind(result.tree.root(), SyntaxKind::Error));
    assert!(!contains_zero_width_token(
        result.tree.root(),
        TokenKind::Missing
    ));
    assert!(result.tree.has_recovery());
}

#[test]
fn unsupported_compound_let_pattern_recovers_at_the_initializer_boundary() {
    let text = concat!(
        "module demo.main;\n",
        "pub fn value(input: I32) -> I32 {\n",
        "    let (left, right) = input;\n",
        "    let value = input;\n",
        "    value\n",
        "}\n",
    );
    let result = parse(&source("unsupported-compound-let-pattern.mnd", text));

    assert_eq!(result.tree.source_text(), text);
    assert_eq!(result.diagnostics.len(), 1, "{:#?}", result.diagnostics);
    assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-UNSUPPORTED-0001");
    assert_eq!(result.diagnostics[0].actual(), Some("("));
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::Statement), 2);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::LetStatement), 2);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::Pattern), 2);
    assert_eq!(
        count_kind(result.tree.root(), SyntaxKind::BindingPattern),
        1
    );
    assert!(contains_kind(result.tree.root(), SyntaxKind::Error));
    assert!(!contains_zero_width_token(
        result.tree.root(),
        TokenKind::Missing
    ));
    assert!(result.tree.has_recovery());
}

#[test]
fn unsupported_identifier_led_let_pattern_recovers_at_the_initializer_boundary() {
    let text = concat!(
        "module demo.main;\n",
        "pub fn value(input: I32) -> I32 {\n",
        "    let Foo(bar) = input;\n",
        "    let value = input;\n",
        "    value\n",
        "}\n",
    );
    let result = parse(&source("unsupported-identifier-led-let-pattern.mnd", text));

    assert_eq!(result.tree.source_text(), text);
    assert_eq!(result.diagnostics.len(), 1, "{:#?}", result.diagnostics);
    assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-UNSUPPORTED-0001");
    assert_eq!(result.diagnostics[0].actual(), Some("Foo"));
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::Statement), 2);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::LetStatement), 2);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::Pattern), 2);
    assert_eq!(
        count_kind(result.tree.root(), SyntaxKind::BindingPattern),
        1
    );
    assert!(contains_kind(result.tree.root(), SyntaxKind::Error));
    assert!(!contains_zero_width_token(
        result.tree.root(),
        TokenKind::Missing
    ));
    assert!(result.tree.has_recovery());
}

#[test]
fn unsupported_let_type_recovers_at_the_initializer_boundary() {
    let text = concat!(
        "module demo.main;\n",
        "pub fn value(input: I32) -> I32 {\n",
        "    let wrapped: Box<I32> = input;\n",
        "    let value = input;\n",
        "    value\n",
        "}\n",
    );
    let result = parse(&source("unsupported-let-type.mnd", text));

    assert_eq!(result.tree.source_text(), text);
    assert_eq!(result.diagnostics.len(), 1, "{:#?}", result.diagnostics);
    assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-UNSUPPORTED-0001");
    assert_eq!(result.diagnostics[0].actual(), Some("<"));
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::Statement), 2);
    assert_eq!(count_kind(result.tree.root(), SyntaxKind::LetStatement), 2);
    assert_eq!(
        count_kind(result.tree.root(), SyntaxKind::BindingPattern),
        2
    );
    assert!(contains_kind(result.tree.root(), SyntaxKind::Error));
    assert!(!contains_zero_width_token(
        result.tree.root(),
        TokenKind::Missing
    ));
    assert!(result.tree.has_recovery());
}

#[test]
fn expression_shaped_unsupported_let_types_recover_at_the_initializer_boundary() {
    for (case, unsupported_type, actual) in [
        ("call-shaped", "Foo(Bar)", "("),
        ("adjacent-name", "Foo Bar", "Bar"),
        ("nested-paren-boundary", "Foo(Bar = Baz)", "("),
        ("nested-bracket-boundary", "Foo[Bar = Baz]", "["),
    ] {
        let text = format!(
            concat!(
                "module demo.main;\n",
                "pub fn value(input: I32) -> I32 {{\n",
                "    let wrapped: {} = input;\n",
                "    let value = input;\n",
                "    value\n",
                "}}\n",
            ),
            unsupported_type,
        );
        let result = parse(&source(&format!("unsupported-{case}-let-type.mnd"), &text));

        assert_eq!(result.tree.source_text(), text, "source loss for {case}");
        assert_eq!(
            result.diagnostics.len(),
            1,
            "{case}: {:#?}",
            result.diagnostics
        );
        assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-UNSUPPORTED-0001");
        assert_eq!(result.diagnostics[0].actual(), Some(actual));
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::Statement), 2);
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::LetStatement), 2);
        assert_eq!(
            count_kind(result.tree.root(), SyntaxKind::BindingPattern),
            2
        );
        assert!(contains_kind(result.tree.root(), SyntaxKind::Error));
        assert!(!contains_zero_width_token(
            result.tree.root(),
            TokenKind::Missing
        ));
        assert!(result.tree.has_recovery());
    }
}

#[test]
fn malformed_let_boundaries_recover_without_losing_the_block_tail() {
    let cases = [
        (
            "missing-let-pattern.mnd",
            "let = input; input",
            "identifier",
            1,
        ),
        (
            "missing-let-type.mnd",
            "let value: = input; value",
            "type",
            1,
        ),
        ("missing-let-equals.mnd", "let value input; value", "=", 1),
        (
            "missing-let-equals-after-type.mnd",
            "let value: I32 input; value",
            "=",
            1,
        ),
        (
            "missing-let-initializer.mnd",
            "let value = ; input",
            "expression",
            1,
        ),
        (
            "missing-let-semicolon-before-statement.mnd",
            "let first = input let second = first; second",
            ";",
            2,
        ),
        (
            "missing-let-semicolon-before-tail.mnd",
            "let value = input value",
            ";",
            1,
        ),
    ];

    for (path, body, expected, statement_count) in cases {
        let text = format!("module demo.main;\npub fn value(input: I32) -> I32 {{ {body} }}\n");
        let result = parse(&source(path, &text));

        assert_eq!(result.tree.source_text(), text, "source loss for {path}");
        assert_eq!(
            result.diagnostics.len(),
            1,
            "{path}: {:#?}",
            result.diagnostics
        );
        assert_eq!(result.diagnostics[0].code(), "E-SYNTAX-MISSING-0001");
        assert_eq!(result.diagnostics[0].expected(), Some(expected));
        assert_eq!(
            count_kind(result.tree.root(), SyntaxKind::Statement),
            statement_count,
            "statement loss for {path}",
        );
        assert_eq!(
            count_kind(result.tree.root(), SyntaxKind::LetStatement),
            statement_count,
            "let loss for {path}",
        );
        assert_eq!(count_kind(result.tree.root(), SyntaxKind::FunctionDecl), 1);
        assert!(contains_zero_width_token(
            result.tree.root(),
            TokenKind::Missing
        ));
        assert!(!contains_kind(result.tree.root(), SyntaxKind::Error));
        assert!(result.tree.has_recovery());
    }
}

#[test]
fn rejects_syntax_beyond_the_implemented_expression_subset() {
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
            "unary-expression",
            "pub fn value(input: I32) -> I32 { -input }",
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
        (
            "import_decl",
            "\"import\", import_path, [ \"as\", identifier ], \";\"",
        ),
        (
            "import_path",
            "qualified_name, [ \".\", \"{\", import_item, { \",\", import_item }, [ \",\" ], \"}\" ]",
        ),
        ("import_item", "identifier, [ \"as\", identifier ]"),
        ("qualified_name", "identifier, { \".\", identifier }"),
        ("visibility", "\"pub\" | \"internal\""),
        (
            "record_decl",
            "[ visibility ], \"record\", identifier, [ generic_params ], [ where_clause ], record_body",
        ),
        ("record_body", "\"{\", { record_field }, \"}\""),
        (
            "record_field",
            "attributes, [ visibility ], identifier, \":\", type, \",\"",
        ),
        (
            "enum_decl",
            "[ visibility ], \"enum\", identifier, [ generic_params ], [ where_clause ], enum_body",
        ),
        ("enum_body", "\"{\", { enum_variant }, \"}\""),
        (
            "enum_variant",
            "attributes, identifier, [ \"{\", { record_field }, \"}\" ], \",\"",
        ),
        ("function_decl", "function_head, [ contract_clause ], block"),
        ("block", "\"{\", { statement }, [ expression ], \"}\""),
        (
            "statement",
            "let_statement | var_statement | use_statement | assignment_statement | expression_statement | return_statement | break_statement | continue_statement | while_statement | for_statement",
        ),
        (
            "let_statement",
            "\"let\", pattern, [ \":\", type ], \"=\", expression, \";\"",
        ),
        ("return_statement", "\"return\", [ expression ], \";\""),
        (
            "pattern",
            "wildcard_pattern | literal_pattern | binding_pattern | variant_pattern | record_pattern | tuple_pattern | list_pattern",
        ),
        ("binding_pattern", "[ \"move\" ], identifier"),
        (
            "additive_expression",
            "multiplicative_expression, { ( \"+\" | \"-\" ), multiplicative_expression }",
        ),
        (
            "multiplicative_expression",
            "unary_expression, { ( \"*\" | \"/\" | \"%\" ), unary_expression }",
        ),
        (
            "postfix_expression",
            "primary_expression, { postfix_suffix }",
        ),
        (
            "postfix_suffix",
            "type_apply_suffix | call_suffix | member_suffix | index_suffix | await_suffix | try_suffix | with_suffix",
        ),
        (
            "call_suffix",
            "\"(\", [ argument, { \",\", argument }, [ \",\" ] ], \")\"",
        ),
        ("argument", "[ identifier, \":\" ], expression"),
        ("parenthesized_expression", "\"(\", expression, \")\""),
        (
            "literal",
            "integer_literal | float_literal | decimal_literal | char_literal | string_literal | byte_string_literal | duration_literal | \"true\" | \"false\" | \"Unit\" | \"None\"",
        ),
    ] {
        assert_eq!(
            production_rule(production),
            Some(expected_rule),
            "{production}"
        );
    }
}
