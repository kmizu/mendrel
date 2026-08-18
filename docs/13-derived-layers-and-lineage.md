# 13. Derived layers and our design lineage

## 1. Status

この文書は Mendrel core 1.0 の必須構文を増やすためのものではない。

Mendrel の小さい意味核を使って、後から構築できる高価値な公式 layer を定義する。目的は、Onion、ASTER、Klassic、Macro PEG / Consume-L で得た強い設計を捨てず、同時にそれらを全部 core language へ押し込んで巨大化させないことや。

優先順位は次の通り。

1. Mendrel core の型・effect・resource・task・artifact semantics を安定させる
2. 普通の service/CLI/batch を production trial へ出す
3. 同じ core 上へ Data、Tool、Agent、Syntax layer を追加する
4. 複数 layer で反復して必要になった機構だけを edition RFC で core 候補にする

この順序を逆転させない。

## 2. 共通原則

すべての derived layer は次を守る。

- 新しい ambient authority を作らない
- text source と canonical formatter を正本にする
- public contract、effect、resource、wire、artifact metadata へ重要な意味を出す
- generated source/IR の入力、generator、version、digest、source map を残す
- compiler/runtime が判定できる性質を LLM judge や prose convention に逃がさない
- runtime AI を使う場合も、AI を executor、security principal、policy oracle にしない
- normal build の再現性を壊す network/clock/randomness を compile-time に持ち込まない
- library で済むものへ専用 syntax を先に与えない

要約すると、**AI は候補を作り、Mendrel の型・validator・policy・runtime が判決する。**

## 3. Mendrel Data

### 3.1 目的

Onion で育てた中心案は、parse、print、validate、generate、shrink、schema、documentation、partial update を別々の ad-hoc API にしないことや。

ただし、一個の万能 type class に全意味を押し込むと law と failure model が曖昧になる。Mendrel Data は一つの root descriptor と、能力別の明示 interface に分ける。

```mendrel
pub opaque type Shape<T>;

pub trait Decode<Source, T> {
    type Error;
    fn decode(source: Source) -> Result<T, Self.Error>;
}

pub trait Encode<T, Target> {
    type Error;
    fn encode(value: T) -> Result<Target, Self.Error>;
}

pub trait Generate<T> {
    fn generate(seed: Seed, size: U32) -> T;
}

pub trait Shrink<T> {
    fn shrink(value: T) -> Stream<T>;
}
```

`Shape<T>` は nominal type、field/variant、constraint、wire ID、redaction、default policy から compiler が作る immutable descriptor であり、実行可能な任意 code ではない。

### 3.2 Derived products

型と明示 adapter から、必要に応じて次を生成する。

- JSON/CSV/line-oriented/config decoder
- encoder
- JSON Schema / OpenAPI / WIT / Protobuf adapter metadata
- generator と shrinker
- field-level documentation
- compatibility classifier
- form/CLI/MCP input schema
- source-preserving lens

すべてを自動 derive できるとは仮定しない。secret、resource、function、unbounded recursive type、domain-specific invariant は derive failure を明示し、generator 不足を silent skip しない。

### 3.3 Parse result

壊れた production input を一件の例外文字列へ潰さない。

```mendrel
pub record ParseIssue {
    code: ParseIssueCode,
    span: SourceSpan,
    path: DataPath,
    raw_fragment: Option<Text>,
    expected: ExpectedShape,
    recovery: Option<Recovery>,
}

pub record ScanReport<T> {
    values: List<T>,
    issues: List<ParseIssue>,
    consumed: SourceRange,
}
```

batch/ETL では valid row と invalid row を同時に保持できる。strict decode は `Result<T, ParseReport>`、stream scan は `ScanReport<T>` を使う。`null`、ignored row、warning-only drop を既定にしない。

### 3.4 Lossless Source Lens

通常の serializer は次を行う。

```text
source -> value -> newly formatted source
```

Mendrel Data の source lens は次を目標にする。

```text
original lossless source + modified typed value
    -> minimal source patch
```

可能な範囲で保持する。

- comment
- whitespace
- field order
- unknown field
- quote style
- line ending
- original source span
- parser recovery island

```mendrel
use document = SourceDocument.open::<TomlSubset, AppConfig>(path)?;
let updated = document.value with {
    server: document.value.server with {
        port: 8443,
    },
};
let patch = document.plan_update(updated)?;
files.apply(patch)?;
```

`plan_update` は pure proposal を返し、filesystem capability を直接持たない。conflict は base source digest と anchors で検出する。

### 3.5 Provenance

すべての ordinary value に重い provenance を背負わせない。boundary で必要な値だけを明示 wrapper にする。

