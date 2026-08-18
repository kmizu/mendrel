# 08. Conformance, testing, and MendrelBench

## 1. なぜ benchmark を仕様の一部にするか

「LLM に理想的」「保守しやすい」は、設計者の美学だけでは判定できない。Mendrel は次の仮説を持つ。

> semantic locality、explicit effects、canonical source、structured diagnostics、semantic edit protocol は、repository maintenance に必要な context、修正回数、回帰率、review effort を下げる。

この仮説は、既存言語との比較と longitudinal production data で反証可能にする。

同時に、language implementation 自体の soundness、determinism、performance も conformance suite で固定する。

## 2. Test layers

### 2.1 Spec conformance

normative rule ごとに:

- rule ID
- positive program
- negative program
- expected diagnostic code
- evaluator result
- native result
- Wasm result where supported
- edition applicability

を持つ。

example:

```yaml
rule: EFFECT-NAMED-ROW-EXACT-LABEL
positive: effects/exact_label_ok.mnd
negative: effects/label_mismatch.mnd
diagnostic: E-EFFECT-0012
```

rule inventory が docs/formal kernel と機械照合される。

### 2.2 Parser/formatter

- lexer coverage
- valid corpus parse
- malformed corpus recovery
- CST round trip
- formatter idempotence
- formatter parse preservation
- comment attachment
- incremental/full equivalence
- arbitrary UTF-8 fuzz
- deeply nested/adversarial input
- complexity/stack safety

### 2.3 Semantic

- name resolution
- type inference
- trait coherence
- effect row
- error conversion
- exhaustive match
- contract well-formedness
- resource flow
- task scope
- Send/Share/SuspendSafe
- wire compatibility
- unsafe boundary

### 2.4 Differential

同じ program/input を:

- reference evaluator
- LLVM O0
- LLVM optimized
- Wasm component
- optional alternate GC backend

で実行し、result/error/panic/observable event を比較。

nondeterministic source は fake clock/random/scheduler で control する。

### 2.5 Runtime stress

- GC root/moving stress
- allocation failure
- low memory
- resource acquire/cleanup
- panic during cleanup
- cancellation at every suspension point
- deadline races
- channel saturation
- scheduler interleavings
- actor mailbox overflow
- supervisor restart storm
- signal/drain/shutdown
- FFI sanitizer
- stack overflow/deep recursion
- crash bundle integrity

### 2.6 Production artifact

- hermeticity
- undeclared input
- reproducibility
- SBOM/provenance
- signature
- dependency tamper
- generator sandbox
- API/effect/schema diff
- deployment capability mismatch
- rollback schema range
- secret redaction

## 3. Property testing

stdlib に built-in property test framework を置く。

```mendrel
property test encode_decode_round_trip(order: Order) {
    let bytes = OrderWire.encode(order);
    OrderWire.decode(bytes) == Ok(order)
}
```

generator/shrinker は type-derived 可能だが、constraint/newtype/secret/resource に適切な policy を要求する。

失敗:

- seed
- minimal counterexample
- shrink trace
- compiler/runtime/artifact fingerprint
- scheduler trace if async

を保存。

## 4. Fuzzing

### 4.1 Compiler fuzz

- lexer
- parser
- formatter
- AST lowering
- type/effect checker
- MIR verifier
- optimizer
- API/schema diff
- diagnostic renderer
- MAP decoder/transaction

### 4.2 Runtime/library fuzz

- text/Unicode
- wire/JSON/Protobuf decode
- SQL parser
- URL/HTTP
- archive/package
- FFI safe wrapper
- cancellation/cleanup sequence
- actor/channel protocol

### 4.3 Differential fuzz

random well-typed program generator を段階的に作る。

- pure expression
- ADT/match
- generic/trait
- Result/effect fake
- resource state
- task/scheduler

evaluator と backend の差を探索。ill-typed generator も diagnostic determinism/no panic を検査。

## 5. Mutation testing

test suite の弱さを測るため、compiler/runtime/application fixture に mutation testing。

mutation:

- branch inversion
- error mapping removal
- capability label swap
- deadline removal
- retry count change
- resource cleanup omission
- enum arm replacement
- wire ID change
- secret redaction removal
- task join removal
- bounds/overflow check removal in compiler test build

mutation survivor を coverage より重い signal とする。ただし compiler implementation の低レベル mutation は cost が高いため periodic profile。

