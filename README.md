# Mendrel

> **Know what code can do. Know what a change can break.**

Mendrel is an experimental, statically typed programming language for production software maintained by humans and coding agents together. It makes authority, failure, resource lifetime, and change impact part of the program's contract, so a maintainer can act with less context and less guesswork.

The goal is not to make code generation easier by making syntax shorter. The goal is to make real repository maintenance safer: understand a change quickly, preview its blast radius, apply it mechanically, and reject it when the evidence is stale.

Mendrel is currently in an early bootstrap stage. The repository contains an executable syntax frontend and a detailed language design, but not yet a type checker, runtime, code generator, or agent protocol server. It is not ready for production use.

## One contract instead of scattered conventions

Consider a checkout operation. In many systems, its real contract is spread across source code, dependency-injection setup, error conventions, async runtime behavior, deployment permissions, tests, and operational documentation.

Mendrel intends to put that contract in one place.

> **Design target:** The example below describes the intended language. The current bootstrap compiler does not accept this full syntax yet.

```mendrel
pub async fn checkout(
    request: CheckoutRequest,
    idempotency_key: IdempotencyKey,
) -> Result<CheckoutReceipt, CheckoutError>
uses {
    orders: OrdersRepo,
    payments: PaymentGateway,
    idempotency: IdempotencyStore,
    clock: Clock,
}
contract {
    requires request.total.amount > 0;
}
{
    use lease = idempotency
        .claim(idempotency_key)
        .await
        .map_error(CheckoutError.from_storage)?;

    let maybe_order = orders
        .find(request.order_id)
        .await
        .map_error(CheckoutError.from_storage)?;

    let order = match maybe_order {
        Some(value) => value,
        None => return Err(CheckoutError.OrderNotFound {
            order_id: request.order_id,
        }),
    };

    let authorization = within 2.seconds {
        payments
            .authorize(request)
            .await
            .map_error(CheckoutError.from_payment)
    }?;

    let receipt = CheckoutReceipt.create(order, authorization, clock.now());

    lease
        .complete(receipt)
        .await
        .map_error(CheckoutError.from_storage)?;

    Ok(receipt)
}
```

The surface syntax is not the main advantage. The value is what the declaration makes available to every reader and tool.

### What a human gets

- **Review without reading the whole application.** The signature identifies inputs, output, failures, asynchronous behavior, external authority, and the precondition before the body is inspected.
- **No hidden authority.** The function can use the order store, payment gateway, idempotency store, and clock under those exact labels. Access to an unrelated database, secret, or network client is not ambient.
- **A complete test boundary.** The `uses` row states exactly which capabilities a test must replace. Time, randomness, storage, and networking do not hide behind a global application context.
- **Visible resource and time behavior.** `use lease` gives the idempotency lease a checked cleanup path. `within 2.seconds` makes the payment deadline part of program semantics rather than an informal convention.
- **Reviewable operational impact.** Capability growth, public errors, API changes, and wire-schema changes are intended to be classified before publication, using the same semantic facts as the compiler.

### What an AI coding agent gets

- **Task-specific context instead of a repository dump.** The planned Mendrel Agent Protocol (MAP) can select the relevant declarations, callers, tests, type/effect facts, and source evidence for a maintenance task.
- **Structured failures instead of log scraping.** Diagnostics carry stable codes, primary spans, cause graphs, expected and actual semantic facts, and revision-bound repair suggestions.
- **Constrained completion instead of symbol guessing.** Typed holes can expose candidates that satisfy the expected type, effect row, resource state, and required conversions.
- **Blast-radius preview before editing.** A semantic change can report affected callers, tests, public API, capability surface, wire schema, and unsafe dependencies before it is committed.
- **Protection from stale context.** MAP edits follow `plan -> preview -> commit`; the commit is rejected when the workspace revision no longer matches the preview.

## A change should explain its consequences

A future MAP change preview is intended to answer questions like these before source files are modified:

```text
Change: add `fraud: FraudCheck` to `checkout`

Affected callers:       3
Affected tests:         5
Public API impact:      source-breaking
Capability impact:      authority grows (breaking)
Wire-schema impact:     none
Unsafe impact:          none
Base revision:          sha256:7f8c...
```

The preview is not a second source format. Mendrel keeps canonical text as the source of truth and produces an ordinary patch for human review. Semantic IDs remain snapshot-local, and a stale preview cannot silently overwrite newer work.

## Quick start

The executable bootstrap currently accepts a deliberately small syntax slice:

```mendrel
module demo.main;

pub fn add(left: I32, right: I32) -> I32 {
    left + right
}
```

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

## What exists today

Version `0.0.1` provides the first Phase 0/Phase 1 vertical slice:

- UTF-8 source handling with byte-based spans and content revisions;
- stable human-readable and JSONL diagnostics;
- tokenization that preserves whitespace, comments, nested block comments, and invalid tokens;
- an error-tolerant, lossless CST with explicit recovery elements;
- canonical formatting for well-formed input;
- explicit rejection of syntax outside the implemented subset; and
- command-line entry points for syntax checking, CST inspection, formatting, and repository verification.

The current implementation does **not** include name resolution, type or effect checking, HIR or MIR, evaluation, a runtime, native or WebAssembly code generation, package management, or MAP.

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

The README focuses on what Mendrel offers, why it exists, and how to try the current bootstrap. The normative language design begins with the [executive decision](docs/00-executive-decision.md) and the [language reference](docs/01-language-reference.md). Detailed compiler contracts, formal obligations, schemas, implementation boundaries, and validation notes remain in the internal design pack under [`docs/`](docs/).

## License

Mendrel is licensed under the Apache License 2.0.
