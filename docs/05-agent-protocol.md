# 05. Mendrel Agent Protocol（MAP）and machine feedback

## 1. なぜ LSP だけでは足りないか

LSP は editor との補完、definition、reference、rename、diagnostic に適している。一方、repository-scale coding agent には追加要件がある。

- snapshot が変わった状態で stale edit を適用しない
- token budget に合わせて意味的に必要な context を選ぶ
- 変更前に API/effect/wire/test blast radius を予測する
- 複数ファイルの semantic change を transaction にする
- diagnostic の cause graph と修正候補を機械的に扱う
- compiler/test/build を sandbox policy 付きで実行する
- patch proposal と commit を分離する
- agent action の provenance を残す

Mendrel は editor interoperability に LSP を実装し、agent-oriented operation に **Mendrel Agent Protocol（MAP）** を追加する。

## 2. 原則

1. source text が正本
2. semantic graph は compiler snapshot の派生物
3. request は protocol version と workspace revision を必須
4. semantic ID は snapshot-local
5. public/wire identity だけ明示的長期 ID/fingerprint を持つ
6. edit は plan → preview → commit
7. commit は compare-and-swap
8. operation は deterministic ordering
9. response は token/size budget と truncation reason を持つ
10. human diff と structured impact の両方を返す
11. compiler diagnostic schema と MAP schema は versioned
12. agent に unrestricted shell/network authority を与えない

## 3. Transport

- JSON-RPC 2.0 compatible
- stdio、local socket、authenticated remote channel
- message framing は transport profile で規定
- protocol version: semantic major/minor
- server capability negotiation
- request/response ID
- cancellation
- streaming partial result
- deterministic pagination cursor
- optional gzip/zstd framing
- remote mode は mutual authentication と workspace ACL

LSP と同じ process `mendreld` が endpoint を分けて提供してよい。

## 4. Workspace snapshot

### 4.1 Revision

revision は content-addressed snapshot。

```json
{
  "workspace_revision": "sha256:7f...",
  "source_tree": "sha256:18...",
  "lockfile": "sha256:a3...",
  "toolchain": "sha256:91...",
  "target_profile": "dev"
}
```

uncommitted editor buffer を含む overlay snapshot も content hash を持つ。

### 4.2 Snapshot-local semantic ID

```text
sym:shop.checkout.checkout@rev:7f...
type:shop.domain.Order@rev:7f...
expr:file:src/checkout.mnd:byte:812-933@rev:7f...
```

ID は revision と組にしない限り無効。source move/rename を跨いで hidden ID を保つ保証はしない。

長期追跡:

- public symbol: qualified name + API fingerprint + rename ancestry metadata
- wire field/case: explicit numeric ID
- diagnostic: stable code
- artifact: content digest
- migration: explicit migration ID

## 5. Core methods

### 5.1 Workspace

```text
workspace.open
workspace.snapshot
workspace.status
workspace.diff
workspace.refresh
workspace.close
```

`workspace.snapshot` は source/lock/toolchain/overlay hash と diagnostics summary を返す。

### 5.2 Syntax

```text
syntax.parse-fragment
syntax.expected
syntax.format-fragment
syntax.node-at
```

`syntax.parse-fragment` は expected nonterminal（例: `expression`、`type`、`top_level_decl`）と source fragment を受け、lossless CST、recovery node、structured diagnostic を返す。

`syntax.expected` は revision、file、byte offset に対し、次を返す。

- legal token/nonterminal classes
- enclosing CST path
- missing delimiter/recovery state
- safe insertion/replacement anchors
- canonical fragment examples generated from the grammar inventory

`syntax.format-fragment` は category と surrounding indentation/context を受け、完全な file を書き換えず canonical fragment を返す。fragment API は grammar-constrained decoding を必須にはしないが、agent host が制約生成に使える deterministic data を提供する。

これらは parser と formatter の同じ table/query を使う。別実装の completion grammar を持たない。

### 5.3 Symbol

```text
symbol.search
symbol.get
symbol.references
symbol.callers
symbol.callees
symbol.implementations
symbol.overrides
symbol.usage_examples
symbol.related_tests
symbol.normalized-ir-digest
```