## 6. Impacted test selection

compiler graph から candidate test を選ぶ。

signal:

- symbol reference
- instantiated generic
- trait impl resolution
- effect/capability call
- wire schema consumer
- contract
- generated source
- runtime feature
- historical coverage map
- package dependency

report:

- selected tests and reasons
- omitted tests and confidence
- graph edge
- full-suite policy trigger

impact selection は release gate の full conformance を完全に置き換えない。local/PR feedback を速め、risk threshold で full suite を実行。

precision/recall を seeded change corpus で測る。

## 7. Deterministic concurrency suite

### 7.1 Schedule representation

scheduler decision:

```text
step 1: run task root/checkout/payment
step 2: complete fake gateway attempt 1
step 3: cancel sibling inventory
...
```

replay token は:

- test ID
- source/artifact revision
- seed
- virtual clock events
- capability fake events
- scheduler decisions
- injection points

を含む。

### 7.2 Exploration

bounded DPOR または同等の partial-order reduction を将来評価。v1 minimal は seeded randomized schedule + systematic bounded branch。

探索の completeness を過大主張しない。report に bound と unexplored frontier を出す。

### 7.3 Invariants

- task leak zero
- resource leak zero
- no blocked task at scope exit
- channel capacity respected
- cancellation acknowledged
- deadline monotonic
- actor serial state access
- panic/cause tree deterministic for same schedule

## 8. API/effect/schema corpus

compatibility rule ごとに before/after pair。

### API

- add/remove function
- rename
- parameter change
- return/error change
- enum variant
- trait method/associated type
- visibility
- generic constraint
- async
- dynamic dispatch surface

### Effect

- add/remove capability
- relabel
- read→write authority
- finite deadline requirement
- retry semantics metadata
- observer-only change

### Wire

- fresh optional field
- required field
- ID reuse
- reserve
- scalar kind change
- enum unknown handling
- nested message
- default semantics
- unknown preservation
- schema edition

classification output は golden JSON。

## 9. MendrelBench overview

MendrelBench は language-model code generation benchmark ではなく、**human/agent repository maintenance benchmark**。

比較対象の初期候補:

- Rust
- Go
- TypeScript
- Kotlin
- Mendrel

必要に応じて Python/OCaml/Koka 等の profile を追加するが、各言語の idiomatic production implementation を使う。

## 10. Benchmark task families

### 10.1 Navigation

- bug description から relevant symbol/test を見つける
- hidden/renamed domain vocabulary
- generated code と source code の区別
- transitive caller/effect origin の探索

metrics:

- tokens read
- files/symbols inspected
- time/tool calls
- irrelevant context ratio
- correct focus recall

### 10.2 Local repair

- type mismatch
- missing enum arm
- wrong error mapping
- wrong capability label
- resource misuse
- await/Send violation
- wire compatibility violation

metrics:

- compiler iterations
- diagnostic utilization
- patch size
- final correctness
- unrelated edits

### 10.3 Cross-file change

- function signature migration
- capability split
- sync→async
- record/newtype migration
- trait method addition
- module move/rename

metrics:

- compile pass
- caller coverage
- API/effect blast radius prediction
- stale edit conflict
- review time
- regression

### 10.4 Production bug

- duplicate payment on retry
- missing deadline
- swallowed cancellation
- leaked transaction
- secret logged
- unbounded channel
- schema rollout incompatibility
- non-reproducible build input
- permission overgrant

metrics:

- bug detection
- correct invariant
- test added
- production policy passed
- false fix rate

### 10.5 Incident diagnosis

input:

- source revision
- artifact metadata
- trace/log/metric
- panic/task tree
- deployment grants
- partial issue description

task:

- root cause
- affected scope
- safe patch
- regression test
- rollout/rollback note

metrics:

- diagnosis accuracy
- evidence citation
- hallucinated fact
- repair success
- operational safety

### 10.6 Migration

- API SemVer
- wire schema expand/backfill/contract
- edition migration
- dependency upgrade
- FFI wrapper replacement
- capability least-authority tightening

metrics:

- compatibility
- staged plan
- old/new coexistence
- rollback viability
- machine diff use

## 11. Corpus construction

memorization contamination を減らす。

