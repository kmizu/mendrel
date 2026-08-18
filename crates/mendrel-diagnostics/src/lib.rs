mod catalog;
mod json;

pub use catalog::{
    CatalogEntry, INVALID_TOKEN, INVALID_UTF8, MISSING_TOKEN, SOURCE_IO, SOURCE_TOO_LARGE,
    Severity, UNSUPPORTED_SYNTAX,
};
pub use mendrel_source::Position;
use mendrel_source::{ByteSpan, SourceError, SourceFile};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticSpan {
    pub file: String,
    pub start: Position,
    pub end: Position,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextEdit {
    span: DiagnosticSpan,
    replacement: String,
    expected_source_digest: String,
}

impl TextEdit {
    #[must_use]
    pub fn new(
        span: DiagnosticSpan,
        replacement: impl Into<String>,
        expected_source_digest: impl Into<String>,
    ) -> Self {
        Self {
            span,
            replacement: replacement.into(),
            expected_source_digest: expected_source_digest.into(),
        }
    }

    #[must_use]
    pub const fn span(&self) -> &DiagnosticSpan {
        &self.span
    }

    #[must_use]
    pub fn replacement(&self) -> &str {
        &self.replacement
    }

    #[must_use]
    pub fn expected_source_digest(&self) -> &str {
        &self.expected_source_digest
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticFix {
    id: String,
    title: String,
    text_edits: Vec<TextEdit>,
}

impl DiagnosticFix {
    #[must_use]
    pub fn machine_applicable(
        id: impl Into<String>,
        title: impl Into<String>,
        edit: TextEdit,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            text_edits: vec![edit],
        }
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    #[must_use]
    pub fn text_edits(&self) -> &[TextEdit] {
        &self.text_edits
    }
}

impl DiagnosticSpan {
    pub fn from_source(source: &SourceFile, span: ByteSpan) -> Result<Self, SourceError> {
        if span.end > source.byte_len() {
            return Err(SourceError::SpanOutOfBounds {
                byte: usize::try_from(span.end).expect("u32 fits usize on supported targets"),
            });
        }
        Ok(Self {
            file: source.path().to_owned(),
            start: source.position(span.start)?,
            end: source.position(span.end)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    catalog: &'static CatalogEntry,
    id: String,
    summary: String,
    workspace_revision: String,
    primary_span: DiagnosticSpan,
    expected: Option<String>,
    actual: Option<String>,
    notes: Vec<String>,
    fixes: Vec<DiagnosticFix>,
}

impl Diagnostic {
    #[must_use]
    pub fn from_catalog(
        catalog: &'static CatalogEntry,
        primary_span: DiagnosticSpan,
        workspace_revision: impl Into<String>,
    ) -> Self {
        Self {
            catalog,
            id: "diag-0001".to_owned(),
            summary: catalog.summary.to_owned(),
            workspace_revision: workspace_revision.into(),
            primary_span,
            expected: None,
            actual: None,
            notes: Vec::new(),
            fixes: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }

    #[must_use]
    pub fn with_summary(mut self, summary: impl Into<String>) -> Self {
        self.summary = summary.into();
        self
    }

    #[must_use]
    pub fn with_expected(mut self, expected: impl Into<String>) -> Self {
        self.expected = Some(expected.into());
        self
    }

    #[must_use]
    pub fn with_actual(mut self, actual: impl Into<String>) -> Self {
        self.actual = Some(actual.into());
        self
    }

    #[must_use]
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    #[must_use]
    pub fn with_fix(mut self, fix: DiagnosticFix) -> Self {
        self.fixes.push(fix);
        self
    }

    #[must_use]
    pub const fn catalog(&self) -> &'static CatalogEntry {
        self.catalog
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn code(&self) -> &'static str {
        self.catalog.code
    }

    #[must_use]
    pub fn summary(&self) -> &str {
        &self.summary
    }

    #[must_use]
    pub fn workspace_revision(&self) -> &str {
        &self.workspace_revision
    }

    #[must_use]
    pub const fn primary_span(&self) -> &DiagnosticSpan {
        &self.primary_span
    }

    #[must_use]
    pub fn expected(&self) -> Option<&str> {
        self.expected.as_deref()
    }

    #[must_use]
    pub fn actual(&self) -> Option<&str> {
        self.actual.as_deref()
    }

    #[must_use]
    pub fn notes(&self) -> &[String] {
        &self.notes
    }

    #[must_use]
    pub fn recovery_suggestion(&self) -> &'static str {
        self.catalog.recovery_suggestion
    }

    #[must_use]
    pub fn fixes(&self) -> &[DiagnosticFix] {
        &self.fixes
    }
}

#[must_use]
pub fn render_human(diagnostic: &Diagnostic) -> String {
    let span = diagnostic.primary_span();
    format!(
        "{}:{}:{}: {}[{}]: {}; help: {}",
        span.file,
        span.start.line + 1,
        span.start.column_utf16 + 1,
        diagnostic.catalog().severity.as_str(),
        diagnostic.code(),
        diagnostic.summary(),
        diagnostic.recovery_suggestion(),
    )
}

#[must_use]
pub fn render_jsonl(diagnostic: &Diagnostic) -> String {
    json::render(diagnostic)
}
