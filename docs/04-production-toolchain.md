# 04. Production toolchain and operations

## 1. 原則

Mendrel の production story は language の外付け appendix ではない。source、type、effect、wire schema、package graph、build input、runtime signal を同じ artifact graph へ結び付ける。

目標:

- dependency と build input が宣言される
- 同一 input から同一 artifact が得られる
- artifact の由来を検証できる
- 公開 API/effect/schema の変更を release 前に分類できる
- config/secret/time/network/database の誤用を型と lint で止める
- 障害時に source revision、artifact、task tree、trace、schema を対応付けられる
- rollback と forward migration の両方を設計できる

## 2. One official toolchain

公式 CLI は `mendrel` 一つ。

```text
mendrel new
mendrel fmt
mendrel check
mendrel lint
mendrel test
mendrel run
mendrel build
mendrel doc
mendrel api
mendrel effect
mendrel schema
mendrel package
mendrel publish
mendrel artifact
mendrel unsafe
mendrel explain
mendrel agent
mendrel xtask
```

compiler executable を分離する場合は内部/低レベルの `mendrelc`。通常利用者は build tool、package manager、formatter、linter を別々に選ばない。

toolchain version は manifest/lockfile または `mendrel-toolchain.toml` に pin する。package が nightly-only semantics に依存する仕組みは持たない。

## 3. Package manifest

file name:

```text
Mendrel.pkg
Mendrel.lock
```

manifest は宣言的データであり、任意コードを実行しない。

```mendrel
package {
    name: "shop_checkout",
    version: "1.4.0",
    edition: "2026",
    license: "Apache-2.0",
    source: "src",
}

targets {
    library "checkout_core" {
        module: "shop.checkout",
    }

    service "checkout_service" {
        module: "shop.main",
    }
}

dependencies {
    "std_http": {
        version: "2.1.3",
        checksum: "sha256:...",
    },
}

capabilities {
    production: [
        "network.client",
        "database.postgres",
        "secret.read:payment/*",
        "telemetry.emit",
    ],
}

unsafe {
    allowed_packages: [],
}

build {
    reproducible: true,
    warnings: "deny",
}
```

これは説明用 syntax。実 manifest grammar は source language と共有しても、実行可能 expression、function call、import、macro を持たない。

## 4. Dependency model

### 4.1 Direct dependency only

source は manifest に列挙された direct dependency の公開 module だけ import できる。transitive dependency への accidental coupling を禁止する。

### 4.2 Immutable versions

publish 済み package version は不変。置換や削除は security revocation metadata で表し、同じ version の content を差し替えない。

### 4.3 Content address

lock entry は最低限:

- registry identity
- package name/version
- content digest
- manifest digest
- source provenance
- signature/transparency log proof
- dependency graph digest
- license expression
- yanked/revoked state snapshot

build cache key は source だけでなく compiler、edition、target、declared env、generator、dependency、build options を含む。

### 4.4 No feature matrix

Cargo 型の additive feature flag の組み合わせ爆発を v1 に持ち込まない。

代替:

- 別 package
- 別 target
- runtime policy value
- capability implementation selection
- edition
- platform module selection

compile-time variant が本当に必要な場合、manifest に閉じた named profile として定義し、公開 API fingerprint を profile ごとに持つ。任意 dependency が feature を勝手に統合する仕組みはない。

## 5. Hermetic build

build action は宣言 input だけを読める sandbox で実行。

禁止される ambient input:

- host current directory outside workspace
- undeclared environment variable
- wall clock
- network
- user home
- global package cache content without digest
- locale/timezone
- random seed
- host tool discovered from `PATH`
- Git dirty state unless declared as input
- filesystem traversal outside input root

必要な input は manifest/tool invocation へ明示する。

```sh
mendrel build \
  --target x86_64-linux-gnu \
  --define build_version=1.4.0 \
  --source-date-epoch 1786982400
```

timestamp は artifact identity に不要なら埋め込まない。必要なら declared value を使う。

## 6. Reproducible artifact

同じ normalized input set から bit-for-bit 同一 artifact を目標にする。

normalized input set:

