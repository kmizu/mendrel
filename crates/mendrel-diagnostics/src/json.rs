use std::fmt::Write;

use crate::{Diagnostic, DiagnosticSpan, Position};

pub(crate) fn render(diagnostic: &Diagnostic) -> String {
    let mut output = String::new();
    output.push('{');
    field(&mut output, "schema_version", "mendrel.diagnostic/1");
    output.push(',');
    field(&mut output, "diagnostic_id", diagnostic.id());
    output.push(',');
    field(&mut output, "code", diagnostic.code());
    output.push(',');
    field(
        &mut output,
        "severity",
        diagnostic.catalog().severity.as_str(),
    );
    output.push(',');
    field(&mut output, "summary", diagnostic.summary());
    output.push(',');
    field(
        &mut output,
        "workspace_revision",
        diagnostic.workspace_revision(),
    );
    output.push_str(",\"primary_span\":");
    span(&mut output, diagnostic.primary_span());
    output.push_str(",\"related_spans\":[],\"symbols\":[]");
    if let Some(expected) = diagnostic.expected() {
        output.push_str(",\"expected\":");
        string(&mut output, expected);
    }
    if let Some(actual) = diagnostic.actual() {
        output.push_str(",\"actual\":");
        string(&mut output, actual);
    }
    output.push_str(",\"recovery_suggestion\":");
    string(&mut output, diagnostic.recovery_suggestion());
    output.push_str(",\"cause_graph\":{\"roots\":[\"cause-1\"],\"nodes\":[{");
    field(&mut output, "id", "cause-1");
    output.push(',');
    field(&mut output, "kind", "source-expression");
    output.push(',');
    field(&mut output, "summary", diagnostic.summary());
    output.push_str(",\"span\":");
    span(&mut output, diagnostic.primary_span());
    output.push_str("}],\"edges\":[]}");
    output.push_str(",\"notes\":[");
    for (index, note) in diagnostic.notes().iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        string(&mut output, note);
    }
    output.push_str("],\"fixes\":[");
    for (index, fix) in diagnostic.fixes().iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push('{');
        field(&mut output, "fix_id", fix.id());
        output.push(',');
        field(&mut output, "title", fix.title());
        output.push(',');
        field(&mut output, "applicability", "machine-applicable");
        output.push_str(",\"text_edits\":[");
        for (edit_index, edit) in fix.text_edits().iter().enumerate() {
            if edit_index > 0 {
                output.push(',');
            }
            output.push_str("{\"span\":");
            span(&mut output, edit.span());
            output.push_str(",\"replacement\":");
            string(&mut output, edit.replacement());
            output.push_str(",\"expected_source_digest\":");
            string(&mut output, edit.expected_source_digest());
            output.push('}');
        }
        output.push_str("],\"preview_required\":false}");
    }
    output.push(']');
    output.push_str(",\"documentation\":{\"id\":");
    string(&mut output, diagnostic.catalog().documentation_id);
    output.push_str(
        "},\"suppression\":{\"allowed\":false,\"scope\":\"none\",\"reason_required\":false}",
    );
    output.push_str(",\"origin\":{\"component\":\"compiler\",\"phase\":");
    string(&mut output, diagnostic.catalog().phase);
    output.push_str("}}\n");
    output
}

fn field(output: &mut String, name: &str, value: &str) {
    string(output, name);
    output.push(':');
    string(output, value);
}

fn span(output: &mut String, value: &DiagnosticSpan) {
    output.push('{');
    field(output, "file", &value.file);
    output.push_str(",\"start\":");
    position(output, value.start);
    output.push_str(",\"end\":");
    position(output, value.end);
    output.push('}');
}

fn position(output: &mut String, value: Position) {
    write!(
        output,
        "{{\"byte\":{},\"line\":{},\"column_utf16\":{}}}",
        value.byte, value.line, value.column_utf16
    )
    .expect("writing to String cannot fail");
}

fn string(output: &mut String, value: &str) {
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character <= '\u{001f}' => {
                write!(output, "\\u{:04x}", u32::from(character))
                    .expect("writing to String cannot fail");
            }
            character => output.push(character),
        }
    }
    output.push('"');
}
