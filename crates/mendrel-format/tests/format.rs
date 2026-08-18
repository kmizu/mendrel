use mendrel_format::{FormatError, format};
use mendrel_parser::parse;
use mendrel_source::SourceFile;

fn parse_text(path: &str, text: &str) -> mendrel_parser::ParseResult {
    let source = SourceFile::from_bytes(path, text.as_bytes().to_vec())
        .expect("valid UTF-8 formatter fixture");
    parse(&source)
}

#[test]
fn formats_the_first_slice_canonically() {
    let input = "module   demo.main;pub fn add( left:I32,right:I32)->I32{left+right}";
    let parsed = parse_text("compact.mnd", input);
    assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);

    let formatted = format(&parsed.tree).expect("valid tree is formattable");

    assert_eq!(
        formatted,
        include_str!("fixtures/first_slice.formatted.mnd")
    );
}

#[test]
fn formatting_is_idempotent_and_parse_preserving() {
    let variants = [
        include_str!("fixtures/first_slice.formatted.mnd"),
        "module demo.main;pub fn add(left:I32,right:I32)->I32{left+right}",
        "module\n demo . main ;\n\npub   fn add (left : I32 , right : I32 ) -> I32\n{\nleft + right\n}",
    ];

    for (index, input) in variants.into_iter().enumerate() {
        let parsed = parse_text(&format!("variant-{index}.mnd"), input);
        assert!(
            parsed.diagnostics.is_empty(),
            "variant {index}: {:#?}",
            parsed.diagnostics
        );
        let before = parsed.tree.structural_fingerprint();
        let once = format(&parsed.tree).expect("valid tree");
        let reparsed = parse_text(&format!("formatted-{index}.mnd"), &once);
        assert!(reparsed.diagnostics.is_empty());
        assert_eq!(before, reparsed.tree.structural_fingerprint());
        assert_eq!(format(&reparsed.tree).expect("reparsed tree"), once);
    }
}

#[test]
fn formats_parenthesized_expressions_without_changing_their_structure() {
    let input = "module demo.main;pub fn grouped(left:I32,right:I32)->I32{((left+right))}";
    let expected = concat!(
        "module demo.main;\n\n",
        "pub fn grouped(left: I32, right: I32) -> I32 {\n",
        "    ((left + right))\n",
        "}\n",
    );
    let parsed = parse_text("parenthesized.mnd", input);
    assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);

    let before = parsed.tree.structural_fingerprint();
    let formatted = format(&parsed.tree).expect("valid parenthesized tree");
    assert_eq!(formatted, expected);

    let reparsed = parse_text("parenthesized-formatted.mnd", &formatted);
    assert!(
        reparsed.diagnostics.is_empty(),
        "{:#?}",
        reparsed.diagnostics
    );
    assert_eq!(reparsed.tree.structural_fingerprint(), before);
    assert_eq!(format(&reparsed.tree).expect("reparsed tree"), formatted);
}

#[test]
fn keeps_binary_operator_spacing_before_parenthesized_operands() {
    let cases = [
        ("left+(right)", "left + (right)"),
        ("(left+(right))", "(left + (right))"),
    ];

    for (index, (expression, expected_expression)) in cases.into_iter().enumerate() {
        let input =
            format!("module demo.main;pub fn grouped(left:I32,right:I32)->I32{{{expression}}}");
        let expected = format!(
            "module demo.main;\n\npub fn grouped(left: I32, right: I32) -> I32 {{\n    {expected_expression}\n}}\n"
        );
        let parsed = parse_text(&format!("operator-grouping-{index}.mnd"), &input);
        assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);

        let before = parsed.tree.structural_fingerprint();
        let formatted = format(&parsed.tree).expect("valid parenthesized operand");
        assert_eq!(formatted, expected);

        let reparsed = parse_text(
            &format!("operator-grouping-formatted-{index}.mnd"),
            &formatted,
        );
        assert!(
            reparsed.diagnostics.is_empty(),
            "{:#?}",
            reparsed.diagnostics
        );
        assert_eq!(reparsed.tree.structural_fingerprint(), before);
        assert_eq!(format(&reparsed.tree).expect("reparsed tree"), formatted);
    }
}

#[test]
fn keeps_canonical_spaces_before_parenthesized_call_arguments() {
    let input = concat!(
        "module demo.main;",
        "pub fn invoke(value:I32)->I32{",
        "apply ( ) ( value , ( value ) , named : ( value + value ) , )",
        "}",
    );
    let expected = concat!(
        "module demo.main;\n\n",
        "pub fn invoke(value: I32) -> I32 {\n",
        "    apply()(value, (value), named: (value + value),)\n",
        "}\n",
    );
    let parsed = parse_text("call-spacing.mnd", input);
    assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);

    let before = parsed.tree.structural_fingerprint();
    let formatted = format(&parsed.tree).expect("valid call expression");
    assert_eq!(formatted, expected);

    let reparsed = parse_text("call-spacing-formatted.mnd", &formatted);
    assert!(
        reparsed.diagnostics.is_empty(),
        "{:#?}",
        reparsed.diagnostics
    );
    assert_eq!(reparsed.tree.structural_fingerprint(), before);
    assert_eq!(format(&reparsed.tree).expect("reparsed tree"), formatted);
}