```mendrel
pub opaque type Sourced<T>;

fn value<T>(source: &Sourced<T>) -> &T;
fn origin<T>(source: &Sourced<T>) -> OriginGraph;
fn explain<T>(source: &Sourced<T>) -> Explanation;
```

origin graph は source span、decoder、validation、conversion、migration を DAG として保持できる。secret/raw PII は provenance へそのまま保存せず、redacted digest と classification を使う。

### 3.6 Laws

Data layer は少なくとも次を property/conformance suite にする。

- decode(encode(value)) round-trip within declared normalization
- source lens no-op update is byte-identical
- unrelated field update preserves unknown/trivia regions
- generated value satisfies declared constraints
- shrink candidates remain valid unless invalid-input shrinking is explicitly selected
- compatibility classifier agrees with wire corpus
- every recovery retains a source span and stable issue code

「law」と呼ぶ場合、finite exhaustive、property test、proof certificate のどれで検証したかを artifact に出す。

## 4. Mendrel Tool

### 4.1 ToolContract IR

一つの typed public function から CLI、MCP、HTTP、local form を生成できる。ただし function そのものを勝手に外部公開しない。manifest の explicit exposure が必要や。

```mendrel
pub async fn summarize_failures(
    input: FileRef,
    minimum_status: HttpStatus,
) -> Result<FailureReport, SummarizeError>
uses {
    files: ReadFiles,
}
{
    // ...
}
```

```text
expose summarize_failures as cli "summarize-failures"
expose summarize_failures as mcp "logs.summarize_failures"
```

compiler は次を `ToolContract` IR へ落とす。

- nominal input/output/error schema
- defaults and constraints
- capability/effect requirements
- secret/sensitive classification
- streaming/cancellation/deadline behavior
- idempotency and retry class
- examples/properties
- wire adapters
- version and compatibility fingerprint

adapter は ToolContract から生成し、CLI/MCP/HTTP ごとに別 reflection logic を持たない。

### 4.2 Plan before effect

変更系 tool は pure plan と effectful apply を分ける。

```mendrel
pub record Plan<Action> {
    action: Action,
    preconditions: List<Precondition>,
    preview: ChangePreview,
    base_revision: Digest,
}
```

```mendrel
let plan = config.plan_update(request)?;
let approved = policy.authorize(plan)?;
config.apply(plan, approved).await?;
```

`Plan<Action>` 自体は authority ではない。apply は capability と stale-revision check を要求する。これにより dry-run、human approval、audit、retry classification を一つの flow にできる。

## 5. Mendrel Agent

### 5.1 二つの問題を分ける

Mendrel core は「coding agent が code を安全に変更する」問題を MAP で扱う。

Mendrel Agent layer は「production application が model inference を使い、外部 action を行う」問題を扱う。両者を混同しない。

### 5.2 Candidate and Checked

model output は普通の `T` にならない。

```mendrel
pub opaque type Candidate<T>;
pub opaque type Checked<T>;

pub capability Model<Prompt, Output> {
    async fn infer(prompt: Prompt) -> Result<Candidate<Output>, ModelError>;
}

pub trait Validator<T> {
    type Error;
    fn validate(candidate: Candidate<T>) -> Result<Checked<T>, Self.Error>;
}
```

invariant:

- `Candidate<T>` に public value projection、cast、pattern escape、generic serialization escape を持たせない
- validator は pure、deterministic、total over declared input
- provider は capability/tool handle を受け取らない
- external response は declared wire type へ decode してから Candidate になる
- prompt instruction と untrusted/runtime data を別 channel にする
- `Secret<T>` は prompt/model data に入らない

### 5.3 Proposal, Permit, Commit, Reconciliation

read と write を分ける。

- read operation: typed observation capability
- write operation: intent → proposal → authorization → commit → reconciliation

```mendrel
pub opaque resource Permit<Action>;

let intent = PaymentIntent.create(checked_request);
let proposal = payments.propose(intent).await?;
let permit = policy.authorize(&proposal)?;
let outcome = payments.commit(proposal, move permit).await?;
let reconciled = payments.reconcile(outcome).await?;
```

`Permit<Action>` は affine、single-use、expiring、exact proposal digest に binding される。source code は authority を mint/broaden できない。runtime configuration が grant し、delegation は narrower subset だけを許す。

### 5.4 Budget

model/tool/approval/write は explicit multidimensional budget を使う。

- model calls
- input/output tokens
- external reads
- external writes
- approvals
- money micro-units
- task/time deadline

variable cost は effect 前に deterministic upper bound を reserve し、result 後に settle する。budget failure は driver invocation 前に起き、trace で before/reserved/actual/released/after を残す。

### 5.5 Durable flow and replay

