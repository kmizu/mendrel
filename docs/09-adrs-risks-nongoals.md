# 09. Architecture decisions, risks, rejected alternatives, and non-goals

## 1. ADR summary

### ADR-001 — Text source is canonical

**Decision:** UTF-8 `.mnd` text is the source of truth. Lossless CST/HIR/semantic graph are derived per workspace revision.

**Why:** Git diff、comment、editor、shell、long-term readability を守る。agent は MAP semantic operation を使える。

**Rejected:** persistent AST/graph source、hidden sidecar node IDs as identity。

**Consequence:** semantic edit は canonical text patch と conflict handling を必要とする。

### ADR-002 — Managed memory for ordinary values

**Decision:** ordinary values use precise tracing GC.

**Why:** service/domain code から pervasive lifetime proof を外し、変更局所性を上げる。

**Rejected:** universal ownership/borrow checking、manual memory default。

**Consequence:** hard real-time 非目標、GC telemetry/backend engineering が必要。

### ADR-003 — Affine resources, no user finalizers

**Decision:** deterministic external resources are affine `resource`; lexical `use` guarantees cleanup. User finalizer/resurrection is forbidden.

**Why:** cleanup を source/task control flow へ戻す。

**Consequence:** resource verifier と cancellation/panic cleanup path が compiler/runtime の中核になる。

### ADR-004 — Named capability/effect rows

**Decision:** external authority appears in `uses { label: Capability }`.

**Why:** dependency、security、testing、impact、deployment grant を同じ情報から得る。

**Rejected:** ambient globals、service locator、type-only implicit context、string effect labels only。

**Consequence:** signature verbosity と row growth を lint/tool で管理。

### ADR-005 — No general effect handlers in v1

**Decision:** effect rows yes; resumable general handlers no.

**Why:** continuation/resource/cancellation/observability/codegen complexity と非局所 behavior を避ける。

**Consequence:** retry/logging/cache/test fake は capability wrapper で表現。

### ADR-006 — Structured concurrency only by default

**Decision:** tasks are lexical children. Detached service requires `Supervisor`.

**Why:** lifecycle、failure、cancellation、observability の局所性。

**Rejected:** global executor、fire-and-forget、actor-only core。

### ADR-007 — Explicit public signatures, local inference

**Decision:** declaration boundary signatures are explicit; body/local expressions infer.

**Why:** API stability、incremental compilation、agent context、review。

**Rejected:** whole-module/public type inference、annotation everywhere。

### ADR-008 — Nominal public types, limited structural internals

**Decision:** public/domain/wire types nominal. Anonymous structural record local/private only.

**Why:** accidental compatibility と rename/change blast radius を減らす。

### ADR-009 — One recoverable error model

**Decision:** `Result<T,E>` only. Panic is invariant failure.

**Rejected:** exceptions + Result dual model、unchecked exception。

### ADR-010 — No arbitrary macros or build scripts

**Decision:** fixed derive/schema forms + sandboxed declared Wasm generators.

**Why:** parsing、name resolution、security、reproducibility、agent understanding。

### ADR-011 — Canonical formatter with no configuration

**Decision:** one stable format.

**Why:** equivalent text forms、diff noise、prompt variance、style debate を減らす。

### ADR-012 — Stable component/wire boundary, not native ABI

**Decision:** WIT/Wasm Component Model and explicit wire schema for stable interop.

**Why:** native layout/ABI freezes optimizer/runtime and is language-specific.

### ADR-013 — Rust reference compiler, LLVM native backend

**Decision:** bootstrap compiler/runtime in Rust; LLVM first native backend; Wasm component second.

**Why:** implementation safety、mature codegen/debug/sanitizer ecosystem。

**Rejected:** hand-written machine code as primary、early self-hosting、JVM-only semantics。

### ADR-014 — Stable machine diagnostics

**Decision:** diagnostic schema/code is a versioned product API.

**Why:** agent/IDE/CI repair loop depends on structured facts, not prose scraping.

### ADR-015 — LSP plus MAP

**Decision:** LSP for editors; MAP for snapshot-aware semantic transactions/context/build impact.

