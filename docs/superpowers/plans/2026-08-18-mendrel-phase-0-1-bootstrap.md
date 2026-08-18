# Mendrel Phase 0/1 Bootstrap Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the deterministic Phase 0 substrate and the first Phase 1 `module`/`pub fn`/addition vertical slice from bytes through diagnostics, lossless CST, canonical formatting, reparsing, and CLI output.

**Architecture:** Keep source coordinates, diagnostics, syntax storage, parsing, and formatting in separate crates with one-way dependencies. Use a hand-written recursive-descent parser plus Pratt expressions; preserve every input token and recovery artifact in the CST, and derive grammar inventories from `spec/grammar.ebnf`.

**Tech Stack:** Rust 1.96.0, Cargo workspace, Rust standard library only in production crates, cargo-fuzz/libFuzzer for the optional fuzz package, Python 3 only for the existing design-pack validator.

**Spec:** `PROMPT_FOR_CODEX.md`, with normative precedence from `docs/10-formal-kernel.md`, `docs/01-language-reference.md`, and the remaining documents listed by the bootstrap prompt.

## Global Constraints

- Text source remains canonical; AST/HIR, type checking, effects, MIR, runtime, code generation, packages, and MAP are out of scope.
- Source spans are half-open UTF-8 byte ranges; line and UTF-16 columns are derived.
- Invalid or incomplete source returns a CST with invalid, error, or zero-width missing elements.
- Formatting consumes the CST, preserves comments, has no options, and is idempotent.
- Diagnostic codes, severity, summaries, fixes, and renderers share one catalog entry per error.
- Grammar literals and production names come from `spec/grammar.ebnf`; generated output is checked rather than hand-duplicated.
- Compiler results cannot depend on the current directory, wall clock, randomness, locale, or filesystem enumeration order.
- Production crates add no third-party dependencies.
- This checkout has no Git metadata; do not initialize Git or create commits without a separate user request.

---

### Task 1: Workspace and pack boundary

**Files:**
- Create: `Cargo.toml`
- Create: `rust-toolchain.toml`
- Create: `.gitignore`
- Create: `xtask/Cargo.toml`
- Create: `xtask/src/main.rs`
- Modify: `scripts/validate_pack.py`
- Modify: `MANIFEST.md`
- Test: `scripts/validate_pack.py`

**Interfaces:**
- Consumes: the existing design-pack manifest and `spec/grammar.ebnf`.
- Produces: `cargo run -p xtask -- generated --check` and `cargo run -p xtask -- verify`; pack validation limited to the immutable design-pack surface rather than build output.

- [ ] **Step 1: Add a failing validator regression**

Add `check_manifest()` logic that computes expected paths only from the declared pack roots (`README.md`, `AGENTS.md`, `PROMPT_FOR_CODEX.md`, `VALIDATION.md`, `docs/*.md` excluding `docs/superpowers`, `schemas`, `spec`, `examples`, and `scripts`). The new plan file must no longer make strict pack validation fail.

- [ ] **Step 2: Run the validator and preserve the red result**

Run: `python scripts/validate_pack.py --strict-schema`

Expected before implementation: failure listing `docs/superpowers/plans/2026-08-18-mendrel-phase-0-1-bootstrap.md` as a manifest mismatch.

- [ ] **Step 3: Implement the pack boundary and workspace skeleton**

Create a workspace resolver 2 manifest with members for the crates named in later tasks and a pinned `rust-toolchain.toml` for `1.96.0` with `rustfmt` and `clippy`. Implement `xtask` argument parsing with these exact commands:

```text
xtask generated --check
xtask generated
xtask verify
```

Unknown arguments exit with code 2 and a deterministic usage line.

- [ ] **Step 4: Refresh only the validator digest row**

Update the `scripts/validate_pack.py` size and SHA-256 in `MANIFEST.md`; do not add implementation files to the design-pack manifest.

- [ ] **Step 5: Verify Task 1**

Run: `python scripts/validate_pack.py --strict-schema`

Expected: PASS with the original design-pack file count and all schema/EBNF checks green.

Run: `cargo run -p xtask -- generated --check`

Expected at this task boundary: a deterministic message that generated syntax output has not been created yet, with exit code 1.

### Task 2: UTF-8 source and structured diagnostic substrate