- freshly generated repositories
- hidden package/symbol names
- semantic isomorph variants
- generated domain fixtures
- unpublished/private task set
- periodic rotation
- public methodology + hidden final instances
- paired implementation across languages
- same business semantics, language-idiomatic design
- task author and evaluator separation
- test oracle independent of model

既存 open-source issue も external-validity set として使えるが、training contamination を前提に解釈する。

## 12. Agent conditions

比較条件を分ける。

1. raw text + shell
2. LSP
3. language-native structured diagnostics
4. Mendrel MAP
5. MAP without learned ranking
6. human-only
7. human + agent

これにより「言語構文の効果」と「tool interface の効果」を分離する。

model/version/context budget/tool budget を固定。temperature/seed、retry policy、human intervention を記録。

## 13. Primary metrics

### Correctness

- tests pass
- hidden tests pass
- static gates pass
- production invariant pass
- no introduced vulnerability
- compatibility class correct

### Efficiency

- input/output tokens
- context bytes
- tool calls
- compile/test iterations
- wall-clock under controlled hardware
- files touched
- diff size

### Reliability

- success@1
- success within N iterations
- variance across seeds
- hallucinated symbol/API rate
- stale patch rate
- regression rate
- nondeterministic failure rate

### Maintainability

- human review time
- reviewer defect detection
- explanation accuracy
- blast-radius prediction
- follow-up fix count
- code churn after 30/90 days
- API/effect/unsafe surface growth

### Operations

- incident resolution
- reproducibility
- rollback success
- trace-to-source linkage
- secret/permission violation
- tail-latency regression

## 14. Human review study

reviewer に提供する情報を段階化。

- plain diff
- diff + compiler diagnostics
- diff + MAP blast-radius report
- diff + generated proof/compatibility/artifact report

測定:

- approve/reject correctness
- defect detection
- time
- confidence calibration
- irrelevant warning burden
- explanation comprehension

「agent の成功」だけでなく、人間が agent patch を安全に受け入れられるかを測る。

## 15. Statistical plan

- task を primary endpoint 前に固定
- paired comparison
- language/task/model random effects
- confidence interval
- effect size
- multiple comparison correction
- failure taxonomy
- censored budget handling
- preregistered exclusions
- raw event log and artifact preservation where licensing permits

一つの aggregate pass rate だけで結論を出さない。task family ごとの trade-off を見る。

## 16. Failure taxonomy

- navigation failure
- requirement misunderstanding
- hallucinated symbol
- syntax
- name resolution
- type/effect
- resource/concurrency
- test inadequacy
- production policy
- API/schema compatibility
- security
- stale edit
- overbroad refactor
- under-scoped patch
- nondeterminism
- tool/protocol failure
- evaluator oracle issue

同じ compiler error を何度も出す場合、model だけでなく diagnostic/interface の欠陥候補として扱う。

## 17. Acceptance hypothesis

Mendrel が比較対象に対して期待するのは、すべての task で fastest になることではない。

期待:

- raw coding microbenchmark では学習量の多い言語に負ける可能性
- repository navigation、cross-file repair、effect/resource/schema bug で優位
- compiler iteration と hallucinated symbol が減る
- public boundary は冗長でも review time が下がる
- production invariant violation の事前検出が増える
- MAP の効果が syntax の効果より大きい task もある

この期待と違う結果も公開する。Mendrel の機能が複雑さを増やしただけなら削る。

## 18. Continuous quality dashboard

compiler repository で追う。

- conformance pass
- fuzz hours/crashes
- differential mismatches
- parser incremental latency
- type/effect query invalidation
- compiler memory
- runtime benchmark
- GC pause
- reproducible artifact rate
- diagnostic repair success on fixed agent suite
- MAP semantic edit success
- API/schema false classification
- unsafe surface
- dependency risk

feature merge で dashboard が悪化した場合、性能/正確性 budget を超える理由を ADR にする。

## 19. Benchmark anti-pattern

禁止:

- toy one-function generation だけで LLM suitability を主張
- Mendrel だけ専用 tool、比較言語は raw text
- public benchmark を学習済み model で一度だけ評価
- compiler error を test failure と別カウントして有利に見せる
- human review を省く
- hidden operational regression を無視
- token 数だけで correctness を無視
- best seed/model run だけ報告
- 言語に不自然な実装を比較対象へ押し付ける
- 未成熟 compiler の crash を task failure taxonomy から外す
