use mendrel_diagnostics::{
    Diagnostic, DiagnosticFix, DiagnosticSpan, INVALID_TOKEN, INVALID_UTF8, MISSING_TOKEN,
    SOURCE_IO, SOURCE_TOO_LARGE, TextEdit, UNSUPPORTED_SYNTAX, render_human, render_jsonl,
};
use mendrel_source::{ByteSpan, SourceFile};

fn missing_token_diagnostic() -> Diagnostic {
    let source = SourceFile::from_bytes("src/quote\"demo.mnd", b"module demo.main".to_vec())
        .expect("valid fixture");
    let span = DiagnosticSpan::from_source(
        &source,
        ByteSpan::new(16, 16).expect("zero-width insertion span"),
    )
    .expect("valid diagnostic span");

    Diagnostic::from_catalog(&MISSING_TOKEN, span, "workspace:bootstrap")
}

#[test]
fn human_and_json_renderers_share_the_catalog_fact() {
    let diagnostic = missing_token_diagnostic();

    assert_eq!(diagnostic.code(), "E-SYNTAX-MISSING-0001");
    assert_eq!(diagnostic.primary_span().start.byte, 16);
    assert_eq!(
        render_human(&diagnostic),
        concat!(
            "src/quote\"demo.mnd:1:17: error[E-SYNTAX-MISSING-0001]: ",
            "expected syntax is missing; help: insert the expected syntax at the primary span",
        )
    );
    assert_eq!(
        diagnostic.recovery_suggestion(),
        "insert the expected syntax at the primary span"
    );

    let json = render_jsonl(&diagnostic);
    assert!(json.ends_with('\n'));
    assert!(json.contains("\"schema_version\":\"mendrel.diagnostic/1\""));
    assert!(json.contains("\"diagnostic_id\":\"diag-0001\""));
    assert!(json.contains("\"code\":\"E-SYNTAX-MISSING-0001\""));
    assert!(json.contains("\"workspace_revision\":\"workspace:bootstrap\""));
    assert!(json.contains("\"cause_graph\":{\"roots\":[\"cause-1\"]"));
    assert!(
        json.contains("\"recovery_suggestion\":\"insert the expected syntax at the primary span\"")
    );
    assert!(json.contains("\"fixes\":[]"));
    assert!(json.contains("src/quote\\\"demo.mnd"));
}

#[test]
fn json_output_is_deterministic() {
    let diagnostic = missing_token_diagnostic();

    assert_eq!(render_jsonl(&diagnostic), render_jsonl(&diagnostic));
}

#[test]
fn machine_applicable_insertions_are_structured_fixes() {
    let span = missing_token_diagnostic().primary_span().clone();
    let diagnostic = missing_token_diagnostic().with_fix(DiagnosticFix::machine_applicable(
        "insert-semicolon",
        "Insert `;`",
        TextEdit::new(
            span,
            ";",
            "sha256:91b27a33e2a7d2f4c2e1e9ea0640b2a0171686618b58b6f31f7e83b4bbf6b2c7",
        ),
    ));

    assert_eq!(diagnostic.fixes().len(), 1);
    assert_eq!(diagnostic.fixes()[0].text_edits()[0].replacement(), ";");
    let json = render_jsonl(&diagnostic);
    assert!(json.contains("\"fix_id\":\"insert-semicolon\""));
    assert!(json.contains("\"applicability\":\"machine-applicable\""));
    assert!(json.contains("\"replacement\":\";\""));
    assert!(json.contains(
        "\"expected_source_digest\":\"sha256:91b27a33e2a7d2f4c2e1e9ea0640b2a0171686618b58b6f31f7e83b4bbf6b2c7\""
    ));
    assert!(json.contains("\"preview_required\":false"));
}

#[test]
fn every_bootstrap_error_has_a_recovery_suggestion() {
    for entry in [
        INVALID_UTF8,
        SOURCE_IO,
        SOURCE_TOO_LARGE,
        INVALID_TOKEN,
        MISSING_TOKEN,
        UNSUPPORTED_SYNTAX,
    ] {
        assert!(
            !entry.recovery_suggestion.is_empty(),
            "{} has no recovery suggestion",
            entry.code
        );
    }
}