**Files:**
- Create: `crates/mendrel-source/Cargo.toml`
- Create: `crates/mendrel-source/src/lib.rs`
- Create: `crates/mendrel-source/tests/source.rs`
- Create: `crates/mendrel-diagnostics/Cargo.toml`
- Create: `crates/mendrel-diagnostics/src/catalog.rs`
- Create: `crates/mendrel-diagnostics/src/json.rs`
- Create: `crates/mendrel-diagnostics/src/lib.rs`
- Create: `crates/mendrel-diagnostics/tests/render.rs`

**Interfaces:**
- Produces: `ByteSpan::new(start, end)`, `SourceFile::from_bytes(path, bytes)`, `SourceFile::position(byte)`, `Diagnostic::from_catalog(entry, span)`, `render_human`, and `render_jsonl`.
- Consumes: normalized explicit paths and source bytes; no ambient working-directory lookup.

- [ ] **Step 1: Write failing source tests**

Cover a multibyte source containing CRLF and assert byte offsets, zero-based line numbers, and UTF-16 columns. Cover invalid UTF-8 and lexical path normalization of `a/./b/../c.mnd` to `a/c.mnd`.

- [ ] **Step 2: Run the source test red**

Run: `cargo test -p mendrel-source --test source`

Expected: compilation failure because `ByteSpan` and `SourceFile` do not exist.

- [ ] **Step 3: Implement source types**

Use these public signatures:

```rust
pub struct ByteSpan { pub start: u32, pub end: u32 }
pub struct Position { pub byte: u32, pub line: u32, pub column_utf16: u32 }
pub struct SourceFile { /* private text, path, and line starts */ }
pub enum SourceError { InvalidUtf8 { valid_up_to: usize }, SpanOutOfBounds { byte: usize } }
impl SourceFile {
    pub fn from_bytes(path: impl Into<String>, bytes: Vec<u8>) -> Result<Self, SourceError>;
    pub fn text(&self) -> &str;
    pub fn path(&self) -> &str;
    pub fn position(&self, byte: u32) -> Result<Position, SourceError>;
}
```

- [ ] **Step 4: Write failing diagnostic renderer tests**

Construct one missing-token diagnostic and assert catalog code, primary byte span, human summary, required JSON fields, deterministic repeat output, and JSON escaping for a path containing quotes.

- [ ] **Step 5: Implement the diagnostic catalog and renderers**

Start the catalog with `E-SOURCE-UTF8-0001`, `E-SYNTAX-INVALID-0001`, `E-SYNTAX-MISSING-0001`, and `E-SYNTAX-UNSUPPORTED-0001`. Every diagnostic contains one root cause node, an explicit empty fix list when no fix exists, and an `origin` with component and phase.

- [ ] **Step 6: Verify Task 2**

Run: `cargo test -p mendrel-source -p mendrel-diagnostics`

Expected: all source and diagnostic tests pass without warnings.

### Task 3: Generated grammar inventory, tokens, and immutable CST

**Files:**
- Create: `crates/mendrel-syntax/Cargo.toml`
- Create: `crates/mendrel-syntax/src/generated.rs`
- Create: `crates/mendrel-syntax/src/generate.rs`
- Create: `crates/mendrel-syntax/src/cst.rs`
- Create: `crates/mendrel-syntax/src/lib.rs`
- Create: `crates/mendrel-syntax/tests/inventory.rs`
- Modify: `xtask/src/main.rs`

**Interfaces:**
- Produces: `generate::render_inventory(grammar)`, `generate::check_inventory(root)`, `Token`, `TokenKind`, `SyntaxKind`, `SyntaxNode`, `SyntaxElement`, `SyntaxTree::source_text`, and `SyntaxTree::structural_fingerprint`.
- Consumes: quoted literals and production names extracted from `spec/grammar.ebnf` after block comments are removed.

- [ ] **Step 1: Write failing inventory tests**

Assert that the derived inventory contains `module`, `pub`, `fn`, `;`, `->`, and productions `source_file`, `function_decl`, and `additive_expression`, with stable sorted output independent of source order.

- [ ] **Step 2: Run the inventory test red**

Run: `cargo test -p mendrel-syntax --test inventory`

Expected: compilation failure because the generator and CST types do not exist.

