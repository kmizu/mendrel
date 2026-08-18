# Mendrel Literal Expressions Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the fully specified literal forms already recognized by the lexer to Phase 1 primary expressions without inventing lexical rules for the remaining literal classes.

**Architecture:** Keep literal recognition in the recursive-descent parser and retain the existing lossless token stream. Wrap integer and string tokens in their generated leaf production nodes, keep keyword literals directly under `Literal`, and let the CST-only formatter consume the unchanged tokens.

**Tech Stack:** Rust 1.96.0, the existing standard-library-only Mendrel parser/formatter crates, and the generated syntax inventory from `spec/grammar.ebnf`.

**Spec:** `PROMPT_FOR_CODEX.md`, `spec/grammar.ebnf` productions `literal` and `primary_expression`, `docs/01-language-reference.md` sections 9.6 and 10, and `docs/07-roadmap-and-acceptance.md` Phase 1.

## Global Constraints

- Text source remains canonical and every non-missing token must be retained byte-for-byte in the CST.
- The formatter must remain CST-only, deterministic, parse-preserving, and idempotent.
- `integer_literal`, backslash-free `string_literal`, `true`, `false`, `Unit`, and `None` are in scope because those token shapes are already specified or generated.
- A string token containing `\` is retained in a `Literal` recovery subtree and rejected once with `E-SYNTAX-UNSUPPORTED-0001`; the design pack requires escape diagnostics but does not yet define the allowed escape set.
- `float_literal`, `decimal_literal`, `char_literal`, `byte_string_literal`, and `duration_literal` stay rejected until their lexical shapes and escape rules are specified.
- No public Rust API, diagnostic code, dependency, effect, wire, or unsafe surface is added.

---

### Task 1: Parse and format specified literal primary expressions

**Files:**
- Modify: `crates/mendrel-parser/src/parser.rs`
- Modify: `crates/mendrel-parser/tests/first_slice.rs`
- Modify: `crates/mendrel-format/tests/format.rs`
- Modify: `docs/internal/design-pack-overview.md`
- Modify: `MANIFEST.md`

**Interfaces:**
- Consumes: existing `TokenKind::{Integer, String, Keyword}`, `SyntaxKind::{Literal, IntegerLiteral, StringLiteral}`, and `Parser::parse_primary_expression`.
- Produces: lossless `PrimaryExpression -> Literal` CSTs for integer, string, `true`, `false`, `Unit`, and `None`.

- [x] **Step 1: Write failing parser tests**

Add one source containing six functions whose final expressions are the six supported literal forms. Assert zero diagnostics, exact source reconstruction, six `Literal` nodes, one `IntegerLiteral`, one `StringLiteral`, and no recovery. Add a second regression with `"line\\n"` and assert exact source reconstruction, one `Literal` containing an `Error`, no missing token, and exactly one `E-SYNTAX-UNSUPPORTED-0001` whose actual field is the full string token. Pin the normative literal production:

```rust
assert_eq!(
    production_rule("literal"),
    Some("integer_literal | float_literal | decimal_literal | char_literal | string_literal | byte_string_literal | duration_literal | \"true\" | \"false\" | \"Unit\" | \"None\"")
);
```

- [x] **Step 2: Run the parser test and preserve the red result**

Run:

```sh
cargo test -p mendrel-parser --test first_slice parses_specified_literal_primary_expressions -- --exact
```

Expected before implementation: FAIL because the first literal is diagnosed as unsupported block syntax.

- [x] **Step 3: Implement literal recognition and CST construction**

Add a parser predicate that accepts integer/string token kinds plus the four literal keywords. Parse the leaf without changing token text or span; reject a string containing a backslash as an explicit unsupported recovery node because the normative escape set is not defined:

```rust
fn parse_literal(&mut self) -> SyntaxNode {
    let mut literal_children = Vec::new();
    let leaf_kind = match self.current_significant() {
        Some(token) if token.kind == TokenKind::Integer => Some(SyntaxKind::IntegerLiteral),
        Some(token) if token.kind == TokenKind::String => Some(SyntaxKind::StringLiteral),
        _ => None,
    };
    if let Some(kind) = leaf_kind {
        let mut leaf_children = Vec::new();
        if kind == SyntaxKind::StringLiteral
            && self.current_significant().is_some_and(|token| token.text.contains('\\'))
        {
            leaf_children.push(SyntaxElement::Node(self.reject_current_token(
                "string escapes are outside the implemented Phase 1 subset",
            )));
        } else {
            self.bump_expected(&mut leaf_children);
        }
        literal_children.push(SyntaxElement::Node(SyntaxNode::new(kind, leaf_children)));
    } else {
        self.bump_expected(&mut literal_children);
    }
    SyntaxNode::new(SyntaxKind::Literal, literal_children)
}
```

Route `parse_primary_expression` through this function before path/grouping alternatives, and include the same predicate in `at_expression_start`.

- [x] **Step 4: Run focused parser tests green**

Run:

```sh
cargo test -p mendrel-parser --test first_slice parses_specified_literal_primary_expressions -- --exact
cargo test -p mendrel-parser --test first_slice rejects_string_escapes_until_the_escape_set_is_specified -- --exact
cargo test -p mendrel-parser --test first_slice implemented_production_shapes_are_pinned_to_the_normative_grammar -- --exact
```

Expected: PASS with lossless literal CST structure.

- [x] **Step 5: Add formatter round-trip coverage**

Add a compact literal source and assert exact canonical output, equal structural fingerprints after reparse, and a second format equal to the first:

```rust
let expected = concat!(
    "module demo.main;\n\n",
    "pub fn literal(value: I32) -> I32 {\n",
    "    choose(1, text: \"value\", true, false, Unit, None)\n",
    "}\n",
);
```

Run:

```sh
cargo test -p mendrel-format --test format formats_literal_expressions_without_changing_structure -- --exact
```

Expected: PASS.

- [x] **Step 6: Update the internal implementation boundary**

State in `docs/internal/design-pack-overview.md` that integer, string, boolean, `Unit`, and `None` literal expressions are implemented, while the lexical forms left unspecified by the design pack remain outside the slice. Refresh only that file's size and SHA-256 row in `MANIFEST.md`.

- [x] **Step 7: Verify, fuzz, and commit**

Run:

```sh
cargo run -p xtask -- verify
cargo build --manifest-path fuzz/Cargo.toml --bin parser
fuzz/target/debug/parser -runs=1000
```

Expected: full verification passes; the fuzz smoke completes without a crash or source-reconstruction failure.

Commit:

```sh
git add crates/mendrel-parser/src/parser.rs \
  crates/mendrel-parser/tests/first_slice.rs \
  crates/mendrel-format/tests/format.rs \
  docs/internal/design-pack-overview.md MANIFEST.md \
  docs/superpowers/plans/2026-08-19-mendrel-literal-expressions.md
git commit -m "Parse specified literal expressions"
```
