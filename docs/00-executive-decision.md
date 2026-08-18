# 00. Executive decision

## 1. 問題設定

「LLM にとって理想的な言語」を、単に学習データが多い言語、短く書ける言語、文法が単純な言語として定義すると、プロダクション保守では外す。

現実の maintenance task では、コード生成そのものより次が難しい。

- repository のどこを読めばよいか決める
- 宣言されていない依存や暗黙の制御フローを見抜く
- 変更の影響範囲を見積もる
- API、schema、async、resource、security の不変条件を守る
- compiler/test/runtime feedback から原因を絞る
- stale context のまま他人の変更を上書きしない
- 本番の障害を再現する
- migration を途中状態込みで安全に進める

したがって本設計の目的関数は、token prediction のしやすさではなく次とする。

\[
\text{Maintenance Cost}
=
\text{Required Context}
+
\text{Semantic Ambiguity}
+
\text{Change Blast Radius}
+
\text{Feedback Iterations}
+
\text{Operational Surprise}
\]

Mendrel は各項を言語・compiler・toolchain・runtime の共通情報で下げる。

## 2. 成功条件

### 2.1 言語レベル

- safe code は未定義動作を持たない
- null、暗黙変換、truthiness、unchecked overflow、非局所 implicit lookup を持たない
- 公開関数の signature から input、output、error、async、外部権限が分かる
- resource lifecycle と task lifetime が静的に検査できる
- evaluation order と overflow semantics が build profile に依存しない
- syntax と formatting が canonical である

### 2.2 repository maintenance

- compiler が symbol、type、effect、caller、test、wire compatibility の graph を問い合わせ可能にする
- typed hole と diagnostic が修正候補を structured data で返す
- semantic edit が revision-aware transaction になる
- API/effect/schema change の blast radius を patch 前に preview できる
- generated code の入力、generator、output、source map が追跡できる

### 2.3 production

- build が hermetic、reproducible、content-addressed である
- artifact に SBOM、provenance、compiler/runtime/schema/API fingerprint が付く
- config、secret、deadline、retry、observability が typed interface を持つ
- debug/release の観測可能な意味が一致する
- cancellation、panic、shutdown で resource が漏れない
- plugin boundary は stable native ABI ではなく typed component interface を優先する

## 3. 比較した設計系

| 案 | 強み | 本質的な弱み | 結論 |
|---|---|---|---|
| Rust 型の全面所有権 | GC 不要、resource と alias を精密に制御、低レベル適性 | 通常の業務コードでも lifetime/borrow obligation が広がり、局所修正が signature cascade を起こしやすい | 全面採用しない。`resource` と `Owned<T>` に限定して借りる |
| Go 型の小さい言語＋GC | 学習・実装・配備が簡単、toolchain が一つ | error/effect/authority が型に乗りにくく、goroutine の寿命と API compatibility が外部規約へ逃げる | 簡潔さと single toolchain を借りるが、意味情報を増やす |
| ML/Koka 型の推論＋effect | ADT、pattern、local inference、effect visibility | 一般 effect handler は continuation semantics と runtime 実装を難しくし得る | type/effect row を採用。一般 handler は v1 から外す |
| Pony 型の actor/reference capability | data-race safety と mutable sharing の規則が強い | 全コードを actor/reference-capability mental model に寄せると業務ロジックが重い | `Send`/`Share`/isolation と actor stdlib を採用 |
| Erlang/BEAM 型の actor-only | fault isolation、supervision、hot operation | request 内の細粒度 fan-out や通常の関数合成には冗長 | long-lived state と supervision に限定 |
| TypeScript/Python 型の gradual/dynamic | LLM の既存学習量、試作速度、interop | runtime failure、ambient behavior、弱い schema/API compatibility | boundary scripting には有用だが core language として不採用 |
| AST/graph を正本にする agent-native 言語 | rename、merge、semantic query が容易 | text diff、コメント、Git、既存 editor、人間の自由な読解を損ねる | text を正本にし、compiler semantic protocol を外付け |
| full dependent/refinement＋SMT | 強い correctness proof | proof burden、solver 非決定性、build latency、理解範囲が増える | pure bounded contract と nominal constraint を常用。SMT は任意 profile |
| C/C++ 互換中心 | ecosystem と低レベル制御 | UB、ABI、macro、build variance を引き継ぐ | FFI 境界に隔離。言語核は互換性を負わない |

## 4. 推奨アーキテクチャ

最終案は四層ある。

### A. Small semantic core

- strict static type
- nominal ADT
- local bidirectional inference
- `Result<T, E>`
- named capability/effect row
- immutable value default
- affine resource
- structured task scope
- explicit `unsafe`

### B. Canonical language surface

- braces と明示 delimiter
- statement semicolon
- fixed operator set
- no preprocessor
- no arbitrary macro
- no wildcard import
- no shadowing
- one formatter
- public signature explicit

### C. Semantic tool substrate

- lossless CST
- typed HIR
- query-based incremental compiler
- stable diagnostic schema
- typed hole
- MAP semantic query/edit transaction
- impacted test/API/effect/schema analysis

### D. Production contract

- declarative manifest
- hermetic/reproducible build
- signed content-addressed dependency
- SBOM/provenance
- wire schema with explicit IDs
- typed config/secret/time/deadline/retry
- OpenTelemetry-compatible signals
- deterministic test scheduler
- artifact verification

四層は別製品に分けない。compiler が持つ symbol/type/effect/resource/schema graph を、IDE、agent、CI、publish、runtime metadata が共有する。

## 5. なぜ管理メモリか

全面的な ownership は、低レベル安全性に対して強力や。ただし Mendrel の主戦場は service、CLI、batch、業務ロジックであり、主要な保守コストは allocator の除去ではなく、依存と変更影響の追跡にある。