- [ ] **Step 3: Implement grammar extraction and generated output**

The generator must fail on unterminated EBNF comments or quoted literals. `generated.rs` is the only compiled keyword/punctuation/production inventory; parser code may reference generated `SyntaxKind` variants but may not copy the inventory into a second table.

- [ ] **Step 4: Implement lossless CST storage**

`SyntaxElement` is `Node(SyntaxNode)` or `Token(Token)`. Tokens retain exact source text and byte span; missing tokens have empty text and a zero-width span. `source_text()` concatenates all non-missing token text. `structural_fingerprint()` includes node kinds and non-trivia token kind/text but excludes spans and trivia.

- [ ] **Step 5: Generate and verify the inventory**

Run: `cargo run -p xtask -- generated`

Run: `cargo run -p xtask -- generated --check`

Expected: both succeed, and a deliberate in-memory mismatch is rejected by the unit test.

### Task 4: Lossless lexer and first parser slice with recovery

**Files:**
- Create: `crates/mendrel-parser/Cargo.toml`
- Create: `crates/mendrel-parser/src/lexer.rs`
- Create: `crates/mendrel-parser/src/parser.rs`
- Create: `crates/mendrel-parser/src/lib.rs`
- Create: `crates/mendrel-parser/tests/lexer.rs`
- Create: `crates/mendrel-parser/tests/first_slice.rs`
- Create: `crates/mendrel-parser/tests/fixtures/first_slice.mnd`
- Create: `crates/mendrel-parser/tests/fixtures/first_slice.cst`
- Create: `crates/mendrel-parser/tests/fixtures/missing_semicolon.mnd`
- Create: `crates/mendrel-parser/tests/fixtures/missing_semicolon.jsonl`

**Interfaces:**
- Produces: `lex(&SourceFile) -> LexResult` and `parse(&SourceFile) -> ParseResult`, where both return retained tokens/tree plus diagnostics.
- Consumes: generated keyword/punctuation inventory, source spans, CST types, and diagnostic catalog entries.

- [ ] **Step 1: Write failing lexer coverage tests**

Assert exact token-span coverage for the first slice, preservation of whitespace and nested block comments, invalid-tab diagnostics outside strings, and no panic for representative arbitrary Unicode strings.

- [ ] **Step 2: Run lexer tests red, implement the lexer, and rerun green**

Run: `cargo test -p mendrel-parser --test lexer`

Lex identifiers by Unicode scalar boundaries, recognize grammar-derived keywords and longest-match punctuation, preserve unknown scalars as invalid tokens, and append EOF at `source.len()`.

- [ ] **Step 3: Write failing valid-slice and recovery tests**

Assert the supplied sample reconstructs byte-for-byte, contains module/function/parameter/block/additive nodes, and has zero diagnostics. For missing `;`, `}`, type, and identifier variants, assert one local root diagnostic, a zero-width missing token, an error-tolerant root tree, and retained trailing source.

- [ ] **Step 4: Implement recursive descent plus Pratt parsing**

Parse only module declarations and function declarations needed by the first slice. Unsupported top-level syntax is wrapped in an error node and emits `E-SYNTAX-UNSUPPORTED-0001`; it is never accepted without a diagnostic. Synchronize at `;`, `}`, `pub`, `internal`, `fn`, and EOF.

- [ ] **Step 5: Verify Task 4**

Run: `cargo test -p mendrel-parser`

Expected: lexer, valid CST golden, and all four malformed recovery cases pass.

### Task 5: CST-only canonical formatter and round-trip properties

**Files:**
- Create: `crates/mendrel-format/Cargo.toml`
- Create: `crates/mendrel-format/src/lib.rs`
- Create: `crates/mendrel-format/tests/format.rs`
- Create: `crates/mendrel-format/tests/fixtures/first_slice.formatted.mnd`

**Interfaces:**
- Produces: `format(tree: &SyntaxTree) -> Result<String, FormatError>`.
- Consumes: only CST nodes/tokens and parse diagnostics; no semantic AST/HIR.

- [ ] **Step 1: Write failing golden and property tests**

Assert exact canonical output for the first slice, comment text/order preservation, idempotence across a deterministic whitespace corpus, and equality of structural fingerprints before and after formatting/reparsing.

