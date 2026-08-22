use std::fs;
use std::path::PathBuf;

use mendrel_format::format;
use mendrel_parser::parse;
use mendrel_source::SourceFile;

fn phase_one_example_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/phase1.mnd")
}

#[test]
fn phase_one_supported_example_is_canonical_and_parse_preserving() {
    let path = phase_one_example_path();
    let bytes = fs::read(&path).expect("Phase 1 supported example must exist");
    let source = SourceFile::from_bytes(path.to_string_lossy(), bytes)
        .expect("Phase 1 supported example must be valid UTF-8");

    let parsed = parse(&source);
    assert!(
        parsed.diagnostics.is_empty(),
        "Phase 1 supported example must parse without diagnostics: {:#?}",
        parsed.diagnostics,
    );
    assert_eq!(parsed.tree.source_text(), source.text());
    assert!(!parsed.tree.has_recovery());

    let fingerprint = parsed.tree.structural_fingerprint();
    let formatted = format(&parsed.tree).expect("supported example must be formattable");
    assert_eq!(
        formatted,
        source.text(),
        "supported example must be canonical"
    );

    let reparsed_source = SourceFile::from_bytes("phase1-formatted.mnd", formatted.into_bytes())
        .expect("formatter must emit valid UTF-8");
    let reparsed = parse(&reparsed_source);
    assert!(
        reparsed.diagnostics.is_empty(),
        "formatted example must parse without diagnostics: {:#?}",
        reparsed.diagnostics,
    );
    assert_eq!(reparsed.tree.structural_fingerprint(), fingerprint);
    assert_eq!(
        format(&reparsed.tree).expect("reparsed supported example must be formattable"),
        reparsed_source.text(),
    );
}