**Why:** repository agent task requires more than document editing.

### ADR-016 — Hermetic/reproducible build as default contract

**Decision:** no undeclared host input; content-addressed artifacts; SBOM/provenance.

**Why:** production trust、debugging、cache、supply chain。

### ADR-017 — Wire identity explicit

**Decision:** wire fields/cases use stable explicit IDs and reservation.

**Why:** source order/name cannot safely be protocol identity.

### ADR-018 — Debug/release semantics equal

**Decision:** optimization profile may change performance only, except explicitly named diagnostic-only constructs such as `debug_assert`.

**Why:** production-only bugs and LLM/test false confidence reduction.

## 2. Risk register

| Risk | Probability | Impact | Detection | Mitigation |
|---|---:|---:|---|---|
| Capability rows become giant context | high | high | effect surface metrics、lint、review | exact labels、small domain ports、`AppContext` deny、surface diff |
| Signature verbosity reduces adoption | high | medium | user studies、diff data | local inference、signature materializer、formatter |
| GC tail latency | medium | high | p99 pause/latency telemetry | concurrent generational collector、latency profile、allocation report、hard-RT non-goal |
| Resource model becomes borrow checker by stealth | medium | high | corpus annotation burden | lexical borrow only、no user lifetime syntax、owned transfer、feature admission test |
| Generality lost without macros | medium | medium | repeated boilerplate corpus | built-in derive/schema、Wasm generator、only evidence-based additions |
| Generator ecosystem recreates unsafe build scripts | medium | high | capability/provenance audit | sandbox、declared input/output、no network default、pinned digest |
| MAP semantic IDs used as permanent identity | high | medium | client misuse tests | revision-required schema、stale rejection、explicit public/wire IDs only |
| Context bundle omits crucial code | medium | high | benchmark recall、agent failure taxonomy | selection reason、continuation、manual expand、full graph fallback |
| Learned ranking becomes opaque source of truth | medium | medium | deterministic comparison | deterministic core facts、model rank optional/versioned |
| Diagnostic schema freezes bad design | medium | medium | consumer feedback | semantic major/minor、extensible optional fields、stable code not stable prose |
| Effect inference complexity | medium | high | compile/query performance | row constraints limited、no handlers、explicit public rows、deterministic solver |
| Trait system ambiguity | medium | high | corpus/coherence tests | orphan/coherence、no overlap/specialization、explicit import |
| No stable native ABI hurts plugins | high | medium | ecosystem requests | WIT/component、C FFI adapters、same-version native profile only |
| Wasm component performance/feature gaps | medium | medium | interop benchmarks | native backend primary、component boundary only where useful |
| Reproducible build cost | medium | medium | CI duration/cache | content cache、selective double-build、release gate |
| Supply-chain metadata overwhelms users | medium | medium | workflow metrics | one CLI、artifact defaults、policy profiles |
| Wire/domain duplication feels verbose | high | medium | application corpus | derives/adapters、explicit boundary retained for safety |
| Checked overflow performance | low-medium | medium | benchmark/optimizer report | proof-based elimination、explicit wrapping/saturating |
| Unicode identifier security false positives | medium | low-medium | lint feedback | edition-pinned policy、clear diagnostic、security profile |
| Contract runtime cost | medium | medium | profiler | constrained newtypes、proof-based elimination only with artifact |
| Optional verifier creates two classes of code | medium | medium | package policy audit | runtime checks remain default、proof profile metadata |
| Actor and task abstractions overlap | medium | medium | API confusion study | actor is stdlib-only in v1; task is the sole core concurrency syntax |
| Implementation scope too large | high | high | phase slippage、empty subsystem | thin vertical slices、non-goals、reuse LLVM/WIT/OTel/Proto |
| New language lacks ecosystem/training data | high | high | adoption/benchmark | strong interop、docs/examples、agent protocol、honest comparison |
| “LLM language” branding ages poorly | medium | medium | user perception | frame as maintainability language; LLM claims benchmarked |
| Compiler/runtime bugs undermine safety | high early | high | fuzz/differential/conformance | Rust implementation、MIR verifier、sanitizers、external review |
| Observer telemetry becomes hidden effect | medium | high | failure/behavior tests | closed observer semantics、bounded/non-failing、explain tool |
| Secret zeroization impossible under moving GC | high | security-specific | threat modeling | do not promise; affine pinned `SecretBuffer` for strict need |
| API/effect SemVer classification false positive/negative | medium | high | compatibility corpus | separate classes、manual-review category、no overconfident auto-publish |
| No feature flags causes package fragmentation | medium | medium | ecosystem graph | named target/package/cap impl; revisit with evidence |
| Compile-time SQL schema drift | medium | high | migration CI | pinned snapshots、schema-range artifact metadata |
| Deterministic scheduler misses concurrency bugs | high | medium | comparison with stress/production | bound disclosure、random+systematic、chaos/runtime telemetry |
| Canonical formatter harms carefully formatted DSL/data | medium | low | user reports | no arbitrary embedded DSL; raw string content untouched |
| Public explicit effect addition causes major bumps often | high | medium | ecosystem version data | design small capabilities、internal wrappers、authority changes genuinely visible |

