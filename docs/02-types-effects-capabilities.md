# 02. Types, effects, capabilities, errors, and contracts

## 1. 設計目標

型システムは、表現力の最大化ではなく次を最適化する。

- declaration を単独で理解できる
- inference failure の原因が局所的である
- public API の意味が implementation body に依存しない
- effect/authority/error が call graph 上で追跡できる
- resource と task boundary の安全性を静的に検査できる
- compiler が machine-applicable repair を提示できる
- language server と batch compiler の結果が一致する
- normal build が solver の heuristics に依存しない

## 2. 型の分類

Mendrel は型を意味上、次へ分類する。

| 分類 | 例 | copy | mutation | task crossing | deterministic cleanup |
|---|---|---:|---:|---:|---:|
| scalar value | `I64`, `Bool`, `Instant` | yes | no | yes | no |
| immutable aggregate | `record`, `enum`, `List<T>` | reference/value semantics | no | `Share` if fields | no |
| local object | `object` | alias in task | yes | no by default | no |
| owned isolated graph | `Owned<T>` | no | allowed through owner | `Send` | no |
| affine resource | `resource` | no | API-defined | `Send` if declared/derived | yes |
| capability | `capability` | controlled handle | implementation-defined | policy/type-defined | host-defined |
| dynamic boundary | `Dynamic` | yes | no | yes | no |
| secret wrapper | `Secret<T>` | restricted | no | policy-defined | zeroize best effort |

この分類は syntax sugar ではなく、HIR/MIR verifier と artifact metadata が共有する。

## 3. Type inference

### 3.1 Boundary rule

次の declaration は完全な signature を必須とする。

- top-level function
- method
- trait method
- capability method
- resource method
- public constant
- exported lambda
- service entry
- FFI declaration
- generator interface

signature が含むもの:

- parameter type
- return type
- type parameter
- associated type constraint
- async marker
- `uses` row
- receiver ownership/borrow mode
- safety marker
- contract surface

body からの推論結果を external contract にしない。

### 3.2 Local inference

次は推論できる。

- `let`/`var` local type
- lambda parameter type when expected type exists
- generic instantiation
- enum/record constructor argument
- pattern binding
- effect row of private local expression
- numeric literal type from expected type

推論は bidirectional とする。expected type を上から送り、synthesized type を下から返す。diagnostic は constraint set を丸ごと表示せず、最短の矛盾 path を返す。

### 3.3 Generalization

let-polymorphism は次をすべて満たす binding だけ。

- immutable
- pure effect row
- resource/capability を capture しない
- mutable object state を capture しない
- syntactic value または compiler が value-equivalent と証明する

mutable/resource/effectful binding は monomorphic。hidden value restriction を避けるため、generalize されなかった理由を diagnostic field `generalization_blocker` に入れる。

### 3.4 No inference cliff across files

module A の body 変更が module B の inferred public type を変えてはならない。incremental compilation と review stability のため、public signature が唯一の module boundary になる。

## 4. Nominality と structural typing

- public record/enum/newtype/opaque type は nominal
- trait conformance は explicit `impl`
- function type と tuple は structural
- capability row は label/type の row
- anonymous structural record は local/private temporary shape に限って MAY
- anonymous structural record を public API、wire schema、persistent storage、trait impl target に使ってはならない

構造的便利さを境界へ持ち出すと accidental compatibility と silent coupling が増えるためや。

## 5. Subtyping

一般 nominal subtyping と class inheritance はない。

許す関係:

- `Never` は全型へ coercion
- lifetime-bounded borrow mode の安全な弱化
- effect row subset
- capability implementation から capability interface への upcast
- `dyn Trait` への explicit boxing/upcast
- numeric conversion は explicit method であり subtyping ではない

function parameter/return variance は internal type relation として扱うが、overload resolution には使わない。

## 6. Generic

通常 call は型推論する。明示 type argument が必要なときだけ、比較演算子と曖昧にならない `::<...>` を使う。

```mendrel
let channel = Channel.bounded::<Event>(capacity: 256);
```

```mendrel
pub fn find<A>(
    items: List<A>,
    predicate: fn(&A) -> Bool,
) -> Option<A>
where {
    A: Share,
}
{
    // ...
}
```

### 6.1 制限

- generic parameter は宣言
- implicit generic parameter はない
- HKT は v1 にない
- associated type はある
- const generic は fixed-size buffer 等の限定 subset から始める
- recursive implicit/type-class search はない
- specialization はない
- overlapping impl はない
- generic default type argument は public API で禁止

