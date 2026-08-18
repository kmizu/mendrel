# 06. Reference compiler and runtime architecture

## 1. 実装方針

reference implementation は Rust で作る。理由は、明示的な ownership、安全な低レベル実装、LLVM/Wasm ecosystem、既存 debugger/sanitizer/tooling との接続を得るためや。Mendrel の source semantics 自体を Rust の borrow model へ合わせる必要はない。

self-hosting は初期目標にしない。仕様、diagnostic、runtime ABI、conformance が安定する前の self-host は、compiler bug と language bug を相互に隠しやすい。

## 2. Architecture principles

- lossless/error-tolerant CST と semantic AST/HIR を分離
- immutable interned data と query-based incremental computation
- source span を全層で追跡
- builtin/diagnostic/syntax/runtime intrinsic の single source of truth
- parser/checker/codegen が deterministic
- compiler phase 間の typed boundary
- invalid/incomplete program でも可能な範囲の HIR/query を返す
- error recovery のために soundness を緩めない
- release artifact 生成前に verifier を独立実行
- evaluator/native/Wasm の意味差を differential test
- runtime collector/backend を adapter boundary で分離
- compiler daemon と batch compiler が同じ core library を使う

## 3. Repository structure

```text
.
├── Cargo.toml
├── rust-toolchain.toml
├── README.md
├── AGENTS.md
├── docs/
├── examples/
├── schemas/
├── spec/
├── crates/
│   ├── mendrel-source/
│   ├── mendrel-diagnostics/
│   ├── mendrel-syntax/
│   ├── mendrel-parser/
│   ├── mendrel-format/
│   ├── mendrel-ast/
│   ├── mendrel-hir/
│   ├── mendrel-resolve/
│   ├── mendrel-types/
│   ├── mendrel-effects/
│   ├── mendrel-contracts/
│   ├── mendrel-mir/
│   ├── mendrel-verify/
│   ├── mendrel-codegen-llvm/
│   ├── mendrel-codegen-wasm/
│   ├── mendrel-runtime-abi/
│   ├── mendrel-runtime/
│   ├── mendrel-package/
│   ├── mendrel-artifact/
│   ├── mendrel-map/
│   ├── mendreld/
│   ├── mendrelc/
│   └── mendrel-cli/
├── runtime/
│   ├── gc/
│   ├── scheduler/
│   ├── platform/
│   └── ffi/
├── stdlib/
├── tests/
│   ├── conformance/
│   ├── ui/
│   ├── differential/
│   ├── runtime/
│   └── production/
├── fuzz/
└── xtask/
```

これは最終責務図であり、bootstrap 時に空 crate を全部作らない。crate boundary は compile-time/ownership/API の理由が成立した時点で分割する。

## 4. Compilation pipeline

```text
Source bytes
  ↓
Line index / source database
  ↓
Lexer
  ↓
Lossless CST + recovery nodes
  ↓
AST view
  ↓
Module graph / name resolution
  ↓
Typed HIR
  ├─ type constraints
  ├─ capability/effect rows
  ├─ contracts
  ├─ trait resolution
  └─ public API/schema model
  ↓
MIR
  ├─ explicit control flow
  ├─ ownership/resource moves
  ├─ task scope/cancellation
  ├─ cleanup edges
  ├─ checked arithmetic
  └─ unsafe operations
  ↓
MIR verifier
  ↓
Optimization IR
  ├─ LLVM IR
  └─ Wasm Component lowering
  ↓
Object/component
  ↓
Link/package
  ↓
MRA artifact + metadata
```

各 arrow は pure query または明示 input を持つ deterministic transform とする。

## 5. Source database

source database が持つもの:

- file ID
- package/module mapping
- UTF-8 bytes
- content digest
- line start index
- overlay/base origin
- generated/source origin
- generator provenance
- edition
- path normalization
- source map chain

file ID は snapshot-local。path string を identity にしないが、hidden persistent ID も正本にしない。

line/column:

- byte span が canonical
- Unicode scalar column と UTF-16 column は client rendering 用派生
- invalid UTF-8 は source load diagnostic。lexer に不正 byte を流さない
- CRLF input は span mapping を保ったまま parse し、format で LF canonical