- source tree digest
- generated source digest
- lockfile
- compiler/runtime/toolchain digest
- target triple and target spec
- edition
- optimization profile
- declared build values
- linker/tool digest
- generator input/output
- build policy

`mendrel build --reproducible` は二回 isolated build または independent builder attestation の比較を行える。

差異が出た場合、section-level diff を返す。

- code section
- symbol/debug info
- archive order
- timestamp
- path
- randomized hash/seed
- generator output
- linker metadata

## 7. Artifact format

Mendrel Release Artifact（`.mra`）は、native binary/Wasm component と metadata bundle を含む署名可能 container とする。

metadata:

- artifact digest
- source revision/digest
- compiler/runtime digest
- target
- package graph
- API fingerprint
- effect/capability fingerprint
- wire schema fingerprint
- unsafe surface
- SBOM
- provenance
- license report
- vulnerability scan snapshot
- generator provenance
- conformance/test summary
- debug/source map reference
- runtime compatibility version

metadata は executable へ埋め込む最小 locator と外部 bundle の両方を持てる。

```sh
mendrel artifact inspect checkout_service.mra
mendrel artifact verify checkout_service.mra
mendrel artifact diff old.mra new.mra
```

## 8. Supply-chain security

release pipeline は次を生成/検査する。

- source and dependency checksums
- signed provenance
- SBOM in SPDX-compatible form
- license policy
- vulnerability advisory resolution
- registry transparency proof
- generator sandbox report
- reproducibility result
- binary/source correspondence
- unsafe dependency tree
- capability grant manifest

credential は build input に含めず、publish/sign action の isolated capability として渡す。

CI runner が持つ ambient cloud credential を build process から見えなくする。

## 9. Code generation

### 9.1 Arbitrary build script を禁止

package install/build 時に host 上の任意コードを実行しない。

### 9.2 Built-in derive

compiler-owned derive は type HIR から deterministic output を作る。

### 9.3 Sandboxed generator

外部 generator は WebAssembly component として動き、manifest に input/output/capability を宣言。

```mendrel
generator "openapi_client" {
    component: "registry/openapi-generator@2.3.1",
    input: "schema/payment.yaml",
    output: "generated/payment_client",
    capabilities: [],
}
```

規則:

- default no network/time/random
- declared immutable input only
- output directory only writeable
- generator/component digest pinned
- output source canonical format
- generated file に generator/input digest と source map
- dirty manual edit は `generated --check` で拒否
- generator output は review/grep 可能
- release artifact provenance に含める

## 10. Platform conditional

scattered `#if`/`cfg` を禁止。module/file selection を manifest で行う。

```mendrel
platform {
    when target.os == "linux" {
        module "std.platform.process" from "src/linux/process.mnd";
    }

    when target.os == "windows" {
        module "std.platform.process" from "src/windows/process.mnd";
    }
}
```

public signature は platform variants 間で一致を検査する。違う場合は target-specific package に分ける。

## 11. Edition and compatibility

edition は syntax/name-resolution/default semantics の migration boundary。

原則:

- 同一 compiler が複数 edition を parse/check できる
- package ごとに edition 固定
- dependency edition は独立
- edition migration tool が canonical patch を作る
- edition により debug/release semantics は変えない
- wire schema は edition 変更だけで勝手に変えない
- edition warning は stable diagnostic code
- publish artifact は edition metadata を持つ

## 12. Semantic versioning enforcement

publish 前に compiler が比較する。

```sh
mendrel api diff --against registry:shop_checkout@1.3.2
mendrel effect diff --against registry:shop_checkout@1.3.2
mendrel schema diff --against registry:shop_checkout@1.3.2
```

分類:

- patch-compatible
- source-additive
- behavior/authority-sensitive
- source-breaking
- wire-backward-compatible
- wire-forward-compatible
- wire-breaking
- security-sensitive
- unknown/manual-review

version rule の例:

- public type/function removal: major
- parameter/return/error/effect/async change: major
- additive public function: minor
- additive closed enum variant: major
- capability authority escalation: major or security review
- optional fresh wire field: minor
- wire ID reuse: publish rejected
- documentation/internal implementation only: patch

`@deprecated(since, replacement)` は machine-readable migration link を持つ。deprecated removal は declared policy window と major bump を要求。

