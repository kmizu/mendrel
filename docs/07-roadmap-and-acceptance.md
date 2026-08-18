# 07. Implementation roadmap and acceptance criteria

## 1. Roadmap principle

Mendrel は巨大な「compiler の骨格」を先に作らない。各 phase は、利用者が実際に確認できる薄い end-to-end slice を完成させる。

各 phase の完了条件:

- 仕様 section が明示される
- 成功/失敗 example
- structured diagnostic
- golden/property/fuzz のいずれか
- CLI から実行可能
- 次 phase が依存できる public boundary
- 計測値
- 未実装 syntax を silent accept しない
- 文書と generated table の整合
- 完了 verification の実行

phase 間で「一時的に safe code が unsafe」「release だけ意味が違う」状態を許さない。未実装機能は diagnostic で拒否する。

## 2. Phase 0 — Repository and diagnostic substrate

### Scope

- Rust workspace
- source database
- UTF-8 byte span、line index
- `mendrel`/`mendrelc` minimal CLI
- diagnostic catalog/model
- human + JSONL renderer
- test/golden harness
- deterministic path normalization
- generated asset check
- CI skeleton

### Acceptance

```sh
mendrelc --version
mendrelc check empty.mnd --error-format=json
```

invalid UTF-8/file load error が stable diagnostic を返す。

minimum tests:

- span conversion
- line ending
- path normalization
- diagnostic JSON schema
- human/structured semantic consistency
- deterministic output ordering
- arbitrary file bytes no compiler panic

### Exit evidence

- command output fixture
- diagnostic schema validation
- no ambient wall clock/current-dir dependency in result
- build/test command documented

## 3. Phase 1 — Lexer, lossless CST, parser, formatter

### Scope

- token/trivia
- comments
- identifier/literal
- module/import
- record/enum
- function signature/body
- block/let/return/call/basic expression
- error recovery
- CST dump
- canonical formatter
- incremental reparse basic
- parser/formatter fuzz

### First vertical slice

```mendrel
module demo.main;

pub fn add(left: I32, right: I32) -> I32 {
    left + right
}
```

### Acceptance

- valid sample parse
- missing `;`, `}`, type、identifier で local diagnostic
- comments preserved
- `format(format(x)) == format(x)`
- parse-equivalence after format
- incremental parse equals full parse
- arbitrary UTF-8 no panic
- syntax kind/keyword generated table check
- example source canonical

### Performance gate

暫定 benchmark を固定:

- full parse throughput
- 1-character edit latency
- formatter throughput
- CST memory/byte

数値目標は最初の実測 baseline を置いた後に決める。根拠のない絶対値を仕様にしない。

## 4. Phase 2 — Modules, names, nominal types, ADT

### Scope

- package-local module graph
- import/visibility
- symbol collection
- lexical resolution
- primitive type
- record/enum/newtype
- function type
- local inference
- explicit public signature
- pattern matching/exhaustiveness
- no shadowing
- basic trait declarationなし

### Acceptance examples

- cross-file import
- unknown/ambiguous symbol diagnostic
- transitive dependency import rejection
- private type leakage
- local type inference
- public signature omission rejection
- exhaustive/non-exhaustive/unreachable match
- closed public enum wildcard rejection
- newtype implicit conversion rejection
- module cycle report

### MAP/LSP substrate

- symbol search/get/definition/reference の core query
- まだ protocol server は不要
- semantic ID + revision model test

## 5. Phase 3 — Trait, Result, capability/effect rows

### Scope

- trait/impl/coherence/orphan
- associated type
- static/dynamic dispatch minimum
- `Option`/`Result` and `?`
- capability declaration/value
- named `uses` row
- effect inference inside body
- row subset/polymorphism
- explicit capability remap
- error enum metadata
- no general handler

### Acceptance

- exact label/type forwarding
- same type/different label requires remap
- undeclared capability error with trace
- pure function empty effect
- higher-order effect polymorphic `map`
- effect surface snapshot/diff
- trait ambiguity rejected
- overlapping/orphan impl rejected
- error conversion chain report
- `?` lowering/evaluator behavior
- capability fake generation sketch or minimal implementation

