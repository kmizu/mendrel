# 12. Primary references and design lineage

This design deliberately combines established ideas rather than claiming each mechanism as novel. The important contribution is the particular production-oriented composition around maintenance locality and agent interfaces.

The list below records what Mendrel borrows and what it intentionally does not inherit.

## 1. Effects and capabilities

### Koka: Programming with Row-Polymorphic Effect Types

- Daan Leijen, Microsoft Research
- https://www.microsoft.com/en-us/research/publication/koka-programming-with-row-polymorphic-effect-types/

**Borrowed**

- effects visible in function types,
- Hindley–Milner-style inference combined with effects,
- row polymorphism as a practical representation of effect sets,
- semantic significance of an absent effect.

**Not copied directly**

- Koka syntax,
- the complete Koka effect system,
- duplicate labels in Mendrel source rows,
- general effect handlers as a v1 user feature.

### Effect Handlers, Evidently

- Daan Leijen et al., Microsoft Research
- https://www.microsoft.com/en-us/research/publication/effect-handlers-evidently/

**Used as caution and evidence**

Effect handlers are powerful enough that linearity, scoped resumptions, resources, and runtime representation become central. Mendrel adopts explicit effect rows but defers resumable general handlers until production evidence justifies the semantic and implementation cost.

### Object-capability security

- Agoric documentation: https://docs.agoric.com/
- ERTP overview: https://docs.agoric.com/guides/ertp/

**Borrowed**

- authority by possession of an unforgeable reference,
- explicit delegation,
- least authority,
- separation of authority from ambient names.

Mendrel’s named `uses` rows add static role labels, effect tracing, deployment-grant comparison, and API compatibility classification.

## 2. Ownership, reference capabilities, and memory

### The Rust Programming Language and Rust Reference

- https://doc.rust-lang.org/book/
- https://doc.rust-lang.org/reference/

**Borrowed**

- safe/unsafe boundary,
- move semantics for affine resources,
- ownership as a way to represent non-copyable state,
- explicit `Result`/`Option`,
- tooling culture around diagnostics, formatter, package metadata, and sanitizers.

**Not copied**

- universal borrow/lifetime obligations for ordinary managed values,
- Rust syntax and trait system in full,
- release overflow differences,
- macro system,
- Cargo feature unification.

Mendrel’s conclusion is that ownership should be concentrated where lifetime is semantically important rather than applied to every heap value.

### Pony reference capabilities

- Pony Tutorial — Reference Capabilities
- https://tutorial.ponylang.io/reference-capabilities/reference-capabilities.html

**Borrowed**

- static reasoning about aliasing and safe transfer across concurrent actors/tasks,
- the idea that share/send properties derive from reference/data capabilities.

**Adapted**

Mendrel exposes a smaller `Share`/`Send`/`Owned<T>` model and keeps ordinary mutable objects task-local. It does not require Pony’s full reference-capability vocabulary in ordinary domain code.

### Memory Management Toolkit status

- https://www.mmtk.io/status

MMTk is valuable as an experimental/future collector backend. Its own status page states that it is under active development and not yet production-ready, so Mendrel v1 must not make production readiness depend on it.

## 3. Structured concurrency

### OpenJDK JEP 505 — Structured Concurrency (Fifth Preview)

- https://openjdk.org/jeps/505

**Borrowed**

- related tasks treated as a unit,
- lexical task lifetime,
- failure/cancellation propagation,
- improved observability through task hierarchy,
- join policies.

**Strengthened in Mendrel**

- task handle scope encoded in the type checker,
- no ordinary detached task API,
- deadline propagation,
- resource cleanup integrated with cancellation,
- deterministic schedule replay,
- `Send`/`Share` capture checking.

The precise JDK API is not copied; the design principle is.

### Erlang/OTP supervision principles

- https://www.erlang.org/doc/system/design_principles.html

**Borrowed**

- explicit supervision trees for long-lived services,
- restart and escalation policy separated from ordinary request logic,
- observable parent/child operational structure.

**Adapted**

Mendrel does not make every computation an actor/process. Supervision is used for service lifecycle and long-lived state, while request-local parallelism remains lexical structured concurrency.

## 4. Contracts and verification

### Dafny Reference Manual

- https://dafny.org/dafny/DafnyRef/DafnyRef

### SPARK User’s Guide — Subprogram Contracts

