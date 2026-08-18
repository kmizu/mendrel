#![no_main]

use libfuzzer_sys::fuzz_target;
use mendrel_format::format;
use mendrel_parser::parse;
use mendrel_source::{SourceError, SourceFile};

fuzz_target!(|bytes: &[u8]| {
    let source = match SourceFile::from_bytes("fuzz.mnd", bytes.to_vec()) {
        Ok(source) => source,
        Err(SourceError::InvalidUtf8 { valid_up_to }) => {
            assert_eq!(
                std::str::from_utf8(bytes)
                    .expect_err("source rejected as invalid UTF-8")
                    .valid_up_to(),
                valid_up_to,
            );
            return;
        }
        Err(SourceError::SourceTooLarge { length }) => {
            assert_eq!(length, bytes.len());
            return;
        }
        Err(error) => panic!("source construction returned an unexpected error: {error}"),
    };
    let parsed = parse(&source);
    assert_eq!(parsed.tree.source_text(), source.text());

    if parsed.diagnostics.is_empty() && !parsed.tree.has_recovery() {
        let formatted = format(&parsed.tree).expect("valid CST must be formattable");
        let reparsed_source = SourceFile::from_bytes("formatted.mnd", formatted.into_bytes())
            .expect("formatter emits UTF-8");
        let reparsed = parse(&reparsed_source);
        assert!(reparsed.diagnostics.is_empty());
        assert_eq!(
            parsed.tree.structural_fingerprint(),
            reparsed.tree.structural_fingerprint()
        );
    }
});