## 6. Lexer

lexer requirements:

- deterministic
- full-fidelity token/trivia
- nested block comment
- Unicode identifier normalization/security metadata
- numeric literal shape/overflow prefix metadata
- string escape diagnostic
- incremental relex window
- invalid token を token stream に保持
- keyword table generated from spec source

lexer は semantic name resolution をしない。contextual keyword を作らないことで token kind を局所に固定する。

fuzz property:

- no panic for arbitrary UTF-8
- token spans cover input without overlap/gap
- concatenated token text equals input
- incremental relex equals full relex

## 7. Lossless CST parser

### 7.1 CST

CST は:

- token/trivia 全保持
- error/missing node
- stable syntax kind
- parent/child structure
- byte span
- green tree sharing
- incremental reparse
- malformed subtree

を持つ。

### 7.2 Parser approach

候補:

1. hand-written recursive descent + Pratt expression parser
2. parser generator/GLR

v0.1 は **hand-written recursive descent + fixed Pratt table** を推奨。

理由:

- grammar が意図的に曖昧でない
- recovery strategy を declaration/block/list ごとに調整しやすい
- missing token と error node を lossless tree へ統合しやすい
- diagnostic code/source span を精密に制御しやすい
- incremental reparse boundary を設計しやすい
- generator runtime/grammar dialect 依存を減らす

ただし grammar drift を防ぐため、`spec/grammar.ebnf` と parser test inventory を機械照合する。

### 7.3 Recovery

synchronization token:

- `;`
- `,`
- `}`
- top-level declaration keyword
- `case`/pattern arm boundary
- `else`
- end-of-file

recovery は token を大量に捨てず、`ERROR` node へ包む。missing delimiter は zero-width `MISSING_*` token を挿入し、fix を提供。

diagnostic storm を抑えるため、root cause suppression を cause graph で表す。後続 semantic query は unknown/error type を使い継続するが、release success にはならない。

## 8. Formatter

formatter input は CST。AST/HIR から source を再生成しない。

pipeline:

1. CST + trivia classify
2. comment attachment
3. layout document
4. deterministic line breaking
5. output
6. parse/structural equivalence check in debug/test
7. idempotence test

generated source も同じ formatter。

partial malformed formatting は safe island だけ。unknown/error region の raw text を保持し、周囲 delimiter を壊さない。

## 9. AST and HIR

### 9.1 AST

AST は CST の typed view。trivia を通常 semantic operation から隠すが、source node/anchor へ戻れる。

AST node を独立 mutable object にせず、CST pointer/range と typed accessor にする選択を優先。

### 9.2 HIR

HIR では desugar を明示。

- `?`
- `if`/`match`
- method call
- `use` cleanup
- async/await
- task scope
- contract
- derive
- wire declaration

ただし user-facing diagnostic は desugared internal syntax ではなく source construct へ map。

HIR ID は snapshot-local query key。source ancestry を持つ。

HIR node の例:

```text
HirFunction {
  symbol,
  signature,
  body,
  effect_row,
  contracts,
  source,
}

HirCapabilityCall {
  capability_label,
  capability_type,
  method,
  args,
  source,
}

HirUseResource {
  binding,
  initializer,
  cleanup,
  body,
  source,
}
```

## 10. Name resolution

二段階:

1. module/import/export symbol collection
2. body lexical resolution

規則:

- explicit imports
- no wildcard in release
- no shadowing
- trait method scope explicit
- direct dependency only
- same module mutual recursion
- package/module DAG
- generated module provenance

resolver output は candidate set/why-not information を保持し、diagnostic/MAP completion に使う。

symbol table は public/private/generated/unsafe/wire/capability metadata を持つ。

## 11. Type/effect engine

### 11.1 Representation

type interner:

- primitive
- nominal type constructor
- tuple/function
- generic parameter
- associated type projection
- capability row
- inference variable
- error type

effect row:

```text
Row {
  entries: sorted map<label, CapabilityType>,
  tail: Closed | RowVar,
}
```