search filter:

- kind
- visibility
- package/module
- type shape
- effect/capability
- annotation
- text query
- definition/reference
- generated/source
- unsafe

response は stable sort key を持つ。

`symbol.normalized-ir-digest` は、trivia、source location、local compiler ID を除いた versioned typed HIR/MIR projection の digest を返す。これは **意味同値性の証明ではない**。format/comment-only change の除外、incremental cache、review diff の補助に使い、等しい digest から一般的な contextual equivalence を主張してはならない。projection schema と compiler edition を必ず digest domain に含める。

### 5.4 Type/effect/resource

```text
type.of
type.explain
type.members
type.instances
effect.of
effect.explain
effect.trace
effect.diff
resource.state
resource.flow
concurrency.scope
concurrency.capture
```

`effect.trace` は capability requirement の由来を call chain で返す。

`resource.flow` は construct/move/borrow/consume/cleanup path を返す。

### 5.5 Context

```text
context.bundle
context.expand
context.explain-selection
```

bundle request:

```json
{
  "goal": "Fix duplicate payment authorization on retry",
  "focus": [
    "sym:shop.checkout.checkout@rev:7f..."
  ],
  "budget": {
    "max_tokens": 24000,
    "max_files": 18,
    "max_symbols": 120
  },
  "include": [
    "definitions",
    "callers",
    "effects",
    "errors",
    "contracts",
    "related_tests",
    "recent_diagnostics"
  ],
  "exclude": [
    "generated_bodies"
  ]
}
```

selection algorithm は deterministic score を使う。

候補 signal:

- direct definition/reference
- type/effect dependency
- failing diagnostic cause graph
- caller/callee distance
- changed file proximity
- related test coverage
- API/schema boundary
- unsafe/resource/task boundary
- documentation link
- historical failure relevance if repository provides it

response は各 item の selection reason、omission summary、continuation cursor を返す。長い file を raw cut せず declaration/CST boundary で分割する。

### 5.6 Check/test/build

```text
check.run
lint.run
test.discover
test.impacted
test.run
test.replay
build.plan
build.run
artifact.inspect
```

request は sandbox policy を持つ。

```json
{
  "command": "test.run",
  "revision": "sha256:7f...",
  "selection": {
    "mode": "impacted"
  },
  "sandbox": {
    "network": "deny",
    "filesystem": "workspace-read-target-write",
    "time": "virtual",
    "secrets": []
  }
}
```

agent が arbitrary shell string を送る method は標準 MAP にない。必要なら host policy が `tool.exec` extension を別 authority で提供する。

### 5.7 Edit transaction

```text
change.plan
change.preview
change.commit
change.abort
```

semantic operations:

- rename symbol
- change function signature
- add/remove/reorder parameter
- add record field
- add enum variant
- add wire field/case with allocated ID
- implement trait
- extract function
- inline function
- move declaration/module
- replace capability
- remap capability label
- make function async
- materialize inferred local signature
- fill typed hole
- apply diagnostic fix
- migrate edition
- update deprecated call
- regenerate source
- reserve removed wire ID/name

### 5.8 Compatibility

```text
api.snapshot
api.diff
effect.snapshot
effect.diff
schema.snapshot
schema.diff
unsafe.snapshot
unsafe.diff
deployment.check
```

## 6. Transaction flow

### 6.1 Plan

request:

```json
{
  "method": "change.plan",
  "params": {
    "workspace_revision": "sha256:7f...",
    "operation": {
      "kind": "change_signature",
      "symbol": "sym:shop.checkout.checkout@rev:7f...",
      "new_signature": {
        "parameters": [
          {
            "name": "request",
            "type": "shop.checkout.CheckoutRequest"
          },
          {
            "name": "idempotency_key",
            "type": "shop.payment.IdempotencyKey"
          }
        ]
      }
    }
  }
}
```

response:

- transaction ID
- resolved target
- planned semantic steps
- ambiguous decision points
- affected declarations
- projected compatibility class
- required user choices
- estimated file/symbol count
- no source mutation

