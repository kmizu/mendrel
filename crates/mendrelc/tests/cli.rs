use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../mendrel-parser/tests/fixtures")
        .join(name)
}

fn run(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mendrelc"))
        .args(arguments)
        .output()
        .expect("run mendrelc")
}

fn run_in(current_directory: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mendrelc"))
        .current_dir(current_directory)
        .args(arguments)
        .output()
        .expect("run mendrelc")
}

#[test]
fn version_and_usage_are_stable() {
    let version = run(&["--version"]);
    assert!(version.status.success());
    assert_eq!(
        String::from_utf8(version.stdout).expect("UTF-8 stdout"),
        "mendrelc 0.0.1\n"
    );
    assert!(version.stderr.is_empty());

    let unknown = run(&["unknown"]);
    assert_eq!(unknown.status.code(), Some(2));
    assert_eq!(
        String::from_utf8(unknown.stderr).expect("UTF-8 stderr"),
        "usage: mendrelc <check|cst|fmt> <path> [--error-format=json] [--check]\n"
    );
}

#[test]
fn rejects_command_specific_flags_on_irrelevant_commands() {
    let path = fixture("first_slice.mnd");
    let path = path.to_str().expect("UTF-8 fixture path");

    for command in ["check", "cst"] {
        let output = run(&[command, path, "--check"]);
        assert_eq!(output.status.code(), Some(2), "{command}");
        assert!(output.stdout.is_empty());
        assert_eq!(
            String::from_utf8(output.stderr).expect("UTF-8 usage"),
            "usage: mendrelc <check|cst|fmt> <path> [--error-format=json] [--check]\n"
        );
    }
}

#[test]
fn valid_check_is_silent_and_deterministic() {
    let path = fixture("first_slice.mnd");
    let path = path.to_str().expect("UTF-8 fixture path");
    let first = run(&["check", path, "--error-format=json"]);
    let second = run(&["check", path, "--error-format=json"]);

    assert!(first.status.success());
    assert!(first.stdout.is_empty());
    assert!(first.stderr.is_empty());
    assert_eq!(first.stdout, second.stdout);
    assert_eq!(first.stderr, second.stderr);
}

#[test]
fn malformed_check_emits_schema_valid_jsonl() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = run_in(
        &root,
        &[
            "check",
            "crates/mendrel-parser/tests/fixtures/missing_semicolon.mnd",
            "--error-format=json",
        ],
    );

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let json = String::from_utf8(output.stderr.clone()).expect("UTF-8 JSONL");
    assert_eq!(json, include_str!("fixtures/missing_semicolon.jsonl"));
    assert_eq!(json.lines().count(), 1);
    assert!(json.contains("\"code\":\"E-SYNTAX-MISSING-0001\""));
    assert!(!json.contains("error[E-"));

    let mut validator = Command::new("python")
        .arg("-c")
        .arg(
            "import json,jsonschema,sys,pathlib; schema=json.loads(pathlib.Path(sys.argv[1]).read_text()); jsonschema.Draft202012Validator(schema).validate(json.loads(sys.stdin.read()))",
        )
        .arg(root.join("schemas/diagnostic-v1.schema.json"))
        .stdin(Stdio::piped())
        .spawn()
        .expect("start schema validator");
    use std::io::Write;
    validator
        .stdin
        .take()
        .expect("validator stdin")
        .write_all(&output.stderr)
        .expect("write diagnostic JSON");
    assert!(
        validator
            .wait()
            .expect("wait for schema validator")
            .success()
    );
}

#[test]
fn cst_and_formatter_outputs_match_their_goldens() {
    let path = fixture("first_slice.mnd");
    let path = path.to_str().expect("UTF-8 fixture path");

    let cst = run(&["cst", path]);
    assert!(cst.status.success());
    assert_eq!(
        String::from_utf8(cst.stdout).expect("UTF-8 CST"),
        include_str!("fixtures/first_slice.cst")
    );

    let formatted = run(&["fmt", path]);
    assert!(formatted.status.success());
    assert_eq!(
        String::from_utf8(formatted.stdout).expect("UTF-8 source"),
        include_str!("../../mendrel-format/tests/fixtures/first_slice.formatted.mnd")
    );
}

#[test]
fn invalid_utf8_is_a_stable_source_diagnostic() {
    let directory = std::env::temp_dir().join(format!("mendrelc-utf8-test-{}", std::process::id()));
    fs::create_dir_all(&directory).expect("create test directory");
    fs::create_dir_all(directory.join("unused")).expect("create path-normalization segment");
    let path = directory.join("invalid.mnd");
    fs::write(&path, b"header\n\xF0\x9F\x98\x80\xFF").expect("write invalid UTF-8");
    let diagnostic_path = directory.join("unused/../invalid.mnd");

    let output = run(&[
        "check",
        diagnostic_path.to_str().expect("UTF-8 temporary path"),
        "--error-format=json",
    ]);

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).expect("UTF-8 diagnostic");
    assert!(stderr.contains("\"code\":\"E-SOURCE-UTF8-0001\""));
    assert!(stderr.contains("\"start\":{\"byte\":11,\"line\":1,\"column_utf16\":2}"));
    assert!(!stderr.contains("/unused/../"));
    assert!(stderr.contains("\"workspace_revision\":\"sha256:"));
    fs::remove_dir_all(directory).expect("remove test directory");
}

#[test]
fn unreadable_source_has_a_distinct_io_diagnostic() {
    let arguments = ["check", "missing/../gone.mnd", "--error-format=json"];
    let output = run_in(Path::new(env!("CARGO_MANIFEST_DIR")), &arguments);
    let repeated = run_in(Path::new(env!("CARGO_MANIFEST_DIR")), &arguments);

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).expect("UTF-8 diagnostic");
    assert_eq!(stderr.as_bytes(), repeated.stderr);
    assert!(stderr.contains("\"code\":\"E-SOURCE-IO-0001\""));
    assert!(!stderr.contains("E-SOURCE-UTF8-0001"));
    assert!(stderr.contains("\"file\":\"gone.mnd\""));
    assert!(stderr.contains("\"summary\":\"source file could not be read\""));
    assert!(stderr.contains("\"actual\":\"NotFound\""));
    assert!(!stderr.contains("No such file or directory"));
    assert!(stderr.contains("\"workspace_revision\":\"sha256:"));
}