## 3. Rejected alternatives in detail

### 3.1 Universal ownership and lifetime annotations

魅力:

- memory safety without GC
- deterministic memory
- zero-cost abstraction
- alias control
- resource safety

棄却理由は性能ではなく主戦場との fit。ordinary service change が data representation、borrow path、async boundary、signature lifetime へ波及しやすい。LLM は compiler feedback で直せても、人間 review の意味負担が残る。

採用部分:

- affine resource
- move
- lexical borrow
- `Send`/`Share`
- unsafe isolation
- explicit pinning
- ownership transfer for mutable graph

### 3.2 Dynamic/gradual typing

魅力:

- fast prototyping
- JSON/dynamic ecosystem
- LLM familiarity
- REPL

棄却:

- runtime-only missing symbol/type
- API compatibility weaker
- dynamic behavior surface
- agent hallucinationが遅く発見
- production schema/security policy が外付け

採用部分:

- `Dynamic` boundary
- evaluator/REPL eventual
- good error recovery
- easy scripting target may be separate profile/language, not core semantics

### 3.3 Full algebraic effects

魅力:

- orthogonal composition
- mock/handler
- async/state/exception unification
- research elegance

棄却:

- continuation semantics
- resource/cancellation interaction
- stack/trace complexity
- nonlocal handler scope
- backend/runtime burden

effect row だけを採用し、capability value で practical handling。

### 3.4 Actor-only concurrency

魅力:

- isolation
- supervision
- no shared memory
- distributed story

棄却:

- request-local fork/join が message ceremony
- type/error/cancellation flow が indirect
- location transparency の罠
- mutable state がない pure pipeline まで actor 化

actor は専用 core syntax を持たない stdlib/supervision profile。

### 3.5 Persistent semantic graph as source

魅力:

- stable node identity
- structural merge
- exact refactor
- no syntax error

棄却:

- human/Git/editor ecosystem
- comment/format nuance
- storage migration
- compiler/tool lock-in
- diff/review portability

MAP で利点の大半を得る。

### 3.6 Full proof language

魅力:

- strongest correctness
- executable specification
- contract proof

棄却:

- proof cost
- solver/proof maintenance
- ordinary production team adoption
- agent context explosion
- library/FFI trust boundary

pure contract/newtype + optional verifier に限定。

### 3.7 JVM as primary target

魅力:

- GC/JIT/ecosystem
- service deployment
- tooling

棄却ではなく deferred backend。初期は LLVM native/Wasm により runtime semantics と component boundary を自前で明確化する。JVM backend は GC/async/resource/FFI mapping の feasibility を後で評価。

### 3.8 Custom machine-code backend

魅力:

- control
- research value
- small dependency

棄却:

- debug info
- optimizers
- platform ABI
- sanitizer
- linker/object format
- architecture support
- maintenance burden

reference backend は既存成熟基盤を使う。

## 4. Feature admission test

新機能は RFC で以下へ答える。どれか重大項目が満たせなければ不採用。