そこで通常の値は tracing GC に置き、次だけを静的 lifecycle 管理する。

- OS handle
- socket/stream
- database transaction
- lock guard
- temporary credential
- subprocess
- foreign pinned memory
- user-defined resource

これにより、通常データの aliasing proof を減らしつつ、解放時刻が意味を持つものは決定的に閉じられる。

この判断は「GC は常に速い」という仮定には依存しない。hard real-time を v1 非目標にし、collector を runtime backend として交換可能にする。pause/heap telemetry と service SLO を言語 toolchain へ統合する。

## 6. なぜ capability と effect row か

Dependency injection container や global context は、依存を runtime graph に隠す。単純な effect label だけでは、「どの database」「どの secret store」かが分からない。

Mendrel は名前付き capability を使う。

```mendrel
pub async fn load_order(id: OrderId) -> Result<Order, LoadError>
uses {
    orders: OrdersRepo,
    clock: Clock,
}
{
    // ...
}
```

ここから compiler は同時に次を得る。

- この関数が純粋でないこと
- 外部依存の種類と logical role
- test fake の必要 surface
- call chain の effect propagation
- production deployment に必要な authority
- change impact
- observability span の boundary
- security review の対象

effect row は「任意の hidden behavior を handler で差し替える機構」より狭く使う。v1 では continuation を再開する一般 handler を持たず、ordinary value と wrapper implementation で差し替える。

## 7. なぜ text source＋MAP か

LLM が AST を出力すれば syntax error は減る。しかし source の正本を AST にすると、次の損失が大きい。

- コメントと局所的な説明の位置
- 人間に馴染んだ Git diff
- conflict marker と通常の merge workflow
- editor/grep/shell ecosystem
- 実装独立な長期保存性

したがって text を正本に保つ。ただし LLM に raw text しか与えないのも弱い。

MAP は、compiler が source revision 上で意味操作を計画し、canonical text patch と影響情報を返す折衷や。semantic ID は snapshot-local と明示し、hidden sidecar ID を source identity にしない。rename ancestry、public API fingerprint、wire field ID のように長期安定性が本当に必要なものだけを明示 metadata にする。

## 8. 意図的に減らしたもの

強い言語を作るため、機能数を減らす。

- class inheritance
- overloading based on implicit conversion
- user-defined operator/precedence
- general-purpose syntax macro
- arbitrary compile-time execution
- general algebraic effect handler
- ambient dependency lookup
- wildcard import
- top-level executable statement
- global mutable variable
- exception と `Result` の二重 error model
- configurable formatter
- build-time feature matrix
- stable native plugin ABI
- release-only unchecked semantics
- solver-dependent normal build
- transparent dynamic reflection

必要な表現力は library、nominal ADT、trait、capability、Wasm generator/component へ寄せる。

## 9. 最も大きい反論への回答

### 「既存 LLM は Python/TypeScript/Rust を大量学習している。新言語は不利では」

初期の zero-shot token familiarity では不利になる。これは消せない。Mendrel はそこを否定せず、次で逆転可能かを benchmark する。

- grammar と formatter の canonicality
- compiler-selected context
- typed hole
- stable structured diagnostics
- semantic edit transaction
- smaller legal state space
- deterministic tests
- explicit effects and authority

比較は toy generation ではなく、隠した symbol 名を含む seeded repository maintenance task で行う。勝てなければ「LLM 理想言語」という主張を撤回または修正する。

### 「明示的 signature と uses は冗長では」

冗長や。ただし公開 boundary の冗長さは、caller、reviewer、agent、deployment tool が共有する索引になる。body 内の local declaration は推論する。

compiler は private body から候補 signature を生成できる。

```sh
mendrel fix --materialize-signatures src/
```

人が契約を確認して source に固定する。推論を賢くして boundary を不可視にするより、保守では強い。

### 「capability row が巨大な AppContext になるのでは」

その危険は大きい。以下を仕様で止める。

- label と type の exact match
- package-wide `AppContext` capability を lint deny
- capability 一個あたりの method/authority budget
- public function の capability count budget
- domain capability と infrastructure capability の分離
- `mendrel effect explain/trace` による由来表示
- effect surface growth を SemVer/publish check の対象にする

### 「GC pause が本番で問題になるのでは」

問題になり得る。Mendrel は pause を隠さない。

- allocation、heap、pause telemetry を runtime 標準にする
- latency-sensitive service profile を持つ
- `Owned<Bytes>`、arena、pool を安全な library abstraction で使えるようにする
- collector backend を交換可能にする
- hard real-time は非目標と明記する

### 「一般 macro がなければ ecosystem が伸びないのでは」

token macro の代わりに、必要性が高い閉じた mechanism を用意する。

- built-in derive
- wire/schema declaration
- compile-time checked SQL
- sandboxed Wasm generator
- explicit generated source
- source map と provenance

macro hygiene と expansion semantics を agent/tooling が理解する負担を、かなり減らせる。

## 10. 最終判断

Mendrel の核は、新奇な一つの型理論ではない。既存の強い考えを、**変更可能性**を中心に再配置することや。

最重要な設計判断は次の順になる。

1. 外部権限と副作用を署名へ出す
2. task/resource lifetime を lexical に閉じる
3. text と semantic graph を両立させる
4. compiler feedback を人間向け文字列ではなく versioned data contract にする
5. language build artifact と production artifact の意味を連結する
6. 機能追加より legal state space の縮小を優先する

「LLM が書くための言語」ではなく、**LLM が誤ったときに早く、局所的に、機械的に戻れる言語**を作る。この定義なら、人間にとっても強い。
