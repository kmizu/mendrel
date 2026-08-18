use mendrel_source::{
    ByteSpan, Position, SourceError, SourceFile, content_revision, normalize_path,
    position_in_valid_utf8_prefix,
};

#[test]
fn derives_utf16_positions_from_utf8_byte_offsets() {
    let source = SourceFile::from_bytes("a/./b/../c.mnd", "a😀\r\nβ".as_bytes().to_vec())
        .expect("valid UTF-8 source");

    assert_eq!(source.path(), "a/c.mnd");
    assert_eq!(source.position(5).expect("position at CR").line, 0);
    assert_eq!(source.position(5).expect("position at CR").column_utf16, 3);
    assert_eq!(source.position(7).expect("second line").line, 1);
    assert_eq!(
        source
            .position(7)
            .expect("start of second line")
            .column_utf16,
        0
    );
    assert_eq!(source.position(9).expect("end of source").column_utf16, 1);
}

#[test]
fn rejects_invalid_utf8_with_the_first_invalid_byte() {
    let error = SourceFile::from_bytes("bad.mnd", vec![b'a', 0xff, b'b'])
        .expect_err("invalid UTF-8 must be rejected");

    assert_eq!(error, SourceError::InvalidUtf8 { valid_up_to: 1 });
}

#[test]
fn rejects_non_boundary_and_out_of_bounds_positions() {
    let source = SourceFile::from_bytes("unicode.mnd", "😀".as_bytes().to_vec())
        .expect("valid UTF-8 source");

    assert_eq!(
        source.position(1),
        Err(SourceError::InvalidBoundary { byte: 1 })
    );
    assert_eq!(
        source.position(5),
        Err(SourceError::SpanOutOfBounds { byte: 5 })
    );
}

#[test]
fn byte_spans_are_half_open_and_ordered() {
    assert_eq!(ByteSpan::new(4, 9).expect("ordered span").len(), 5);
    assert_eq!(
        ByteSpan::new(9, 4),
        Err(SourceError::InvalidSpan { start: 9, end: 4 })
    );
}

#[test]
fn revisions_are_stable_content_digests() {
    assert_eq!(
        content_revision(b"abc"),
        "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );

    let source = SourceFile::from_bytes("same.mnd", b"abc".to_vec()).expect("valid source");
    assert_eq!(source.revision(), content_revision(b"abc"));
}

#[test]
fn normalizes_paths_even_when_source_loading_fails() {
    assert_eq!(normalize_path(r"a\.\b\..\c.mnd"), "a/c.mnd");
    assert_eq!(normalize_path("/a/./b/../c.mnd"), "/a/c.mnd");
}

#[test]
fn derives_positions_at_the_end_of_a_valid_utf8_prefix() {
    let bytes = b"line one\n\xF0\x9F\x98\x80\xFF";
    assert_eq!(
        position_in_valid_utf8_prefix(bytes, 13).expect("valid UTF-8 prefix"),
        Position {
            byte: 13,
            line: 1,
            column_utf16: 2,
        }
    );
}