## 13. Configuration

### 13.1 Typed config

config は startup 前に typed schema へ decode/validate。

```mendrel
pub record CheckoutConfig {
    listen: SocketAddress,
    database: DatabaseConfig,
    request_timeout: Duration,
    max_in_flight: U32,
}
```

source:

- file
- environment
- command line
- secret reference
- platform config service

merge precedence は runtime/library が固定し、各 field に provenance を保持する。

```text
request_timeout = 2s
origin = environment:CHECKOUT_REQUEST_TIMEOUT
```

config error は process が listener/readiness を開始する前に fail。unknown field は deny by default。secret value は error/log に出さない。

### 13.2 No direct environment access

application code が `std.env.get` を ambient に呼べない。`ConfigSource` capability または startup decoder だけが env を読む。

## 14. Secrets

secret resolution は `SecretStore` capability。

```mendrel
pub capability SecretStore {
    async fn read(name: SecretName) -> Result<Secret<Bytes>, SecretError>;
}
```

deployment manifest は exact path/pattern grant を持つ。secret access は audit event。secret を config file へ materialize しない profileを推奨。

rotation:

- versioned secret handle
- refresh policy
- expiry
- lease resource
- failure behavior
- rollback compatibility

## 15. Networking

### 15.1 Typed clients

outbound network は `HttpClient` 等の capability。raw socket は lower-level package/unsafe policy。

client request は:

- finite deadline
- cancellation
- bounded body/response
- redirect policy
- TLS policy
- DNS policy
- retry classification
- trace propagation
- structured error

を明示/継承する。

### 15.2 No implicit shell/network

URL string concatenation、shell curl、global proxy env の暗黙利用を避ける。URL は parsed nominal type。

### 15.3 Retry

retry は generic loop ではなく typed policy。

```mendrel
let policy = RetryPolicy {
    max_attempts: 3,
    backoff: Exponential {
        initial: 50.milliseconds,
        maximum: 500.milliseconds,
    },
    retry_on: PaymentRetryClass.temporary_only,
    idempotency: Idempotency.required(order.id.text()),
};
```

規則:

- 残り deadline 内
- idempotency classification
- max attempts/backoff cap
- cancellation respect
- attempt trace
- retry budget
- server hint (`Retry-After`) policy
- nested retry amplification lint

## 16. Database and transactions

### 16.1 Capability

```mendrel
pub capability OrdersRepo {
    async fn find(id: OrderId) -> Result<Option<Order>, RepoError>;
    async fn save(order: Order) -> Result<Unit, RepoError>;
}
```

domain code は raw database handle より port capability を優先。

### 16.2 Checked SQL

infrastructure module では compile-time checked SQL を使える。

```mendrel
let row = sql.query_one::<OrderRow>(
    schema: "schemas/orders.snapshot",
    """
    SELECT id, total_amount, currency
    FROM orders
    WHERE id = :id
    """,
    { id },
).await?;
```

- parser と type checker が parameter/result shape を検査
- schema snapshot digest が build input
- production database へ compile 時接続しない
- migration による compatibility window を検査
- dynamic SQL は `DynamicSql` marker と injection-safe builder を要求
- raw string concatenation は error

### 16.3 Migration

migration strategy は expand → backfill → contract を標準化。

tool は:

- current/target schema
- old/new artifact compatibility
- dual-read/write period
- rollback constraints
- lock/rewrite risk
- backfill resumability
- validation query
- irreversible marker

を report する。

application publish は「new binary が current+target schema のどちらで動くか」を metadata に持つ。

## 17. Serialization and wire

wire declaration から生成:

- schema IR
- Protobuf adapter
- JSON adapter
- WIT adapter
- OpenAPI adapter
- compatibility rules
- fuzz corpus
- redaction/classification metadata

canonical domain↔wire conversion を explicit function にする。database row、HTTP JSON、event schema を domain record と一体化しない。

unknown fields/cases の preservation を bridge service で守る。

## 18. Observability

標準 signal:

- trace
- metric
- structured log
- profile
- runtime event

API は OpenTelemetry-compatible semantic model を目標にするが、application source を特定 vendor SDK に結合しない。