- [ ] **Step 2: Run formatter tests red**

Run: `cargo test -p mendrel-format --test format`

Expected: compilation failure because `format` does not exist.

- [ ] **Step 3: Implement the minimum deterministic layout**

Use four spaces, LF, one blank line after the module declaration, spaces around `->` and binary operators, no spaces around `.`, no space before punctuation, and a final newline. Refuse trees with recovery elements using `FormatError::MalformedTree`; no destructive full-file formatting of malformed source.

- [ ] **Step 4: Verify Task 5**

Run: `cargo test -p mendrel-format`

Expected: golden, idempotence, comment preservation, and parse-equivalence tests pass.

### Task 6: CLI, CST/diagnostic goldens, and command determinism

**Files:**
- Create: `crates/mendrelc/Cargo.toml`
- Create: `crates/mendrelc/src/lib.rs`
- Create: `crates/mendrelc/src/main.rs`
- Create: `crates/mendrelc/tests/cli.rs`
- Create: `crates/mendrel-cli/Cargo.toml`
- Create: `crates/mendrel-cli/src/main.rs`

**Interfaces:**
- Produces: `mendrelc --version`, `mendrelc check`, `mendrelc cst`, `mendrelc fmt`, and equivalent `mendrel` subcommands plus `mendrel xtask generated --check`.
- Consumes: source loading, parser, formatter, diagnostic rendering, and generated inventory check.

- [ ] **Step 1: Write failing CLI tests**

Use `std::process::Command` and Cargo-provided binary paths to assert version output, valid check exit 0, malformed JSONL exit 1, CST dump stability, formatter stdout, unknown option exit 2, and byte-identical repeated output.

- [ ] **Step 2: Run CLI tests red**

Run: `cargo test -p mendrelc --test cli`

Expected: failure because the binaries do not exist.

- [ ] **Step 3: Implement the shared driver and thin frontend**

`mendrelc::run(args, cwd, stdout, stderr)` receives the working path explicitly from `main`; compiler results receive normalized user paths and never call `current_dir`. JSON error mode writes one schema-shaped object per line with no human preamble.

- [ ] **Step 4: Verify Task 6**

Run: `cargo test -p mendrelc -p mendrel-cli`

Run: `cargo run -p mendrelc -- --version`

Run: `cargo run -p mendrelc -- check crates/mendrel-parser/tests/fixtures/first_slice.mnd --error-format=json`

Expected: all tests pass; version and valid check succeed deterministically.

### Task 7: Fuzz entry, local verification, and completion evidence

**Files:**
- Create: `fuzz/Cargo.toml`
- Create: `fuzz/fuzz_targets/parser.rs`
- Modify: `xtask/src/main.rs`
- Modify: `README.md`

**Interfaces:**
- Produces: a cargo-fuzz target that accepts arbitrary bytes without parser panic and an `xtask verify` command covering generated output, formatting, lint, tests, and strict pack validation.
- Consumes: `SourceFile::from_bytes`, `parse`, and `format`.

- [ ] **Step 1: Add the fuzz target**

For valid UTF-8, parse and reconstruct the source; when the tree is format-safe, format, reparse, and assert structural equivalence. For invalid UTF-8, assert source loading returns an error rather than panicking.

- [ ] **Step 2: Implement local verification orchestration**

`cargo run -p xtask -- verify` runs, in order, generated check, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and `python scripts/validate_pack.py --strict-schema`. It stops at the first failure and reports the exact child command.

- [ ] **Step 3: Document only implemented commands and limits**

Add a bootstrap section to `README.md` listing the two CLIs, exact verification command, supported first-slice syntax, diagnostic behavior, and explicit Phase 2+ exclusions.

- [ ] **Step 4: Run fresh completion verification**

Run: `cargo run -p xtask -- verify`

Run: `cargo build --workspace --release`

If cargo-fuzz is installed, run: `cargo fuzz run parser -- -runs=1000`

Expected: workspace verification and release build pass; fuzz status is reported separately if the cargo-fuzz executable is unavailable.

- [ ] **Step 5: Audit public surface**

List every newly public Rust item and justify it by a cross-crate consumer or integration-test need. Report diagnostic codes added, and report API/effect/wire/unsafe surface deltas; effect/wire/unsafe must remain zero.
