# 01. Language reference — surface language draft 0.1

## 1. 基本方針

Mendrel の surface syntax は、次を優先する。

1. 一つの token sequence に一つの主要 parse
2. incomplete source でも局所回復しやすい delimiter
3. declaration boundary の視認性
4. formatter による canonical form
5. text diff の安定性
6. language model が partial edit しても隣接構文を巻き込みにくいこと

syntax golf は目的にしない。意味上重要な区別は keyword へ出す。

## 2. Source file

- encoding は UTF-8
- line ending の canonical form は LF
- BOM は parser が受理しても formatter が除去する
- source span の正本は UTF-8 byte offset の half-open interval `[start, end)`
- tab は string literal 内を除き禁止
- indentation は意味を持たない
- file extension は `.mnd`
- 一ファイルは先頭に一つの `module` declaration を持つ
- file path と module name の対応は package manifest から決定される
- top-level executable statement は禁止

```mendrel
module shop.checkout;

import shop.domain.{Money, Order, OrderId};
import shop.ports.{Clock, OrdersRepo, PaymentGateway};

pub async fn checkout(...) -> Result<Order, CheckoutError>
uses { ... }
{
    // ...
}
```

## 3. Comment

```mendrel
// line comment

/* block comment
   may be nested */

/**
 * Documentation for the following declaration.
 *
 * Examples in documentation are compiled as tests.
 */
```

formatter は comment を CST trivia として保持し、対応 declaration との相対位置を保存する。

## 4. Identifier

### 4.1 Source identifier

- identifier は Unicode を許す
- normalization は NFC
- edition ごとに Unicode version を固定する
- UTS #39 に基づく confusable、mixed-script、invisible character check を行う
- keyword は identifier に使えない
- raw identifier escape は設けない
- package、module segment、wire field name、external symbol alias は ASCII lower snake case に限定する
- type/trait/capability は UpperCamelCase
- value/function/module/field/label は lower_snake_case
- constant は UPPER_SNAKE_CASE

非 ASCII identifier は local domain code では許すが、public package API は lint profile により ASCII を推奨できる。security-sensitive package では ASCII-only を強制できる。

### 4.2 Shadowing

重なった lexical scope で同じ value name を再宣言してはならない。

```mendrel
let user = load_user(id)?;
// let user = normalize(user); // E-NAME-SHADOW-0001
let normalized_user = normalize(user);
```

pattern 内の同名重複も禁止する。名前変更による accidental capture を減らすためや。

## 5. Keyword と punctuation

v0.1 の主要 keyword:

```text
module import pub internal
record enum wire field reserve newtype opaque type
trait impl capability resource object
fn async uses where contract requires ensures invariant
let var move use
if else match for while loop break continue return
scope spawn await within select
Ok Err Some None
true false
unsafe
test property
```

operator set は固定する。

```text
+ - * / %
== != < <= > >=
&& || !
& | ^ << >>
= += -= *= /= %=
? .
-> =>
```

- user-defined operator と precedence は禁止
- operator overloading は compiler-defined trait mapping に限定
- assignment は expression ではなく statement
- comparison chaining は禁止
- implicit semicolon insertion はない

## 6. Module と visibility

### 6.1 Module

```mendrel
module billing.invoice;
```

module graph は DAG とする。相互再帰が必要な declaration は同一 module 内へ置く。package 間 cycle は禁止。

### 6.2 Import

```mendrel
import billing.money.Money;
import billing.tax.{TaxRate, calculate_tax};
import billing.clock.Clock as BillingClock;
```

- wildcard import は release profile で禁止
- transitive dependency の symbol は import できない
- direct dependency だけを manifest に列挙する
- import は symbol を re-export しない
- re-export は明示 declaration でのみ行う
- unused import は error または deny lint

### 6.3 Visibility

- default は module-private
- `pub` は package 外公開
- `internal` は package 内公開
- nested field は type declaration 側で visibility を持つ
- public function が private type を露出してはならない
- public API へ anonymous structural type を露出してはならない