- https://docs.adacore.com/spark2014-docs/html/ug/en/source/how_to_write_subprogram_contracts.html

**Borrowed**

- preconditions, postconditions, invariants,
- explicit specification at function boundaries,
- the value of executable/verified contracts.

**Adapted**

Mendrel’s normal build uses a deliberately bounded pure contract subset and runtime checks. Solver-backed verification is an optional profile with proof provenance; ordinary compilation does not depend on solver heuristics.

## 5. Typed holes and compiler feedback

### GHC User’s Guide — Typed Holes

- https://downloads.haskell.org/ghc/latest/docs/users_guide/exts/typed_holes.html

**Borrowed**

- incomplete expression with an expected type,
- reporting of in-scope candidates and relevant constraints.

**Extended**

Mendrel holes report expected capability/effect rows, resource/task legality, and machine-applicable semantic edits. Release builds reject holes.

### rustc JSON output

- https://doc.rust-lang.org/rustc/json.html

**Borrowed**

- one structured diagnostic message per JSON line,
- stable codes,
- spans,
- child/context information,
- suggestion applicability,
- forwards-compatible consumers.

**Extended**

Mendrel adds a cause DAG, typed expected/actual facts, effect/resource/task paths, workspace revision, and transaction-safe fixes.

### Language Server Protocol

- https://microsoft.github.io/language-server-protocol/

**Borrowed**

- editor-independent language intelligence over JSON-RPC,
- completion, navigation, references, rename, diagnostics.

**Extended**

MAP is separate because repository-scale agents need snapshot transactions, context budgeting, semantic impact, artifact operations, and audit.

### SARIF 2.1.0

- OASIS Standard
- https://docs.oasis-open.org/sarif/sarif/v2.1.0/sarif-v2.1.0.html

Mendrel emits SARIF for interoperability with static-analysis pipelines while retaining a richer native diagnostic schema.

## 6. LLM/software-engineering research

### SWE-bench: Can Language Models Resolve Real-World GitHub Issues?

- Carlos E. Jimenez et al.
- https://arxiv.org/abs/2310.06770

**Design implication**

Real repository issues require coordination across functions/files, execution feedback, and long context. MendrelBench therefore evaluates repository maintenance rather than isolated function generation.

### SWE-agent: Agent-Computer Interfaces Enable Automated Software Engineering

- John Yang et al.
- https://arxiv.org/abs/2405.15793

**Design implication**

The agent-computer interface materially affects software-engineering performance. Mendrel treats MAP and diagnostic schemas as first-class parts of the language product, not editor polish added after the compiler.

### Iterative Refinement of Project-Level Code Context for Precise Code Generation with Compiler Feedback（CoCoGen）

- Zhangqian Bi et al.
- https://arxiv.org/abs/2403.16792

**Design implication**

Compiler/static-analysis feedback can identify project-context mismatch and guide iterative repair. Mendrel makes the feedback structured, versioned, and semantic rather than prose-only.

### AST-T5: Structure-Aware Pretraining for Code Generation and Understanding

- Linyuan Gong, Mostafa Elhoushi, Alvin Cheung
- https://arxiv.org/abs/2401.03003

**Design implication**

Code structure matters for context segmentation and code tasks. Mendrel keeps text as source but exposes lossless CST/HIR-aware context bundles and declaration-boundary chunking.

### GRAMMAR-LLM: Grammar-Constrained Natural Language Generation

- Gabriele Tuccio et al., Findings of ACL 2025
- https://aclanthology.org/2025.findings-acl.177/

**Design implication**

Formal grammar constraints can make generated structured output syntactically reliable without making the model itself the recognizer. Mendrel therefore exposes deterministic parser expectations and fragment formatting through MAP. Grammar-constrained decoding is an optional host technique, not part of language semantics.

### RepoHyper: Search-Expand-Refine on Semantic Graphs for Repository-Level Code Completion

- Huy N. Phan et al.
- https://arxiv.org/abs/2403.06095

**Design implication**

Repository context is better selected from semantic relations than from lexical similarity alone. Mendrel’s context bundles use compiler-derived symbol/type/effect/resource/test graphs, while learned ranking remains optional and versioned.

These papers motivate design hypotheses; they do not prove that Mendrel will outperform mature languages. MendrelBench is required to test that claim.

## 7. Wire compatibility

### Protocol Buffers best practices

- https://protobuf.dev/best-practices/dos-donts/

### Protocol Buffers editions language guide