### 6.2 Code generation

native backend は strategic monomorphization を行う。

- small value type/primitive は monomorphize
- large reference-shaped type は shared representation を選べる
- code-size budget 超過時は compiler が shared generic body を使う
- choice は semantics を変えない
- `mendrel explain dispatch` と `mendrel explain instantiation` で decision を表示
- artifact に monomorphization summary を持つ

## 7. Trait coherence

`impl Trait for Type` は package universe 上で一意。

orphan rule:

- current package が `Trait` を定義する、または
- current package が `Type` の nominal definition を所有する

newtype で foreign type を包めば local impl を作れる。coherence は lockfile で固定した dependency graph 上で検査する。

trait method lookup は次だけ。

1. inherent method
2. lexical scope に明示 import された trait
3. prelude の固定 trait

候補が複数なら error。優先順位で勝手に決めない。diagnostic は candidate と import origin を返す。

## 8. Copy、Clone、Move

- scalar と compiler-approved small immutable value は `Copy`
- `Copy` は明示 derive 可能だが、resource/capability/secret/object を含む型には不可
- heap-backed immutable aggregate の binding copy は共有 reference semantics でもよいが、観測上 immutable
- 明示的 deep duplication は `Clone`
- affine type は `Copy`/`Clone` 不可
- move は dataflow 上で一度
- partial move は record destructuring 時に許すが、残余状態を MIR で追跡
- destructor/finalizer に任意 user code はない

## 9. Borrow

Mendrel は通常の heap value に Rust 型 lifetime annotation を要求しない。一方、resource operation と FFI の一時 borrow は lexical に扱う。

```mendrel
fn read_header(file: &File) -> Result<Header, IoError> {
    // borrow cannot escape unless return type explicitly carries a view token
}
```

borrow mode:

- `&T`: shared lexical borrow
- `&mut T`: exclusive lexical borrow
- `move T`: ownership consume
- plain `T`: immutable value/handle semantics according to type category

規則:

- borrow は named function boundary を越えて return できないのが既定
- zero-copy view が必要な API は `View<'scope, T>` 相当の compiler-owned scope token を使うが、source で任意 lifetime calculus を露出しない
- borrow/resource guard は `await` を越えられない
- FFI pin は lexical `Pinned<T>` resource
- borrow failure diagnostic は source operation と blocking await/resource action を cause graph で示す

## 10. `Send` と `Share`

compiler-known marker trait:

- `Share`: 複数 task から同時参照して安全
- `Send`: ownership を別 task へ移して安全
- `SuspendSafe`: 値を async suspension point 越しに保持して安全
- `Deterministic`: pure input に対する iteration/serialization 等が再現可能
- `WireSafe`: wire schema へ lowering 可能

原則:

- immutable record/enum は field が `Share` なら `Share`
- affine resource は declaration/implementation が保証した場合のみ `Send`
- local object は neither
- `Owned<T>` は isolated graph verifier が通れば `Send`
- mutex guard、transaction borrow、stack view は not `SuspendSafe`
- capability は interface policy により marker を持つ
- unsafe manual impl は safety proof doc と audit metadata を要求

## 11. Capability

### 11.1 Declaration

```mendrel
pub capability OrdersRepo {
    async fn find(order_id: OrderId) -> Result<Option<Order>, RepoError>;
    async fn save(order: Order) -> Result<Unit, RepoError>;
}
```

capability は次を意味する。

- authority への non-forgeable handle
- method surface
- async/error/type contract
- optional deployment binding metadata
- test double generation surface
- observability boundary

capability は service locator ではない。call site から見えない global lookup は禁止。

### 11.2 Named row

```mendrel
uses {
    primary_db: Database,
    replica_db: Database,
    clock: Clock,
}
```

label は semantic role。type が同じでも label が違えば別 authority。

自動転送は exact label と compatible capability type のみ。

```mendrel
let order = read_from_replica(id)
with {
    database: replica_db,
};
```

callee の label `database` と caller の `replica_db` が違うため、明示 remap が必要。

### 11.3 Row subset

effect/capability row を `e` とする。

\[
e_1 \subseteq e_2
\]

なら、`e_1` しか使わない function は `e_2` を持つ context で呼べる。逆は不可。

duplicate label は source row で禁止。row polymorphism の unification では label identity と capability type を保持する。

### 11.4 Higher-order effect polymorphism