## 7. Declaration

### 7.1 Record

record は既定で immutable value。

```mendrel
pub record Customer {
    id: CustomerId,
    name: Text,
    email: Option<EmailAddress>,
}
```

construction:

```mendrel
let customer = Customer {
    id,
    name,
    email: None,
};
```

update copy:

```mendrel
let renamed = customer with {
    name: "Aki",
};
```

- field order は source/API rendering では保持する
- ordinary record の runtime equality/hash は明示 derive のみ
- record は class identity を持たない
- cyclic immutable graph は builder/arena API を通す

### 7.2 Enum / algebraic data type

```mendrel
pub enum PaymentState {
    Pending,
    Authorized {
        authorization_id: AuthorizationId,
    },
    Declined {
        reason: DeclineReason,
    },
}
```

match:

```mendrel
match state {
    PaymentState.Pending => "pending",
    PaymentState.Authorized { authorization_id } => authorization_id.text(),
    PaymentState.Declined { reason } => reason.message(),
}
```

- closed enum の match は exhaustive
- public closed enum に `_` catch-all を使うと error
- private enum では `_` を許せるが deny-by-default lint
- variant 追加は source compatibility 上 breaking と分類
- wire enum の unknown case は別規則を持つ

### 7.3 Newtype

```mendrel
pub newtype OrderId = Uuid;

pub newtype NonEmptyText = Text
invariant value.scalar_count() > 0;
```

- representation は nominal
- implicit conversion はない
- constructor は invariant を検査する
- validated constructor は `Result<NonEmptyText, ConstraintError>`
- compiler が proof できる literal は compile-time accept/reject
- representation access は defining module 内だけ

### 7.4 Opaque type

```mendrel
pub opaque type PasswordHash;
```

public consumer は representation を見られない。implementation module が constructor と operation を公開する。

### 7.5 Object

`object` は mutable identity を持ち、task-local が既定。

```mendrel
object BufferBuilder {
    var bytes: MutableBytes;

    fn push(self, byte: U8) {
        self.bytes.push(byte);
    }
}
```

- `object` は既定で `Send` でも `Share` でもない
- alias は同一 task 内だけ
- task boundary へ移すには isolated ownership を証明して `Owned<T>` にする
- public domain model には record/enum を優先する

### 7.6 Resource

```mendrel
resource File {
    fn read(self: &File, max: USize) -> Result<Bytes, IoError>;
    fn close(move self) -> Result<Unit, IoError>;
}
```

resource は複製不能で、所有値は一度だけ consume/drop される。通常は `use` で lexical cleanup を保証する。

```mendrel
use file = fs.open(path)?;
let data = file.read(4096)?;
```

### 7.7 Capability

```mendrel
pub capability Clock {
    fn now() -> Instant;
    async fn sleep(duration: Duration) -> Result<Unit, Cancelled>;
}
```

capability value は authority を表し、通常 constructor で forge できない。host/runtime、test harness、明示 composition root が提供する。

### 7.8 Trait と impl

```mendrel
pub trait Encode<Target> {
    type Error;

    fn encode(self, target: &mut Target) -> Result<Unit, Self.Error>;
}
```

```mendrel
impl Encode<JsonWriter> for Order {
    type Error = JsonError;

    fn encode(self, target: &mut JsonWriter) -> Result<Unit, JsonError> {
        // ...
        Ok(Unit)
    }
}
```

規則:

- class inheritance はない
- trait は nominal
- impl coherence は package graph 全体で一意
- orphan rule: trait または target type のどちらかを current package が所有
- overlapping impl、negative impl、specialization は v1 にない
- implicit conversion trait はない
- associated type はある
- higher-kinded type parameter は v1 にない
- trait object は明示 `dyn Trait` で、object-safe subset のみ
- dynamic dispatch は signature 上で分かる

### 7.9 Function

public/module-level function は signature を完全に書く。