1. **Locality:** 意味を理解するためにどこまで読む必要があるか。
2. **Visibility:** public signature/manifest/artifact に重要な効果が現れるか。
3. **Canonicality:** 同じ意味の表現を不必要に増やさないか。
4. **Determinism:** parse、type、build、test、protocol の再現性を壊さないか。
5. **Explainability:** stable structured diagnostic/MAP で説明できるか。
6. **Composability:** type/effect/resource/task/contract と一貫するか。
7. **Compatibility:** API/effect/wire change を分類できるか。
8. **Safety:** safe code no-UB/data-race-free を保つか。
9. **Production:** failure、timeout、cancel、observability、deployment を定義できるか。
10. **Implementation:** compiler/runtime/tooling 全体の cost と test plan があるか。
11. **Agent value:** raw text/prompt engineering では得にくい改善か。
12. **Human value:** reviewer/operator の負担も下げるか。
13. **Evidence:**実 corpus の複数 use case があるか。
14. **Removal:**追加しない代替、stdlib/generator/tool で済む可能性を検討したか。
15. **Migration:** edition/auto-fix/compatibility path があるか。

## 5. Language non-goals v1

- hard real-time
- bare-metal/極小 embedded
- OS kernel/device driver
- GPU kernel language
- C++ source compatibility
- stable native ABI
- arbitrary compile-time metaprogramming
- general effect handler
- full dependent type
- proof assistant
- dynamic monkey patch/reflection/eval
- class inheritance
- overload resolution through conversion
- custom operator
- whitespace syntax
- configurable formatter
- implicit global dependency
- detached task
- user finalizer
- transitive import
- feature-flag matrix
- hot code reload/state migration
- distributed location transparency
- self-hosting as milestone
- one language for every domain

## 6. Tooling non-goals v1

- autonomous requirement/product decision
- automatic merge/publish/deploy
- unrestricted shell through MAP
- cloud secret access for code repair
- learned model as semantic oracle
- permanent semantic node identity
- proprietary IDE requirement
- replacing Git
- replacing human review
- optimizing benchmark score through hidden hints

## 7. Deferred but bounded decisions

「未決定」を無限に残さず、decision trigger を置く。

### Production collector algorithm

MVP collector 完成後、service benchmark で pause/throughput/heap を測り、concurrent generational design を ADR で選ぶ。source semantics は既に fixed。

### Panic unwind vs abort profile

resource cleanup と supervisor report の要件を満たす default unwind strategy を実装・測定。abort-only target は explicit profile/target で、同じ recoverable result semantics を保つ。

### Async state-machine ABI

LLVM/Wasm prototype で layout/cancellation/debug info を比較し、runtime ABI freeze gate で決定。

### Contract proof backend

normal runtime contract が安定後、Dafny/SMT/abstract interpretation 風 backend を比較。normal compile は依存しない。

### JVM backend

Wasm/native 1.0 後、service ecosystem value と semantics mapping を評価。source feature を JVM の制約へ先回りして曲げない。

### General effect handler

production corpus で named capability wrapper の明確な限界が反復観測された場合のみ edition RFC。

## 8. Kill criteria

プロジェクトの前提を見直す条件。

- MendrelBench で MAP/explicit semantics が既存言語＋同等 tooling に継続的に優位を示さない
- public signature/effect verbosity が review/defect を改善せず adoption cost だけ増やす
- GC と affine resource の混合が理解/実装上、全面 ownership より悪い
- compatibility classifier の誤判定が運用上許容不能
- compiler/toolchain の scope が core team で保守不能
- interop が弱く production trial へ到達できない
- agent protocol が LSP/standard tools の小拡張で十分と判明

kill は全廃だけでない。MAP standalone、compiler tooling、effect/API diff、wire/toolchain だけを既存言語へ移植する選択も含む。

## 9. 最終的な守り

Mendrel の最大の敵は、機能不足ではなく「良い機能を足し続けた結果、理解するための世界が大きくなること」や。

したがって default response は追加ではなく、

- library で済まないか
- capability interface で済まないか
- generator で済まないか
- MAP/tooling で済まないか
- 既存構文の canonical use で済まないか
- そもそも主戦場外ではないか

を先に問う。
