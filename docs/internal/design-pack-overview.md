# Internal design pack overview

This document preserves implementation-facing context that does not belong on the user-facing project page. The normative requirements remain in the dedicated documents linked below.

## Project identity

- Canonical source form: UTF-8 text with the `.mnd` extension.
- Primary CLI name: `mendrel`.
- Reference compiler implementation language: Rust.
- Initial intended code-generation targets: LLVM native and the WebAssembly Component Model.
- Current design-pack revision: draft 0.2, dated 2026-08-18.

The name *Mendrel* is a coined term derived from *mend* and a suffix suggesting relation, relevance, and locality. A preliminary collision search was performed on 2026-08-18, but this is not legal clearance for trademarks, company names, package registries, or domains.

## Document map

| Document | Responsibility |
|---|---|
| `docs/00-executive-decision.md` | Problem statement, alternatives, and the selected direction |
| `docs/01-language-reference.md` | Surface language and declarations |
| `docs/02-types-effects-capabilities.md` | Types, effects, capabilities, errors, and contracts |
| `docs/03-runtime-concurrency-memory.md` | Memory, resources, concurrency, runtime, and FFI |
| `docs/04-production-toolchain.md` | Packages, builds, operations, and supply-chain requirements |
| `docs/05-agent-protocol.md` | MAP, diagnostics, typed holes, and semantic edits |
| `docs/06-compiler-architecture.md` | Compiler and runtime boundaries and sources of truth |
| `docs/07-roadmap-and-acceptance.md` | Implementation phases and acceptance criteria |
| `docs/08-conformance-and-benchmarks.md` | Conformance, fuzzing, and MendrelBench |
| `docs/09-adrs-risks-nongoals.md` | ADRs, risks, rejected alternatives, and kill criteria |
| `docs/10-formal-kernel.md` | Normative semantic kernel and soundness obligations |
| `docs/11-security-threat-model.md` | Threat model and trust boundaries |
| `docs/12-references.md` | Primary references and design lineage |
| `docs/13-derived-layers-and-lineage.md` | Boundaries for derived language layers |
| `spec/grammar.ebnf` | Machine-readable v0.1 grammar skeleton |
| `schemas/diagnostic-v1.schema.json` | Diagnostic JSON Schema |
| `schemas/map-v1.schema.json` | MAP envelope JSON Schema |
| `VALIDATION.md` | Design-pack validation and deliberate limits |

`AGENTS.md` defines repository-wide implementation discipline. `PROMPT_FOR_CODEX.md` defines the bootstrap task and its scope.

## Normative order

The design pack uses **MUST**, **MUST NOT**, **SHOULD**, and **MAY** in their conventional normative senses. When documents conflict, the precedence is:

1. `docs/10-formal-kernel.md`;
2. `docs/01-language-reference.md` through `docs/05-agent-protocol.md`;
3. `docs/11-security-threat-model.md`;
4. `docs/06-compiler-architecture.md` through `docs/09-adrs-risks-nongoals.md`;
5. examples and explanatory code.

Conflicts must be resolved explicitly through an ADR rather than by silently selecting the more convenient interpretation.

## Bootstrap implementation boundary

The current parser subset accepts a module declaration; direct, grouped, and aliased import declarations; and public function declarations with typed identifier parameters, an explicit return type, and a trailing expression made from identifiers, integer, string, boolean, `Unit`, and `None` literal expressions, the additive operators `+` and `-`, the multiplicative operators `*`, `/`, and `%`, grouping parentheses, and chained function calls. Grouped imports accept item aliases and an optional trailing comma. Calls may have zero or more positional or named arguments and an optional trailing comma.

Qualified expression and type paths, unary operators, empty parameter lists, empty bodies, omitted or `internal` visibility, `async`, `unsafe`, and `move` parameters are outside the current slice. Literal lexical forms whose shapes or escape rules remain unspecified by the design pack are also outside this slice. They are rejected with `E-SYNTAX-UNSUPPORTED-0001` rather than accepted with provisional semantics.

The lexer preserves trivia, nested block comments, and invalid tokens. The parser preserves missing tokens and unsupported regions as CST recovery elements. The formatter canonicalizes only recovery-free trees and does not rewrite malformed input.

The bootstrap does not define AST/HIR, name resolution, type/effect/resource checking, MIR, a runtime, a backend, package tooling, or a MAP server.

## Validation contract

The design pack is checked with:

```sh
python scripts/validate_pack.py --strict-schema
```

The Rust workspace exposes its complete local verification through:

```sh
cargo run -p xtask -- verify
```

That command checks generated syntax inventory, Rust formatting, Clippy with warnings denied, workspace tests, and strict design-pack validation.

The implementation remains intentionally phase-bound. Unsupported language features must be diagnosed rather than silently accepted, and later phases must not be represented by speculative public APIs or empty subsystem scaffolding.