```mendrel
pub async fn authorize(
    order_id: OrderId,
    amount: Money,
) -> Result<Authorization, PaymentError>
uses {
    payments: PaymentGateway,
    clock: Clock,
}
contract {
    requires amount >= Money.zero(amount.currency);
}
{
    // body
}
```

- parameter type、return type、generic parameter、effect/capability row、async は明示
- private local lambda と local binding は推論可能
- function body の最後の expression が return value
- early return は `return expression;`
- overloading は name と arity/type による一般解決を行わない
- 同一 module に同名 function は一つ
- default argument はない
- named argument は public constructor と function で使えるが declaration name と完全一致する
- variadic function はない。`List<T>` 等を使う

## 8. Binding と mutation

### 8.1 Immutable binding

```mendrel
let total = calculate_total(items);
```

`let` は再代入できない。referent が `object` なら object 自体の method による mutation は型規則に従う。

### 8.2 Mutable local binding

```mendrel
var retries: U32 = 0;
retries += 1;
```

- `var` は local scope のみ
- global mutable binding は禁止
- closure が `var` を capture すると、task-local mutable cell になる
- mutable capture を spawn するには isolation rule を満たす必要がある

### 8.3 Move

```mendrel
let task = tasks.spawn move request {
    process(request).await
};
```

`move` は affine/resource/owned value の ownership transfer を明示する。move 後の source binding は利用不能。

## 9. Expressions と statements

### 9.1 Block

block は expression。

```mendrel
let result = {
    let normalized = normalize(input);
    validate(normalized)?
};
```

空 block の値は `Unit`。

### 9.2 If

```mendrel
let fee = if customer.is_member {
    Money.zero(currency)
} else {
    standard_fee
};
```

condition は `Bool` のみ。truthiness はない。両 branch の型は一致または明示的共通型を持つ。

### 9.3 Match

```mendrel
let message = match result {
    Ok(order) => "created " + order.id.text(),
    Err(CheckoutError.PaymentDeclined { reason }) => reason.message(),
    Err(error) => error.public_message(),
};
```

- first-match semantics
- guard は pure expression のみ
- exhaustiveness と unreachable pattern を compile-time 検査
- integer/string の巨大 pattern set は decision tree へ lowering
- effectful pattern extractor はない

### 9.4 Loop

```mendrel
for item in items {
    consume(item)?;
}

while queue.has_items() {
    let item = queue.pop().expect("checked non-empty");
    consume(item)?;
}

loop {
    if done() {
        break;
    }
}
```

- C-style `for(init; cond; step)` はない
- iterator protocol は固定 trait
- `break value` は `loop` だけに許可できる
- loop invariant は contract profile で指定可能

### 9.5 Error propagation

```mendrel
let order = orders.load(order_id)?;
```

`?` は `Result<T,E>` のみ。caller error type への変換は明示 `map_error` または一意な `FromError` mapping に限定し、hidden chain を diagnostic で表示する。

exception/throw/catch はない。

### 9.6 Option

```mendrel
match customer.email {
    Some(email) => send(email),
    None => Ok(Unit),
}
```

null literal と nullable reference はない。

### 9.7 Pipeline は入れない

pipe operator、method cascade、implicit receiver、extension import による曖昧な fluent syntax は v1 へ入れない。普通の call と local binding で data flow を明示する。

## 10. Literals と primitive type

### 10.1 Primitive

```text
Bool
I8 I16 I32 I64 I128
U8 U16 U32 U64 U128 USize
F32 F64
Decimal128
Char
Text
Bytes
Unit
Never
```

- 整数 width は明示
- unsuffixed integer literal は期待型から決まり、範囲検査される
- 期待型がなければ小さな既定型を勝手に選ばず diagnostic を出す
- arithmetic overflow は全 build profile で checked
- wrapping/saturating/checked operation は名前付き method
- floating-point は IEEE semantics を明示し、NaN-sensitive equality/hash を derive 時に選ぶ
- monetary value に float を使う lint を標準提供する