### Formal gate

`docs/10-formal-kernel.md` の type/effect judgments に対応する executable rule inventory を作る。各 rule は positive/negative conformance test を持つ。

## 6. Phase 4 — Reference evaluator and contracts

### Scope

- typed HIR evaluator
- record/enum/function/call/control flow
- Result/error
- capability host adapter
- contract pure subset
- constrained newtype
- doctest
- deterministic value rendering

### Acceptance

- language examples execute
- evaluator panic/error distinction
- contract success/failure
- impure/nonterminating contract rejection
- literal constraint compile-time check
- host fake capability
- evaluator deterministic test
- evaluator trace/source mapping
- no native backend claim yet

### Reason

この phase で semantics を高速に反復し、LLVM/runtime bug と frontend semantics を分離する。

## 7. Phase 5 — MIR, verifier, LLVM spike, simple managed runtime

### Scope

- CFG MIR
- checked arithmetic
- aggregate representation
- function/call
- GC allocation/root
- panic/report
- basic LLVM AOT
- precise simple STW collector
- native executable
- debug info
- evaluator/native differential

### Acceptance

- representative Phase 4 programs native execute
- integer overflow parity
- bounds/contract/panic parity
- GC root stress
- moving/nonmoving decision documented; if nonmoving MVP, source semantics still moving-safe
- MIR verifier catches malformed IR fixtures
- debug/release semantic parity
- ASan/UBSan on runtime/FFI shim
- deterministic object/binary within declared environment or reproducibility diff documented

### Architecture review gate

production collector へ進む前に:

- object layout
- stack map
- runtime ABI
- panic model
- async state-machine feasibility
- Wasm representation

を ADR で凍結する。ただし optimization detail は凍結しない。

## 8. Phase 6 — Resource and unsafe/FFI

### Scope

- affine resource
- move/borrow
- `use`
- cleanup edges
- resource state verifier
- `unsafe` block/module
- C FFI minimal
- pinned resource
- unsafe artifact metadata

### Acceptance

- double use/use-after-move rejection
- all exits cleanup
- return/error/panic cleanup
- cleanup failure aggregation
- borrow escape rejection
- FFI ownership fixtures
- sanitizer profile
- `mendrel unsafe tree`
- no finalizer
- resource leak detector in tests

## 9. Phase 7 — Async and structured concurrency

### Scope

- `async fn`/`await`
- task runtime
- scope/spawn/join
- cancellation
- deadline
- bounded channel
- `Send`/`Share`/`SuspendSafe`
- supervisor minimum
- deterministic test scheduler
- generic actor stdlib minimum（専用 syntax なし）

### Acceptance

- child cannot escape
- scope joins/cancels
- fail-fast/collect-all
- panic propagation
- deadline inheritance
- cancellation cleanup
- guard/resource borrow across await rejected
- non-Send capture rejected
- bounded channel backpressure
- deterministic schedule replay
- bounded exploration finds seeded race/ordering bug
- runtime trace task tree

### Production spike

HTTP-like fake service で:

- concurrent request
- timeout
- cancellation
- graceful shutdown
- resource cleanup
- trace

を end-to-end 実証。

## 10. Phase 8 — Package, hermetic build, artifacts

### Scope

- declarative `Mendrel.pkg`
- local/path/registry dependency model
- lockfile/content digest
- sandbox generator
- target/profile
- MRA artifact
- SBOM/provenance skeleton
- reproducible build
- edition metadata

### Acceptance

- direct dependency only
- undeclared file/env/network access rejected
- same input same artifact
- source path/timestamp nondeterminism test
- generator sandbox no authority
- lockfile tamper detection
- artifact inspect/verify
- SPDX-compatible SBOM
- provenance statement
- unsafe/capability/API fingerprint
- publish dry-run

registry service 自体は local package semantics が安定してから。

## 11. Phase 9 — LSP and MAP

### Scope

- `mendreld`
- LSP core
- MAP workspace/symbol/context/check
- typed hole
- change plan/preview/commit
- stale revision rejection
- impacted test
- API/effect/schema/unsafe query
- agent audit

