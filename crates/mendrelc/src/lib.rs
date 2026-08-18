use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use mendrel_diagnostics::{
    Diagnostic, DiagnosticSpan, INVALID_UTF8, SOURCE_IO, SOURCE_TOO_LARGE, render_human,
    render_jsonl,
};
use mendrel_format::format;
use mendrel_parser::{ParseResult, parse};
use mendrel_source::{
    Position, SourceError, SourceFile, content_revision, normalize_path,
    position_in_valid_utf8_prefix,
};
use mendrel_syntax::generate::check_inventory;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

const COMMANDS: &str = "<check|cst|fmt> <path> [--error-format=json] [--check]";

pub fn run(
    program: &str,
    arguments: impl IntoIterator<Item = OsString>,
    current_directory: &Path,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> u8 {
    let arguments = arguments
        .into_iter()
        .map(|argument| argument.into_string())
        .collect::<Result<Vec<_>, _>>();
    let Ok(arguments) = arguments else {
        let _ = writeln!(stderr, "{program}: command arguments must be valid UTF-8");
        return 2;
    };

    if arguments == ["--version"] {
        let _ = writeln!(stdout, "{program} {VERSION}");
        return 0;
    }
    if program == "mendrel" && arguments == ["xtask", "generated", "--check"] {
        return match check_inventory(current_directory) {
            Ok(()) => {
                let _ = writeln!(stdout, "generated syntax inventory is current");
                0
            }
            Err(error) => {
                let _ = writeln!(stderr, "generated syntax inventory error: {error}");
                1
            }
        };
    }

    let Some(command) = arguments.first().map(String::as_str) else {
        usage(program, stderr);
        return 2;
    };
    if !matches!(command, "check" | "cst" | "fmt") {
        usage(program, stderr);
        return 2;
    }
    let Some(path) = arguments.get(1) else {
        usage(program, stderr);
        return 2;
    };
    let json = arguments
        .iter()
        .skip(2)
        .any(|argument| argument == "--error-format=json");
    let check_only = arguments
        .iter()
        .skip(2)
        .any(|argument| argument == "--check");
    if arguments
        .iter()
        .skip(2)
        .any(|argument| argument != "--error-format=json" && argument != "--check")
        || (check_only && command != "fmt")
    {
        usage(program, stderr);
        return 2;
    }

    let (source, parsed) = match load_and_parse(path, current_directory) {
        Ok(result) => result,
        Err(diagnostic) => {
            write_diagnostics(std::slice::from_ref(diagnostic.as_ref()), json, stderr);
            return 1;
        }
    };
    if !parsed.diagnostics.is_empty() {
        write_diagnostics(&parsed.diagnostics, json, stderr);
        return 1;
    }

    match command {
        "check" => 0,
        "cst" => {
            let _ = write!(stdout, "{}", parsed.tree.dump());
            0
        }
        "fmt" => match format(&parsed.tree) {
            Ok(formatted) if check_only && formatted != source.text() => {
                let _ = writeln!(stderr, "would reformat: {}", source.path());
                1
            }
            Ok(_) if check_only => 0,
            Ok(formatted) => {
                let _ = write!(stdout, "{formatted}");
                0
            }
            Err(error) => {
                let _ = writeln!(stderr, "{}: {error}", source.path());
                1
            }
        },
        _ => unreachable!("command was validated above"),
    }
}

fn load_and_parse(
    path: &str,
    current_directory: &Path,
) -> Result<(SourceFile, ParseResult), Box<Diagnostic>> {
    let io_path = resolve_path(path, current_directory);
    let bytes = fs::read(&io_path).map_err(|error| Box::new(source_io_diagnostic(path, &error)))?;
    let revision = content_revision(&bytes);
    if let Err(error) = std::str::from_utf8(&bytes) {
        let source_error = SourceError::InvalidUtf8 {
            valid_up_to: error.valid_up_to(),
        };
        return Err(Box::new(source_diagnostic(
            path,
            source_error,
            &bytes,
            &revision,
        )));
    }
    let source = SourceFile::from_bytes(path, bytes)
        .map_err(|error| Box::new(source_diagnostic(path, error, &[], &revision)))?;
    let parsed = parse(&source);
    Ok((source, parsed))
}

fn resolve_path(path: &str, current_directory: &Path) -> PathBuf {
    let path = Path::new(path);
    if path.is_absolute() {
        path.to_owned()
    } else {
        current_directory.join(path)
    }
}

fn source_diagnostic(path: &str, error: SourceError, bytes: &[u8], revision: &str) -> Diagnostic {
    let (catalog, position, actual) = match error {
        SourceError::InvalidUtf8 { valid_up_to } => (
            &INVALID_UTF8,
            position_in_valid_utf8_prefix(bytes, valid_up_to).unwrap_or(Position {
                byte: 0,
                line: 0,
                column_utf16: 0,
            }),
            Some(format!("invalid byte at offset {valid_up_to}")),
        ),
        SourceError::SourceTooLarge { length } => (
            &SOURCE_TOO_LARGE,
            Position {
                byte: 0,
                line: 0,
                column_utf16: 0,
            },
            Some(format!("{length} bytes")),
        ),
        other => (
            &INVALID_UTF8,
            Position {
                byte: 0,
                line: 0,
                column_utf16: 0,
            },
            Some(other.to_string()),
        ),
    };
    let mut diagnostic = Diagnostic::from_catalog(
        catalog,
        DiagnosticSpan {
            file: normalize_path(path),
            start: position,
            end: position,
        },
        revision,
    );
    if let Some(actual) = actual {
        diagnostic = diagnostic.with_actual(actual);
    }
    diagnostic
}

fn source_io_diagnostic(path: &str, error: &std::io::Error) -> Diagnostic {
    let file = normalize_path(path);
    let actual = format!("{:?}", error.kind());
    let revision = content_revision(format!("{file}\0{actual}").as_bytes());
    Diagnostic::from_catalog(
        &SOURCE_IO,
        DiagnosticSpan {
            file,
            start: Position {
                byte: 0,
                line: 0,
                column_utf16: 0,
            },
            end: Position {
                byte: 0,
                line: 0,
                column_utf16: 0,
            },
        },
        revision,
    )
    .with_actual(actual)
}

fn write_diagnostics(diagnostics: &[Diagnostic], json: bool, output: &mut impl Write) {
    for diagnostic in diagnostics {
        if json {
            let _ = write!(output, "{}", render_jsonl(diagnostic));
        } else {
            let _ = writeln!(output, "{}", render_human(diagnostic));
        }
    }
}

fn usage(program: &str, output: &mut impl Write) {
    let _ = writeln!(output, "usage: {program} {COMMANDS}");
}