- https://protobuf.dev/programming-guides/editions/

**Borrowed**

- stable numeric field identifiers,
- never reusing deleted field numbers/names,
- reserving removed fields,
- compatibility-aware schema evolution,
- explicit language editions.

**Extended**

Mendrel makes wire declarations part of compiler API/schema diff, preserves unknown data, separates domain and wire types, and includes schema fingerprints in release artifacts.

## 8. Component interfaces

### WebAssembly Component Model — WIT Reference

- https://component-model.bytecodealliance.org/design/wit.html
- upstream specification: https://github.com/WebAssembly/component-model

**Borrowed**

- explicit interfaces and worlds,
- imports/exports as component contracts,
- records/variants/options/results/resources,
- language-neutral boundaries,
- canonical ABI direction.

**Use in Mendrel**

WIT/component boundaries are preferred for stable plugins and sandboxed generators. Mendrel does not promise stable native object layout or ABI across compiler versions.

## 9. Hermetic and reproducible builds

### Bazel hermeticity documentation

- https://bazel.build/basics/hermeticity

### Reproducible Builds definition

- https://reproducible-builds.org/docs/definition/

**Borrowed**

- declared inputs,
- isolation from host state,
- same source/build environment/instructions producing identical artifacts,
- cacheability and verification value.

Mendrel makes these default properties of the official build tool rather than an optional external convention.

## 10. Supply-chain metadata

### SLSA v1.2 provenance

- https://slsa.dev/spec/v1.2/provenance

### SPDX specification

- https://spdx.github.io/spdx-spec/

**Borrowed**

- provenance statements tying artifacts to build process and inputs,
- machine-readable SBOM and licensing/security metadata.

Mendrel Release Artifacts package provenance, SBOM, API/effect/schema/unsafe fingerprints, and reproducibility evidence together.

## 11. Observability

### OpenTelemetry signals

- https://opentelemetry.io/docs/concepts/signals/

**Borrowed**

- traces, metrics, logs, and related telemetry signals,
- context propagation and vendor-neutral semantic conventions.

**Adapted**

Mendrel provides a typed capability/runtime observer layer with redaction, cardinality, volume, and non-interference constraints. It does not expose a general hidden-effect hook.

## 12. Unicode security

### Unicode Technical Standard #39 — Unicode Security Mechanisms

- https://www.unicode.org/reports/tr39/

**Borrowed**

- confusable detection,
- mixed-script and identifier security profiles,
- versioned Unicode security data.

Mendrel pins Unicode behavior by edition and uses stricter ASCII rules for package/module/wire identities.

## 13. Enforced semantic versioning

### Elm

- https://elm-lang.org/

Elm demonstrates compiler-detected public API changes and enforced semantic-version classification.

**Extended**

Mendrel classifies source API, capability/effect surface, wire schema, unsafe surface, and security-sensitive authority changes separately before computing publish policy.

## 14. Content identity and durable execution

### Unison — the big idea

- https://www.unison-lang.org/docs/the-big-idea/

Unison demonstrates the power of content-addressed definitions and identity independent of human-chosen names. Mendrel borrows content digests for packages, artifacts, generated inputs, and normalized IR caches, but deliberately keeps canonical text/Git as source of truth. A normalized IR digest is not presented as a proof of semantic equivalence.

### Dhall

- https://dhall-lang.org/

Dhall demonstrates a total, strongly normalized configuration language and integrity checks over meaning rather than raw presentation. Mendrel borrows the spirit of typed configuration, hermetic evaluation, and content integrity, while keeping configuration as a bounded production interface rather than embedding Dhall’s language.

### Temporal durable execution

- https://docs.temporal.io/

Temporal demonstrates replay-based crash recovery and durable workflow state. Mendrel Agent’s deferred `DurableFlow` layer borrows explicit effect suspension, recorded outcomes, versioned state, and replay divergence checks. Ordinary Mendrel functions are not silently made durable.

## 15. Deliberate source hierarchy

The sources above have different roles.

- Language/type papers inform the semantic design.
- Official language/runtime documentation informs practical constraints.
- Standards define interoperable artifact/protocol formats.
- LLM papers motivate measurable interface hypotheses.
- None is treated as authority that overrides Mendrel’s production goals.

When an external mechanism conflicts with semantic locality, deterministic operation, or a small legal state space, Mendrel narrows or rejects it rather than accumulating features.