### Acceptance

- editor completion/definition/references/rename
- typed hole expected type/effect/candidates
- context bundle budget/selection reason
- multi-file signature change preview
- stale commit rejected
- normal text diff returned
- affected callers/tests
- protocol schema compatibility
- repository prompt injection origin tagging
- sandbox grant enforcement
- daemon/batch semantic agreement

## 12. Phase 10 — Wire, DB, production adapters

### Scope

- wire record/enum/schema IR
- explicit ID/reserve/unknown preservation
- Protobuf adapter
- WIT adapter/component backend
- JSON adapter
- API/effect/schema SemVer gate
- typed config/secret
- OpenTelemetry-compatible signals
- checked SQL snapshot
- migration checker
- service lifecycle/deployment check

### Acceptance

- schema compatibility corpus
- unknown round-trip
- ID reuse publish rejection
- WIT component interop fixture
- config provenance/redaction
- Secret log compile rejection
- SQL parameter/result check
- expand/backfill/contract migration report
- artifact vs deployment capability diff
- graceful service drain
- trace/metric/log correlation

## 13. Phase 11 — Production hardening

### Scope

- concurrent generational collector
- GC telemetry/tuning profiles
- PGO/LTO
- code size/shared generics
- crash bundles
- fuzz scale-up
- supply-chain signatures/transparency
- independent reproducibility
- performance regression infrastructure
- compatibility policy stabilization

### Acceptance

- service trial under load
- latency/throughput/memory data
- p50/p95/p99 GC pause
- cancellation/shutdown chaos
- runtime failure injection
- compiler incremental large-repo benchmark
- MendrelBench against comparison languages
- external security review
- conformance suite version 1
- language spec 1.0 candidate

## 14. Phase 12 — Adoption, not self-hosting by default

候補:

- C/WIT/Protobuf/OpenAPI interop
- migration tooling
- package registry governance
- IDE distribution
- training corpus/docs
- production case study
- language server remote index

self-hosting は、次を満たした場合だけ検討。

- compiler/runtime ABI stable
- bootstrap reproducible
- diagnostic quality regressionを防げる
- Rust implementationとの differential route が残る
- self-host が user value を上げる

「言語として一人前に見える」ためだけには行わない。

## 15. Cross-phase quality gates

全 phase で維持:

- no compiler panic on invalid user input
- deterministic diagnostic order
- stable source span
- formatter idempotence for supported syntax
- JSON schema validation
- no hidden ambient build input
- no silent unsupported syntax
- source/semantic mapping
- generated asset check
- license/security scan
- benchmark baseline
- documented public API change

## 16. Issue template

```markdown
## Goal

## Relevant specification

## User-visible behavior

## Thin vertical slice

## Out of scope

## Invariants

## Diagnostics

## Tests
- unit
- golden
- property/fuzz
- conformance
- differential/runtime

## Public/API/effect/schema/unsafe impact

## Verification commands

## Risks and rollback
```

## 17. Pull request size discipline

PR は line 数で機械的に切るより意味責務で切る。ただし以下は警告信号。

- unrelated crates touched
- grammar/type/runtime/tooling を fixture なしに同時変更
- public API が多数増える
- error strings だけで contract
- test が snapshot 一本
- phase 外の placeholder
- unused abstraction
- speculative extension point
- generated/handwritten duplication
- 「後で verifier を入れる」前提で unsafe state を通す

大きくなる場合、spec/test scaffold、frontend、semantic rule、backend、tooling を順に分ける。

## 18. Definition of done for 1.0

1.0 は次の総合条件を満たす。

- normative syntax/type/effect/resource/task semantics
- reference compiler + runtime
- LLVM native + Wasm component target
- stable diagnostic/MAP protocol v1
- package/build/artifact/reproducibility
- API/effect/schema compatibility
- production observability/config/secret/service lifecycle
- conformance suite
- security threat model review
- representative production service trial
- MendrelBench public methodology
- migration/edition policy
- no unresolved soundness hole
- known performance limitations quantified