```mendrel
pub fn map<A, B, e>(
    items: List<A>,
    transform: fn(A) -> B uses e,
) -> List<B>
uses e
where {
    e: EffectRow,
}
{
    // ...
}
```

- row variable は signature へ明示
- body から public row variable を発明しない
- row constraint は subset/equality/lacks の限定集合
- arbitrary effect-level computation はない
- diagnostic は instantiated row と source origin を表示

### 11.5 Capability composition

composition root:

```mendrel
let app = CheckoutApp {
    orders: PostgresOrders.connect(config.database)?,
    payments: StripeGateway.connect(config.payments)?,
    clock: runtime.clock(),
};
```

capability implementation は ordinary record/object/resource の組み合わせでよいが、unforgeable constructor は host、sealed module、manifest grant で制御する。

### 11.6 Capability budget

標準 lint:

- `capability-too-wide`: method 数や authority class が閾値超過
- `app-context-capability`: unrelated authority を束ねた巨大 context
- `effect-surface-growth`: public `uses` の追加
- `authority-escalation`: read-only から write/admin への変更
- `ambient-authority`: env/global/host API の直接参照
- `capability-alias-confusion`: label が role を表していない

lint threshold は package policy で厳しくできるが、意味を変える feature flag にはしない。

## 12. Pure function

`uses {}` が空で、mutable/object/resource interaction がなく、panic-free な function は pure と分類できる。

pure guarantee:

- 同じ明示 input に同じ result
- wall clock、random、environment、locale、hash seed、filesystem、network を読まない
- observational mutation をしない
- hidden allocation は許す
- compiler/runtime version を跨ぐ bit-identical float result までは保証しない場合、determinism profile に明示

`pure` keyword を人が宣言して信用するのではなく、compiler が推論・検証し、API metadata に出す。必要なら `requires_pure` constraint で higher-order argument を制約する。

## 13. General algebraic effect handler を v1 に入れない理由

effect handler は強力で、exception、state、async、generator、nondeterminism を統一できる。しかし production v1 では次が負担になる。

- resumable continuation の lifetime
- one-shot/multi-shot の区別
- resource cleanup と cancellation の相互作用
- stack trace と observability の意味
- optimizer と native/Wasm backend の複雑化
- agent が handler scope を見落としたときの非局所 behavior
- FFI callback/async との整合

Mendrel v1 は effect **row** と named capability を採用し、handler **semantics** は採用しない。retry、logging、cache、fake、transaction は capability wrapper/implementation で表現する。

後の edition で再検討する条件は、実 production corpus で wrapper では表現できず、かつ MAP/diagnostic/resource semantics を損なわない用途が複数示された場合に限る。

## 14. Error model

### 14.1 One model

recoverable error は `Result<T, E>` だけ。

```mendrel
pub enum LoadOrderError {
    NotFound { order_id: OrderId },
    StorageUnavailable { retry_after: Option<Duration> },
    InvalidStoredData { record_id: Text },
}
```

- `throw`/`throws`/`catch` はない
- panic は programmer invariant failure
- cancellation と deadline は typed error/cause
- process termination は supervisor/runtime event
- FFI exception は boundary で typed error または panic record へ変換

### 14.2 Stable error shape

public error variant は次を持てる。

- stable machine code
- structured fields
- retryability
- public/private message separation
- source cause
- telemetry classification
- HTTP/RPC mapping adapter

```mendrel
pub enum PaymentError {
    @error(code: "PAYMENT_DECLINED", retry: false)
    Declined {
        reason: DeclineReason,
    },

    @error(code: "PAYMENT_TEMPORARY", retry: true)
    TemporaryFailure {
        retry_after: Option<Duration>,
    },
}
```

error text を API contract にしない。

### 14.3 Conversion

error conversion は explicit mapping が基本。

```mendrel
let payment = payments.authorize(request)
    .await
    .map_error(CheckoutError.from_payment)?;
```

`FromError<Source> for Target` は一意な pure mapping のみ許し、`?` が使った conversion chain を diagnostic/explain で表示する。多段 implicit chain は一段まで。

### 14.4 Panic

`panic` は次に限定。

- compiler/runtime invariant violation
- impossible state reached through unsafe/bug
- explicit `expect` failure
- memory exhaustion 等の runtime fatal condition

panic は routine business error に使わない。panic unwinding/catching を application API にしない。task boundary/supervisor が panic report を捕捉して sibling/parent policy を実行する。

