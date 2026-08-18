# Mendrel

> **Make effects visible. Make change local. Make repair mechanical.**

Mendrel is an experimental, statically typed programming language for software that must remain understandable and maintainable as it grows. It is designed for production services, command-line tools, data processing, and other long-lived systems built by humans and coding agents together.

Mendrel is currently in an early bootstrap stage. The repository contains an executable syntax frontend and a detailed language design, but it does not yet contain a type checker, runtime, or code generator. It is not ready for production use.

## Why Mendrel exists

Writing a new function is often the easy part of software development. The difficult part is understanding what the function depends on, what authority it exercises, what can fail, which resources it owns, and what else may break when it changes.

Mendrel aims to reduce that maintenance burden. Its design focuses on three outcomes:

1. less context to understand before making a change;
2. a smaller blast radius after making that change; and
3. a shorter path from a failure to its cause and repair.

This is also what Mendrel means by being friendly to coding agents. The goal is not merely to use simple syntax or produce short programs. The goal is to make dependencies, constraints, and consequences explicit enough that both people and tools can reason about them reliably.

## The language in one minute

Mendrel brings several ideas into one semantic model:

- **Explicit authority.** External access such as time, networking, databases, secrets, and process execution appears as named capabilities rather than ambient global state.
- **Local reasoning.** Public declarations carry explicit contracts, while inference stays within declaration boundaries.
- **Managed values and affine resources.** Ordinary application data uses managed memory. Resources whose identity and lifetime matter—such as files, sockets, locks, and transactions—are tracked separately.
- **Structured concurrency.** Tasks belong to lexical scopes, with cancellation, deadlines, failure, and cleanup following the same lifetime structure.
- **Canonical source.** Text remains the source of truth, with one canonical formatter and a lossless concrete syntax tree that preserves comments and recovery information.
- **Structured feedback.** Diagnostics, semantic queries, and repair plans are intended to be stable, machine-readable, and revision-aware.
- **Production semantics.** Reproducible builds, compatibility checks, observability, provenance, and deployment authority are part of the language and toolchain contract rather than unrelated add-ons.

The name *Mendrel* combines **mend** with a suffix suggesting relation, relevance, and locality: a language intended to make software easier to understand, change, and repair.

## A small Mendrel program

The current bootstrap accepts a deliberately small language slice:

```mendrel
module demo.main;

pub fn add(left: I32, right: I32) -> I32 {
    left + right
}
```

This example already passes through the lossless parser, syntax diagnostics, CST inspection, and canonical formatter. The broader language design includes algebraic data types, explicit errors, capabilities and effect rows, affine resources, contracts, and structured concurrency, but those features are not implemented yet.

## Quick start

The repository pins its Rust toolchain through `rust-toolchain.toml`. With Rust and Git available:

```sh
git clone https://github.com/kmizu/mendrel.git
cd mendrel

cargo run -p mendrel-cli -- --version
cargo run -p mendrel-cli -- check crates/mendrel-parser/tests/fixtures/first_slice.mnd
cargo run -p mendrel-cli -- cst crates/mendrel-parser/tests/fixtures/first_slice.mnd
cargo run -p mendrel-cli -- fmt crates/mendrel-parser/tests/fixtures/first_slice.mnd
```

`check` currently validates only the implemented syntax slice. `fmt` writes canonical source to standard output and does not modify the input file.

To run the complete repository verification:

```sh
cargo run -p xtask -- verify
```

The verification suite checks generated syntax data, formatting, lint cleanliness, workspace tests, and the consistency of the design pack.

## Current status

Version `0.0.1` provides the first Phase 0/Phase 1 vertical slice:

- UTF-8 source handling with byte-based spans and content revisions;
- stable human-readable and JSONL diagnostics;
- tokenization with preserved whitespace, comments, nested block comments, and invalid tokens;
- an error-tolerant, lossless CST with explicit recovery elements;
- canonical formatting for well-formed input;
- explicit rejection of syntax outside the implemented subset; and
- command-line entry points for syntax checking, CST inspection, formatting, and repository verification.

The current implementation does **not** include name resolution, type or effect checking, HIR or MIR, evaluation, a runtime, native or WebAssembly code generation, package management, or the Mendrel Agent Protocol server.

The next milestones are tracked in the [implementation roadmap](docs/07-roadmap-and-acceptance.md).

## Intended use

Mendrel is being designed primarily for:

- backend services;
- command-line tools, batch jobs, and data pipelines;
- WebAssembly components at explicit trust boundaries;
- long-lived business logic; and
- medium-to-large repositories maintained by people and coding agents together.

It is not intended to begin as a hard real-time language, an operating-system or device-driver language, a GPU language, a proof assistant, or a seamless replacement for existing C++ systems.

## Design documents

The README intentionally stays focused on what Mendrel is, why it exists, and how to try the current bootstrap. The normative language design begins with the [executive decision](docs/00-executive-decision.md) and the [language reference](docs/01-language-reference.md). Detailed compiler contracts, formal obligations, schemas, implementation boundaries, and validation notes remain in the internal design pack under [`docs/`](docs/).

## License

Mendrel is licensed under the Apache License 2.0.