source order は diagnostic rendering metadata として別保持し、unification は canonical order。

### 11.2 Constraint solving

- local bidirectional checking
- union-find/type variable
- occurs check
- trait obligation queue
- effect row equality/subset/lacks
- no global backtracking overload search
- bounded candidate resolution
- deterministic tie rejection
- cause ID for each constraint

solver は最初に見つけた arbitrary error を返さず、conflict core と source path を作る。完全な minimal unsat core が高価なら、deterministic bounded explanation algorithm を使う。

### 11.3 Public signature check

body inferred type/effect が declared signature の subtype/subset/contract requirement を満たすか検査。body がより少ない effect を使うのはよい。undeclared capability は error。

## 12. Contracts

contract frontend は pure HIR subset へ lowering。

normal compiler:

- well-formedness
- purity
- termination/boundedness
- simple constant/range proof
- runtime check insertion
- contract ID/fingerprint

optional verifier:

- verification condition generation
- solver adapter
- proof result/provenance
- timeout/unknown
- counterexample mapping

normal compilation success を solver success に依存させない。

## 13. MIR

MIR は control-flow graph。source-level sugar を消し、安全性に必要な operation を明示する。

instruction categories:

- local assign/move/copy
- aggregate construct/project
- checked arithmetic
- call/capability call
- borrow begin/end
- resource acquire/consume/cleanup
- task scope enter/exit
- spawn/join/cancel
- await/suspend
- deadline enter/exit
- panic
- contract check
- unsafe intrinsic
- GC allocation/root
- FFI call
- branch/switch/return

MIR block edge は normal/error/panic/cancel/cleanup を区別できる。

## 14. MIR verifier

codegen 前に独立 verifier。

checks:

- SSA/local definition
- type correctness
- dominance
- resource affine use
- cleanup on all exits
- no borrow/guard across await
- task handle scope containment
- spawn capture `Send`/`Share`
- capability call present in function row
- unsafe intrinsic inside unsafe provenance
- contract check placement
- GC root liveness
- no impossible lowering state
- debug/release independent semantics

verifier failure は user error ではなく ICE。ただし unsafe wrapper violation 等、source rule に戻せるものは frontend diagnostic で止める。

## 15. Reference evaluator

初期意味検証のため、typed HIR または MIR evaluator を持つことを推奨。

用途:

- language conformance
- constant evaluation
- doctest
- differential test
- deterministic concurrency model
- compiler bootstrap debugging

制限:

- production performance model ではない
- FFI/native-only capability は fake/host adapter
- evaluator 固有 behavior を仕様にしない
- native/Wasm と同じ runtime result/error serialization を使う

differential corpus で evaluator、unoptimized LLVM、optimized LLVM、Wasm の結果を比較。

## 16. LLVM backend

LLVM を initial native backend とする。

lowering responsibilities:

- Mendrel ABI
- GC statepoint/stack map strategy
- checked arithmetic
- panic/unwind or abort profile
- async state machine
- task/runtime calls
- trait/static/dynamic dispatch
- debug info
- source maps
- sanitizer-compatible FFI
- codegen determinism
- LTO/PGO
- artifact metadata locator

LLVM version/toolchain を pin。IR text を public stable interface にしない。

## 17. Wasm Component backend

Wasm backend:

- core Wasm lowering
- canonical ABI/component wrapper
- WIT world/interface generation/import
- resource handles
- async/stream/future mapping where target supports
- capability imports
- sandbox policy
- source maps
- deterministic component composition

Mendrel capability row と component imports の対応を明示できる。すべての internal capability を必ず component import にするわけではなく、composition で実装されたものは内部になる。

## 18. Runtime ABI

compiler/runtime ABI は versioned internal contract。

sections:

- object layout descriptor
- GC allocation/root/barrier
- task/scheduler
- resource cleanup
- panic/report
- capability dispatch
- async state machine
- telemetry hooks
- FFI attach/pin
- metadata registration

artifact は required runtime ABI version。mismatch は load 前に拒否。

source language の stable ABI と混同しない。

## 19. Incremental query system

query example:

```text
source_text(file)
lex(file)
parse(file)
module_items(module)
resolve_signature(symbol)
type_of(expr)
effects_of(symbol)
mir(symbol)
codegen_unit(symbol_set, target)
api_snapshot(package)
impacted_tests(change_set)
```

query key/input/output は deterministic/hashable。cycle detection は module/type/effect recursion の規則に応じた diagnostic。

red/green invalidation または同等方式を使う。performance counter:

- executed/reused query
- invalidation cause
- peak memory
- serialization cache
- cold/warm latency

`mendrel explain incremental <file>` で変更が何を再計算したか表示できる。

## 20. Compiler daemon

`mendreld`:

- workspace snapshot
- query DB
- LSP endpoint
- MAP endpoint
- background indexing
- bounded caches
- cancellation
- multi-client overlays
- crash isolation/restart
- protocol version negotiation

daemon crash が source/build を破壊しない。batch `mendrelc` が同じ core library で再現できる。

## 21. Single source generated assets

`spec/` に machine-readable source を置く候補:

```text
syntax-kinds.yaml
keywords.yaml
operators.yaml
diagnostics.yaml
builtins.yaml
intrinsics.yaml
editions.yaml
wire-primitives.yaml
map-methods.yaml
```

bootstrap pack では文書/EBNF/JSON schema から始め、実装時に重複が発生する前に上記へ正規化する。

生成:

- Rust enums/tables
- formatter token classes
- docs reference table
- JSON schema enum
- diagnostic catalog
- syntax highlighter data
- conformance fixtures
- MAP method registry

`xtask generated --check` が dirty output を拒否。

## 22. Diagnostic architecture

diagnostic builder は compiler phase ごとに ad-hoc JSON を作らない。

```text
DiagnosticFact
  ↓
Catalog code/template/schema
  ↓
Structured Diagnostic
  ├─ human renderer
  ├─ JSONL renderer
  ├─ LSP conversion
  └─ SARIF conversion
```

message localization を将来追加しても stable code/structured facts は不変。

ICE crash bundle:

- compiler/runtime version
- query key
- source revision
- sanitized backtrace
- relevant HIR/MIR digest
- deterministic replay command
- secret/source inclusion policy

## 23. Standard library architecture

stdlib layer:

1. core language prelude: `Option`, `Result`, primitive traits
2. pure collections/text/time value
3. resource abstractions
4. async/task/channel/actor
5. capability interfaces
6. production adapters
7. wire/serialization
8. testing fakes/property tools

core prelude は小さく、implicit import symbol を固定。HTTP/DB/framework を prelude に入れない。

capability interface と implementation package を分離。

## 24. Compiler testing

- lexer/parser unit
- CST golden
- formatter golden/idempotence
- UI diagnostic structured/human
- resolver/type/effect conformance
- trait coherence
- resource/task negative tests
- MIR verifier
- evaluator/native/Wasm differential
- optimizer miscompile fuzz
- GC stress
- FFI sanitizer
- incremental edit equivalence
- deterministic build
- MAP transaction/revision race
- API/effect/schema diff corpus

test failure は seed/revision/toolchain を出す。

## 25. Bootstrapping sequence

最短で意味のある縦切りを作る。

1. source/span/diagnostic
2. lexer/CST/parser/formatter
3. module/resolver
4. nominal type/ADT/function
5. evaluator
6. `Result`/pattern
7. capability/effect
8. MIR
9. LLVM + simple GC
10. resource
11. async/task
12. package/artifact
13. LSP/MAP
14. wire/compatibility
15. production hardening

LLVM/GC を早く入れすぎて syntax/type 設計の変更コストを上げない一方、evaluator だけで長期間進めて backend feasibility を見落とさない。Phase 5 前後で小さい LLVM spike を別 branch で検証し、main implementation は縦切り順を守る。

## 26. Compiler non-goals early

- self-hosting
- optimizing JIT
- multiple native backends
- arbitrary macro expansion engine
- plugin compiler pass API
- stable HIR/MIR public API
- distributed compiler
- IDE-only semantic fork
- runtime hot swap
- whole ecosystem package registry before local package works
- general theorem prover