```mendrel
pub capability Telemetry {
    fn counter(name: MetricName) -> Counter;
    fn span(name: SpanName, fields: FieldSet) -> SpanResource;
    fn event(name: EventName, fields: FieldSet);
}
```

### 18.1 Automatic context

task spawn、capability call、service boundary、wire request は trace context を伝播。context は ambient global ではなく runtime-owned task context で、source effect surface を汚さない observer effect として厳密に定義する。

observer effect の条件:

- business result を変えない
- failure を application failure に昇格させない
- bounded buffering
- secret/sensitive redaction
- sampling policy
- deterministic test で disable/record
- `mendrel explain observer` で injection point 表示

これは unrestricted hidden effect の例外ではなく、runtime semantics の閉じた一部。

### 18.2 Structured fields

log interpolation より typed field。

```mendrel
telemetry.event(
    "order.created",
    fields {
        order_id,
        amount: order.total.amount,
        currency: order.total.currency,
    },
);
```

field type に redaction rule。`Secret<T>` は compile error。`Sensitive<T>` は hash/drop/mask policy を要求。

### 18.3 Cardinality and volume

metric label high-cardinality lint。task/channel/log budget。drop/sampling は metrics 自身で観測。

## 19. Health, readiness, shutdown

service runtime:

- liveness は process/runtime health
- readiness は dependency and acceptance state
- startup probe
- drain state
- graceful shutdown deadline
- in-flight request count
- child task/resource count
- final termination cause

handler は shutdown signal を global channel で監視しない。runtime が request scope cancellation/deadline を伝える。

## 20. Deployment manifest

language package manifest と platform deployment config は分けるが、artifact metadata で照合する。

deployment grants:

- capability authorities
- network destinations
- filesystem paths
- secret prefixes
- database roles
- CPU/memory/task limits
- listener ports
- telemetry exporter
- supervisor policy

`mendrel deploy check artifact.mra deployment.yaml` は、artifact が要求する capability と deployment grant の差を出す。

- missing grant
- unused excessive grant
- authority escalation
- schema/runtime incompatibility
- resource limit mismatch

## 21. Performance profiles

official build profiles:

- `dev`: incremental、full checks、fast codegen
- `test`: deterministic hooks、coverage optional
- `release`: optimized、all safety semantics
- `release-latency`: tail latency/GC policy
- `release-size`: code size
- `sanitized`: native FFI sanitizers
- `verify`: proof/extra static analysis
- `repro`: independent rebuild validation

profile は language semantics を変えない。

## 22. PGO and optimization

PGO input は provenance を持つ build input。

- profile source artifact fingerprint
- workload description
- profile digest
- privacy/redaction
- stale profile warning
- deterministic merge

optimizer report:

```sh
mendrel explain inline shop.checkout.checkout
mendrel explain allocation shop.checkout.checkout
mendrel explain dispatch shop.checkout.checkout
mendrel explain gc-roots shop.checkout.checkout
```

性能が必要な箇所を unsafe へ逃がす前に、representation/allocation report を使う。

## 23. Release gate

標準 release gate:

```text
format
generated-source consistency
lint
type/effect/resource/task check
unit/integration/property tests
fuzz smoke
deterministic concurrency
API diff
effect diff
wire schema diff
unsafe audit
dependency/license/vulnerability audit
SBOM
provenance
reproducible build
artifact signature
deployment capability check
```

gate result は machine-readable attestation。manual waiver は署名者、理由、有効期限、対象 diagnostic を持つ。

## 24. Rollback

rollback safety は binary だけで決まらない。

artifact metadata は:

- readable/writable schema range
- event version range
- required capability version
- config schema range
- minimum runtime
- side-effect migration marker
- irreversible operation marker

を持つ。

deploy tool は N と N-1 の同時稼働、message compatibility、database compatibility を検査する。

## 25. Production non-goals v1

- arbitrary package build script
- mutable published package
- transitive dependency import
- combinatorial feature unification
- source checkout 時の network codegen
- stable native ABI promise
- global runtime singleton dependency
- untyped config map throughout application
- direct ambient env access
- unbounded retry/channel/body
- secret in string/log
- deployment permissions outside artifact comparison
- debug/release safety divergence