ambiguity がある場合、agent は option ID を選ぶ。server が勝手に heuristic commit しない。

### 6.2 Preview

preview は in-memory overlay に operation を適用し、次を返す。

- base revision
- preview revision
- canonical unified diff
- structured edits
- diagnostics before/after
- affected callers
- affected tests
- API/effect/schema/unsafe diff
- generated files
- formatter changes
- resource/task impact
- migration suggestions
- verification plan
- confidence limits

### 6.3 Commit

commit request は base revision と preview digest を含む。

```json
{
  "transaction_id": "chg_01J...",
  "base_revision": "sha256:7f...",
  "preview_digest": "sha256:aa..."
}
```

current workspace revision が違えば `E-MAP-STALE-0001`。server は自動 rebase せず、新 revision で re-plan を促す。

commit 後:

- new revision
- applied diff
- diagnostics
- audit record
- optional Git working tree changes
- no automatic commit unless host extension explicitly requested

## 7. Typed holes

development/check mode:

```mendrel
let authorization: Authorization = ?authorize_payment;
```

release build は hole を拒否。

hole response:

```json
{
  "hole": "?authorize_payment",
  "expected_type": "shop.payment.Authorization",
  "expected_effects": {
    "required": [
      {
        "label": "payments",
        "type": "shop.payment.PaymentGateway"
      }
    ],
    "available": [
      {
        "label": "payments",
        "type": "shop.payment.PaymentGateway"
      },
      {
        "label": "clock",
        "type": "std.time.Clock"
      }
    ]
  },
  "resource_state": [],
  "in_scope": [
    {
      "expression": "request.authorization",
      "type": "Option<Authorization>",
      "cost": 2
    },
    {
      "expression": "payments.authorize(order.total).await?",
      "type": "Authorization",
      "cost": 7,
      "requires_async": true
    }
  ],
  "candidate_edits": [
    {
      "kind": "fill_hole",
      "expression": "payments.authorize(order.total).await?",
      "applicability": "requires-review"
    }
  ]
}
```

candidate ranking は type/effect/resource legality を優先し、identifier text similarity だけにしない。

typed hole は unfinished proof/program を repository に残す仕組みではない。CI release profile は hole count 0 を要求する。

## 8. Diagnostic protocol

### 8.1 Stable code

code namespace:

```text
E-SYNTAX-xxxx
E-NAME-xxxx
E-TYPE-xxxx
E-EFFECT-xxxx
E-RESOURCE-xxxx
E-CONCURRENCY-xxxx
E-CONTRACT-xxxx
E-WIRE-xxxx
E-PACKAGE-xxxx
E-BUILD-xxxx
E-SECURITY-xxxx
E-MAP-xxxx
W-...
I-...
ICE-...
```

code の意味は protocol major 内で変えない。retire しても再利用しない。

### 8.2 Fields

minimum:

- schema version
- diagnostic ID for this run
- stable code
- severity
- summary
- primary span
- related spans
- symbol IDs
- expected/actual structured values
- cause graph
- notes
- fixes
- documentation URI/ID
- suppression policy
- origin query/compiler phase
- workspace revision

### 8.3 Cause graph

flat child message だけでなく DAG を許す。

node kind:

- source expression
- declaration
- call
- type constraint
- effect requirement
- capability availability
- resource transition
- task capture
- contract
- wire rule
- package dependency
- build input
- security policy

edge kind:

- requires
- inferred_from
- conflicts_with
- introduced_by
- captured_at
- crosses_boundary
- converted_by
- depends_on
- fixed_by

renderer は人間向けに最短説明 path を選ぶ。agent は DAG 全体を取得できる。

### 8.4 Fix applicability

- `machine-applicable`: semantics-preserving and unambiguous
- `machine-applicable-with-format`: canonical formatter changes included
- `requires-review`: compiles but behavior/authority/API may change
- `has-choice`: option selection required
- `informational`: patch template only
- `unsafe`: requires explicit unsafe approval

fix は base revision と expected source digest を持つ。

### 8.5 Forward compatibility

JSON consumer は unknown field を無視し、unknown enum value を保持/report できる。required field 変更は protocol major。new optional field は minor。