長時間 agent/workflow では、effect boundary ごとに typed serializable control state を snapshot できる。ただし ordinary Mendrel function を暗黙に durable にしない。明示 `DurableFlow` compilation profile/library を使う。

snapshot は少なくとも次を持つ。

- schema/compiler/runtime version
- normalized program/artifact digest
- flow/event identity and input digest
- instruction pointer and frames
- locals/pending state delta
- capability grant fingerprint
- affine ledger
- remaining/reserved budget
- trace position/hash
- pending effect request

replay は model/tool/approval driver を一切呼ばず、recorded effect result と budget transition を再検査する。program、request、effect order、trace が変われば divergence にする。

### 5.6 Scope discipline

Agent layer は core 1.0 の block 条件にしない。先に ASTER-style vertical slice を別 package/runtime として実証し、次を満たしてから official support を昇格する。

- fixture-backed end-to-end run
- append-only hash-chained trace
- zero-driver replay
- tamper/divergence rejection
- Candidate escape negative tests
- Permit double-use/use-after-move rejection
- secret non-leak tests
- budget-before-driver test
- human approval suspension/resume

## 6. Mendrel Syntax

### 6.1 なぜ syntax macro ではないか

SQL、regular expression、routing pattern、binary protocol、parser grammar の stringly API は production bug と injection を生む。一方、一般 token macro と arbitrary compile-time execution は source locality、diagnostic、security、agent comprehension を壊す。

そこで、Macro PEG / Consume-L から得た知見を、閉じた typed syntax island として使う。

### 6.2 Grammar declaration

将来の候補:

```mendrel
syntax EmailAddressText -> EmailAddress
using peg {
    // bounded declarative grammar
}
```

必要条件:

- grammar は declarative data
- left-recursion/termination/ambiguity policy を compile time に検査
- generated parser/printer は ordinary typed IR へ lowering
- source span と recovery code を保持
- arbitrary host code action を grammar production に埋め込まない
- generator、shrinker、formatter、syntax highlighter を同じ grammar inventory から導出
- grammar digest と generator version を artifact に記録
- expansion/generated code を inspect 可能にする

v1 では compiler-owned SQL/wire/format literalsだけに限定し、user-defined grammar は研究/experimental edition feature とする。

### 6.3 Typed literal

```mendrel
let query = sql::<OrderRow>(schema: OrdersV3) """
    SELECT id, total_amount
    FROM orders
    WHERE id = :id
""";
```

literal parser が返すのは raw `Text` ではなく nominal typed value。interpolation point の型、escaping、effect boundary、schema digest を検査する。

## 7. Klassic から継ぐ実装規律

Klassic で重要やったのは、華やかな frontend 機能より evaluator/native parity と target/runtime separation や。

Mendrel では次を明示的に継ぐ。

- reference evaluator を oracle として残す
- representative program を evaluator、LLVM、Wasm で differential test する
- target OS/arch/ABI/object format/runtime strategy を `TargetSpec` data にする
- plain Mendrel で書ける stdlib logic は compiler builtin にしない
- backend-specific branch を frontend/type checker に散らさない
- huge implementation file の機械的分割と semantic change を同じ PR に混ぜない

## 8. 何を core へ昇格させないか

次は derived layer で価値があっても、現時点では core syntax にしない。

- `tool fn`
- `flow` keyword
- model prompt literal
- actor declaration
- arbitrary user grammar
- general lens syntax
- general effect handler
- policy language
- distributed transaction/saga syntax

型、trait、capability、resource、manifest、compiler-known derive、sandboxed generator でまず実証する。

## 9. Layer acceptance metrics

### Data

- malformed input recovery precision/recall
- byte-preserving no-op/edit rate
- unknown-field preservation
- generated test effectiveness
- schema compatibility classifier accuracy

### Tool

- adapter parity across CLI/MCP/HTTP
- authority overgrant rate
- preview/commit stale-conflict detection
- generated documentation/schema drift

### Agent

- zero-driver replay success
- unsafe action rejection
- secret leakage rate
- budget accounting divergence
- human approval resume correctness

### Syntax

- parse/print round-trip
- malformed recovery quality
- injection defect reduction
- generated diagnostic source accuracy
- compile/query cost

数値が既存 library/tooling を上回らない layer は、公式化せず実験 package のままにする。

## 10. 結論

Mendrel の独自性は、過去の各プロジェクトを一個の巨大言語へ足し算することやない。

- Onion の typed boundary と lossless transformation
- ASTER の governed effect と replay
- Klassic の evaluator/backend discipline
- Macro PEG / Consume-L の restricted syntax extensibility

を、**小さい core の上で互いに検証可能な layer として接続すること**にある。

この構造なら、良い発明を残しつつ、LLM と人間が一度に理解せなあかん世界は増やさずに済む。
