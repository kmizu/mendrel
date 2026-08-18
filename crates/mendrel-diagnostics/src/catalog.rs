#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Severity {
    Error,
    Warning,
    Information,
    InternalError,
}

impl Severity {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Information => "information",
            Self::InternalError => "internal-error",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CatalogEntry {
    pub code: &'static str,
    pub severity: Severity,
    pub summary: &'static str,
    pub phase: &'static str,
    pub documentation_id: &'static str,
    pub recovery_suggestion: &'static str,
}

pub const INVALID_UTF8: CatalogEntry = CatalogEntry {
    code: "E-SOURCE-UTF8-0001",
    severity: Severity::Error,
    summary: "source file is not valid UTF-8",
    phase: "source-load",
    documentation_id: "source.invalid-utf8",
    recovery_suggestion: "save the file as UTF-8 and retry",
};

pub const SOURCE_IO: CatalogEntry = CatalogEntry {
    code: "E-SOURCE-IO-0001",
    severity: Severity::Error,
    summary: "source file could not be read",
    phase: "source-load",
    documentation_id: "source.io",
    recovery_suggestion: "verify the source path and read permissions",
};

pub const SOURCE_TOO_LARGE: CatalogEntry = CatalogEntry {
    code: "E-SOURCE-SIZE-0001",
    severity: Severity::Error,
    summary: "source file exceeds the supported byte-span limit",
    phase: "source-load",
    documentation_id: "source.too-large",
    recovery_suggestion: "split the source file into smaller modules",
};

pub const INVALID_TOKEN: CatalogEntry = CatalogEntry {
    code: "E-SYNTAX-INVALID-0001",
    severity: Severity::Error,
    summary: "source contains an invalid token",
    phase: "lex",
    documentation_id: "syntax.invalid-token",
    recovery_suggestion: "remove or replace the invalid token",
};

pub const MISSING_TOKEN: CatalogEntry = CatalogEntry {
    code: "E-SYNTAX-MISSING-0001",
    severity: Severity::Error,
    summary: "expected syntax is missing",
    phase: "parse",
    documentation_id: "syntax.missing-token",
    recovery_suggestion: "insert the expected syntax at the primary span",
};

pub const UNSUPPORTED_SYNTAX: CatalogEntry = CatalogEntry {
    code: "E-SYNTAX-UNSUPPORTED-0001",
    severity: Severity::Error,
    summary: "syntax is outside the implemented Phase 1 subset",
    phase: "parse",
    documentation_id: "syntax.unsupported",
    recovery_suggestion: "rewrite the construct using the implemented Phase 1 subset",
};