### 10.2 Text と Bytes

- `Text` は immutable UTF-8
- integer index は禁止
- iteration unit を `scalar_values()`、`graphemes()`、`bytes()` で明示
- slice は boundary を検査し `Result` を返す
- `Bytes` は arbitrary byte sequence
- encoding/decoding は明示
- secret text は `Secret<Text>` で別扱い

### 10.3 Time

標準型を分ける。

```text
Instant
Duration
UtcDateTime
OffsetDateTime
LocalDate
LocalTime
TimeZoneId
```

wall-clock と monotonic time を混ぜない。deadline は `Instant`、business timestamp は UTC/offset type を使う。

## 11. Evaluation semantics

- argument は source order で left-to-right evaluation
- record field initializer も source order
- boolean `&&` と `||` は short-circuit
- integer overflow は checked
- division by zero は typed panic または checked method。通常 lint は checked method を推奨
- map/set iteration order は type で区別する
  - `HashMap` は順序を契約しない
  - `OrderedMap` は insertion/order semantics を持つ
- hash seed は runtime random でもよいが、test/repro profile では明示 seed
- compiler optimization は observable order を変えてはならない
- debug/release で assert、overflow、contract の意味を変えない
- `debug_assert` だけは明示的に release 省略可能

## 12. Async と task syntax

```mendrel
pub async fn fetch_both(
    left_url: Url,
    right_url: Url,
) -> Result<Pair<Response, Response>, FetchError>
uses {
    http: HttpClient,
}
{
    scope tasks {
        let left = tasks.spawn {
            http.get(left_url).await
        };

        let right = tasks.spawn {
            http.get(right_url).await
        };

        let left_response = left.await?;
        let right_response = right.await?;
        Ok(Pair(left_response, right_response))
    }
}
```

- task は scope から漏れない
- `await` は async function/scope 内のみ
- `spawn` capture は `Share` または explicit moved `Send`
- un-awaited task は scope exit policy に従い join/cancel される
- detached spawn はない

deadline:

```mendrel
let response = within 2.seconds {
    http.get(url).await
}?;
```

`within` は current deadline と min を取り、子 task/capability call へ伝播する。

## 13. Contract syntax

```mendrel
pub fn withdraw(
    balance: Money,
    amount: Money,
) -> Result<Money, WithdrawError>
contract {
    requires amount.currency == balance.currency;
    requires amount >= Money.zero(balance.currency);
    ensures result is Ok(new_balance) => new_balance <= balance;
}
{
    // ...
}
```

- contract expression は pure
- bounded、terminating、side-effect free subset
- compiler が証明できない precondition は public boundary で runtime check
- postcondition は build policy により always-on または verified-only ではなく、v1 production profile では always-on を既定
- heavy SMT proof は別 `verify` profile
- `assume` は safe production code にない
- loop invariant は verifier profile で使える

## 14. Wire declaration

```mendrel
pub wire record OrderCreated {
    field 1 order_id: OrderId,
    field 2 total: WireMoney,
    field 4 created_at: UtcDateTime,
    reserve 3,
    reserve "legacy_customer_id",
}

pub wire enum PaymentStatus {
    case 0 Unknown,
    case 1 Pending,
    case 2 Authorized,
    case 3 Declined,
    unknown Other {
        code: U32,
        payload: Bytes,
    },
}
```

規則:

- field/case ID は source order や name から生成しない
- 削除した ID/name は reserve
- ID 再利用は禁止
- unknown field は decode/encode round-trip で保存
- wire type と domain type を同一視しない
- compatibility は `mendrel schema diff`
- JSON mapping は明示 adapter
- default 値の意味は edition/schema version に固定
- package publish は wire breaking change を version rule へ反映する

## 15. Attribute と derive

v0.1 は compiler-defined attribute だけを許す。

```mendrel
@derive(Eq, Hash, Debug)
@deprecated(since: "1.4.0", replacement: "new_api")
@unsafe_reason("vendor C API")
@wire_adapter("protobuf")
```