## 15. Contract

### 15.1 Contract subset

contract expression は:

- pure
- terminating
- bounded
- deterministic
- allocation bounded by policy
- capability/resource accessなし
- panic-free

利用可能:

- boolean/arithmetic
- field access
- enum test
- collection length
- bounded quantifier
- pure helper marked/verified `contract fn`
- `old(value)` in postcondition
- result pattern

### 15.2 Runtime and proof

通常 build:

- trivially provable contract は compile-time discharge
- それ以外は boundary runtime check
- violation は `ContractViolation` panic report
- message は source span、contract ID、redacted values を持つ

`mendrel verify` profile:

- SMT/abstract interpretation backend を任意に実行
- timeout/unknown は normal compile failure と混同しない
- proof artifact と solver/version/options を保存
- proof がなくても runtime check を削除しないのが既定
- proof-assured deployment profile だけ、同じ proof artifact hash の条件下で check elision を許す

### 15.3 Constrained newtype

domain invariant は newtype へ置く。

```mendrel
pub newtype Percentage = Decimal128
invariant value >= 0.0 && value <= 100.0;
```

これにより repeated precondition を減らす。unsafe representation construction は defining module の audited path のみ。

## 16. Secret と sensitive data

```mendrel
let token: Secret<Text> = secrets.read("payment-token").await?;
```

`Secret<T>`:

- `Debug`/`Display`/generic serialization を実装しない
- string interpolation 不可
- equality は constant-time policy を選べる
- clone は default 禁止
- reveal は `RevealSecret` capability と lexical scope を要求
- telemetry field に渡すと compile error
- crash bundle は redaction
- memory zeroization は best effort と明記し、GC copy を完全保証しない
- 強い zeroization が必要な場合は pinned affine `SecretBuffer` resource を使う

`Sensitive<T>` は redaction policy を持つが、业务上 copy 可能な型。PII classification metadata を schema/artifact に含められる。

## 17. Dynamic boundary

`Dynamic` は parser/interop boundary だけ。

```mendrel
let raw: Dynamic = json.parse(bytes)?;
let request: CheckoutRequest = CheckoutRequest.decode(raw)?;
```

- `Dynamic` から field access するには validation API
- `Dynamic` を domain/public API 内へ伝播する lint
- runtime reflection で arbitrary method call はできない
- schema decode error は path と expected shape を持つ
- successful decode 後は static type に戻す

## 18. Wire type compatibility

type checker は ordinary source compatibility と wire compatibility を分ける。

### Source API

- parameter addition: breaking
- required record field addition: breaking
- optional builder field addition: condition付き
- enum variant addition: closed consumer に breaking
- effect/capability addition: authority/behavior breaking
- async 化: breaking
- error variant addition: exhaustive consumer に breaking

### Wire

- new optional field with fresh ID: backward-compatible
- ID/name reuse: forbidden
- field type wire-kind change: breaking
- enum numeric code reuse: forbidden
- unknown preservation removal: breaking
- reserved range/name removal: policy error

`mendrel api diff`、`effect diff`、`schema diff` は別 report を出し、publish policy がまとめて version bump を判定する。

## 19. Diagnostic requirements

type/effect error は最低限、次の structured field を返す。

```json
{
  "code": "E-EFFECT-0012",
  "summary": "required capability is not available",
  "expected": {
    "label": "payments",
    "type": "PaymentGateway"
  },
  "available": [
    {
      "label": "sandbox_payments",
      "type": "PaymentGateway"
    }
  ],
  "cause_graph": [
    {
      "kind": "call",
      "symbol": "shop.checkout.checkout"
    },
    {
      "kind": "requires_capability",
      "symbol": "shop.payment.authorize",
      "label": "payments"
    }
  ],
  "fixes": [
    {
      "kind": "explicit_capability_remap",
      "applicability": "machine-applicable"
    }
  ]
}
```

human rendering はこの data から作る。人間向け文章と agent 向け JSON が別ロジックで食い違わないようにする。

## 20. Type/effect system の非目標

v1 に入れない。

- full dependent type
- arbitrary refinement type with solver in normal compile
- higher-rank inference
- impredicative polymorphism
- HKT
- type-level general recursion
- overlapping type class
- specialization
- implicit conversion chain
- structural public API
- general union/intersection type
- nullability
- dynamic dispatch by method missing
- checked exception hierarchy
- effect handler continuation
- user-defined variance annotation
- public signature inference