#[test]
fn formats_multiplicative_precedence_without_changing_structure() {
    let input = concat!(
        "module demo.main;",
        "pub fn calculate(left:I32,right:I32,value:I32)->I32{",
        "left*right/value%left+value",
        "}",
    );
    let expected = concat!(
        "module demo.main;\n\n",
        "pub fn calculate(left: I32, right: I32, value: I32) -> I32 {\n",
        "    left * right / value % left + value\n",
        "}\n",
    );
    let parsed = parse_text("multiplicative-format.mnd", input);
    assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);

    let before = parsed.tree.structural_fingerprint();
    let formatted = format(&parsed.tree).expect("valid multiplicative expression");
    assert_eq!(formatted, expected);

    let reparsed = parse_text("multiplicative-formatted.mnd", &formatted);
    assert!(
        reparsed.diagnostics.is_empty(),
        "{:#?}",
        reparsed.diagnostics
    );
    assert_eq!(reparsed.tree.structural_fingerprint(), before);
    assert_eq!(format(&reparsed.tree).expect("reparsed tree"), formatted);
}

#[test]
fn preserves_comment_text_and_relative_order() {
    let input = "// file\nmodule demo.main;\n/* add */\npub fn add(left:I32,right:I32)->I32{/* sum */\nleft+right}";
    let parsed = parse_text("comments.mnd", input);
    assert!(parsed.diagnostics.is_empty());

    let formatted = format(&parsed.tree).expect("valid commented tree");

    let file = formatted.find("// file").expect("file comment retained");
    let module = formatted
        .find("module demo.main;")
        .expect("module retained");
    let add = formatted
        .find("/* add */")
        .expect("function comment retained");
    let function = formatted.find("pub fn add").expect("function retained");
    let sum = formatted.find("/* sum */").expect("body comment retained");
    let expression = formatted.find("left + right").expect("body retained");
    assert!(file < module && module < add && add < function && function < sum && sum < expression);
    assert_eq!(formatted.matches("// file").count(), 1);
    assert_eq!(formatted.matches("/* add */").count(), 1);
    assert_eq!(formatted.matches("/* sum */").count(), 1);
}

#[test]
fn refuses_to_rewrite_a_recovered_tree() {
    let parsed = parse_text("broken.mnd", "module demo.main\n");
    assert!(parsed.tree.has_recovery());

    assert_eq!(format(&parsed.tree), Err(FormatError::MalformedTree));
}

#[test]
fn separates_multiple_top_level_functions_and_is_idempotent() {
    let input = concat!(
        "module demo.main;",
        "pub fn first(value:I32)->I32{value}",
        "pub fn second(value:I32)->I32{value}",
    );
    let expected = concat!(
        "module demo.main;\n\n",
        "pub fn first(value: I32) -> I32 {\n",
        "    value\n",
        "}\n\n",
        "pub fn second(value: I32) -> I32 {\n",
        "    value\n",
        "}\n",
    );

    let parsed = parse_text("multiple.mnd", input);
    assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
    let once = format(&parsed.tree).expect("valid multiple-declaration tree");
    assert_eq!(once, expected);

    let reparsed = parse_text("multiple-formatted.mnd", &once);
    assert!(reparsed.diagnostics.is_empty());
    assert_eq!(format(&reparsed.tree).expect("reparsed tree"), once);
}

#[test]
fn keeps_trailing_and_leading_comments_attached_to_their_declarations() {
    let input = concat!(
        "module demo.main; // module docs\n",
        "/** first docs */\n",
        "pub fn first(value:I32)->I32{/* body */ value}",
        " // first result\n",
        "// second docs\n",
        "pub fn second(value:I32)->I32{value}",
    );
    let expected = concat!(
        "module demo.main; // module docs\n\n",
        "/** first docs */\n",
        "pub fn first(value: I32) -> I32 {\n",
        "    /* body */\n",
        "    value\n",
        "} // first result\n\n",
        "// second docs\n",
        "pub fn second(value: I32) -> I32 {\n",
        "    value\n",
        "}\n",
    );

    let parsed = parse_text("comment-attachment.mnd", input);
    assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
    let formatted = format(&parsed.tree).expect("valid commented tree");

    assert_eq!(formatted, expected);
    let reparsed = parse_text("comment-attachment-formatted.mnd", &formatted);
    assert!(reparsed.diagnostics.is_empty());
    assert_eq!(format(&reparsed.tree).expect("reparsed tree"), formatted);
}

#[test]
fn keeps_same_line_trailing_comment_runs_together() {
    let input = concat!(
        "module demo.main; /* module one */ /* module two */\n",
        "pub fn first(value:I32)->I32{value}",
        " /* result one */ /* result two */\n",
        "// second docs\n",
        "pub fn second(value:I32)->I32{value}",
    );
    let expected = concat!(
        "module demo.main; /* module one */ /* module two */\n\n",
        "pub fn first(value: I32) -> I32 {\n",
        "    value\n",
        "} /* result one */ /* result two */\n\n",
        "// second docs\n",
        "pub fn second(value: I32) -> I32 {\n",
        "    value\n",
        "}\n",
    );

    let parsed = parse_text("comment-run.mnd", input);
    assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
    let formatted = format(&parsed.tree).expect("valid comment-run tree");
    assert_eq!(formatted, expected);

    let reparsed = parse_text("comment-run-formatted.mnd", &formatted);
    assert!(reparsed.diagnostics.is_empty());
    assert_eq!(format(&reparsed.tree).expect("reparsed tree"), formatted);
}