## 9. LSP integration

LSP が担当:

- text document sync
- completion
- hover
- definition
- references
- rename
- code action
- semantic token
- inlay hint
- formatting
- diagnostic publish
- workspace symbol

MAP が担当:

- revision transaction
- task-specific context bundle
- multi-file semantic plan/preview/commit
- type/effect/resource cause graph
- impacted test
- API/effect/schema diff
- artifact/build sandbox
- agent audit/provenance

同じ compiler query database を使い、結果ロジックを複製しない。

## 10. Semantic context rendering

agent に渡す code bundle は、単なる concatenated files ではない。

Mendrel Context Document（MCD）:

```text
[workspace]
revision = sha256:...
edition = 2026
target = ...

[goal]
Fix duplicate payment authorization on retry.

[symbol]
id = ...
signature = ...
effects = ...
contracts = ...
source = ...
selection_reason = direct_focus

[caller]
...

[test]
...

[omitted]
42 lower-ranked symbols; continuation = ...
```

JSON と compact text rendering の両方を持つ。token estimate は tokenizer profile を宣言し、完全一致を保証しない場合は byte/character budget fallback を持つ。

## 11. Repository map

`workspace.map` は次をまとめる。

- package/module DAG
- public API
- capability/effect graph
- service entry
- wire schema
- database migration
- unsafe boundary
- generated source
- tests/coverage
- owners/review policy if provided
- artifact/deployment relation

agent が最初に repository 全 file を列挙して迷うのを防ぐ。map は compiler-derived facts と repository-provided metadata を区別する。

## 12. Agent action audit

audit record:

- actor identity/model/tool version
- user instruction digest
- base revision
- context bundle digest
- methods called
- proposed/applied transaction
- diagnostics/tests
- sandbox grants
- artifact outputs
- human approvals
- timestamp from trusted host, not build semantics

audit は privacy policy に従い prompt/body を丸ごと必須保存しない。digest、redacted summary、selected source ID で再現可能性との折衷を取る。

## 13. Security

### 13.1 Prompt injection in repository

source comment、README、fixture、generated code 内の自然言語は data であり、agent authority instruction ではない。

MAP response は content origin を分類。

```json
{
  "content": "...",
  "origin": "source_comment",
  "trust": "untrusted_repository_text"
}
```

host agent policy は repository text が tool grant、secret access、publish action を変更できないようにする。

### 13.2 Least authority

MAP server の method scope:

- read semantic graph
- create overlay
- write workspace
- run check/test
- run build
- network
- read secret
- publish
- deploy

を別 capability にする。通常 coding session に publish/deploy/secret は与えない。

### 13.3 Exfiltration

context bundle と diagnostic は secret redaction/type metadata を尊重。generated crash bundle に source 全文を勝手に含めない。remote MAP は data residency policy を持つ。

## 14. Determinism

同じ:

- source revision
- compiler/toolchain
- target profile
- method parameter
- protocol version

に対し、semantic query response の facts/order/digest は同一。latency、server instance ID 等の運用 field は digest 外。

search ranking に learned model を使う場合、deterministic core result と optional heuristic rank を分離し、model/version/score を表示する。

## 15. Extensibility

extension method namespace:

```text
x.<vendor>.<method>
```

extension は core semantic ID、revision、security model を守る。unknown extension を core server が silently interpret しない。

core protocol に追加する機能は、複数 agent/editor/backend で必要性が実証されてから。

## 16. MAP success metrics

- stale edit rejection rate
- semantic edit apply success
- context tokens per solved task
- missing-symbol hallucination
- compiler repair iterations
- impacted-test recall/precision
- API/schema regression caught pre-commit
- human review time
- rollback/replay success
- protocol compatibility
- security grant violations blocked

## 17. MAP non-goals v1

- autonomous product requirement interpretation
- unrestricted shell
- arbitrary cloud/deploy control
- hidden persistent AST as source
- cross-repository global symbol identity
- learned ranking as source of truth
- automatic merge conflict resolution without preview
- automatic publish
- secret retrieval for ordinary code repair
- human review replacement