- unknown attribute は error
- arbitrary token tree は attribute argument に使わない
- attribute value は literal、qualified name、閉じた constructor form、list に限定する
- attribute argument は通常 expression ではなく、compile-time data として型検査する
- derive expansion は compiler-owned typed lowering
- derive が生成した public API は API fingerprint に含める
- expansion は `mendrel explain derive` で閲覧できる

## 16. Unsafe

```mendrel
unsafe module native_tls
reason "Binds audited vendor C ABI"
{
    pub fn connect(...) -> Result<TlsStream, TlsError> {
        unsafe {
            ffi_tls_connect(...)
        }
    }
}
```

- safe module から raw pointer を作れない
- `unsafe` は safety contract を doc comment で持つ
- unsafe operation は compiler-known set
- unsafe function call は unsafe context を要求
- package manifest が unsafe usage を宣言
- artifact に unsafe surface summary を埋め込む
- transitive unsafe dependency を `mendrel unsafe tree` で表示

## 17. Test と property

通常 test は引数を持たず、`Result<Unit, TestError>` または `Unit` を返す。

```mendrel
test checkout_reuses_idempotent_result {
    // deterministic fixture-backed test
}
```

property test は generator が与える引数を受け、最後の式が `Bool` または `Result<Unit, PropertyError>` になる。

```mendrel
property test encode_decode_round_trip(order: Order) {
    let bytes = OrderWire.encode(order);
    OrderWire.decode(bytes) == Ok(order)
}
```

- generator、shrinker、試行数、seed は test artifact に記録する
- derive 不能な constraint/newtype/resource/secret は明示 generator を要求する
- generator 不足を silent skip しない
- async property は `uses`、deadline、scheduler policy を通常 function と同じように宣言する
- schedule exploration は `@test(schedule: explore(max_steps: 200, seeds: 1000))` のような閉じた attribute data で指定する

## 18. Main と service entry

CLI:

```mendrel
pub async fn main(args: List<Text>) -> Result<ExitCode, MainError>
uses {
    console: Console,
    clock: Clock,
}
{
    // ...
}
```

service:

```mendrel
pub service CheckoutService
uses {
    listener: HttpListener,
    supervisor: Supervisor,
    telemetry: Telemetry,
}
{
    async fn run() -> Result<Unit, ServiceError> {
        // readiness, drain, shutdown are runtime-managed
    }
}
```

`service` は magic global framework ではなく、runtime lifecycle contract へ lowering される閉じた declaration form。

## 19. Canonical formatter

formatter は設定を持たず、次を固定する。

- indentation 4 spaces
- trailing comma in multi-line list
- brace style
- one declaration per logical block
- import sort/group
- line break algorithm
- contract/uses の multiline shape
- numeric literal grouping
- doc comment normalization は content を変えない
- generated source marker と provenance header

format は冪等:

\[
format(format(source)) = format(source)
\]

かつ parse-preserving:

\[
parse(format(source)) \equiv parse(source)
\]

error-recovery node を含む source は、破壊的 format をせず `--partial` で安全範囲だけを整形する。

## 20. 構文上の非目標

- whitespace-sensitive block
- optional delimiter の増殖
- significant newline
- automatic semicolon insertion
- tuple field `.0`
- magic getter/setter
- implicit constructor
- implicit enum/string conversion
- keyword argument shorthand の多様な形
- decorator DSL
- embedded preprocessor
- arbitrary DSL syntax extension
- method missing
- runtime monkey patch
- eval
- source-level reflection

## 21. 最小例

```mendrel
module hello.main;

import std.console.Console;

pub fn greeting(name: Text) -> Text {
    "Hello, " + name
}

pub async fn main(args: List<Text>) -> Result<ExitCode, MainError>
uses {
    console: Console,
}
{
    let name = match args.get(0) {
        Some(value) => value,
        None => "world",
    };

    console.write_line(greeting(name)).await?;
    Ok(ExitCode.success)
}
```
