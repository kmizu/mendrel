# Mendrel Design Pack

> **Mendrel（メンドレル）** — *Make effects visible. Make change local. Make repair mechanical.*

- Status: language and toolchain design draft 0.2
- Date: 2026-08-18
- Primary audience: compiler implementers, production platform engineers, coding agents, reviewers
- Canonical source form: UTF-8 text files with extension `.mnd`
- Reference compiler implementation language: Rust
- Initial code-generation targets: LLVM native and WebAssembly Component Model
- This pack is a design and implementation contract, not a claim that the language is already implemented.

## 1. 名前

**Mendrel** は、このプロジェクトで作った造語や。

- **mend**: 壊れたものを直す、保守する
- **rel**: relation / relevance / locality を連想させる短い語尾
- 設計上の意味: 「依存関係を見えるようにし、修正範囲を局所化し、修復を機械的に検証できる言語」

読みは「メンドレル」。CLI は `mendrel`、ソース拡張子は `.mnd` とする。

2026-08-18 時点で一般的な検索による予備的な衝突確認はしたが、これは商標・法人名・パッケージレジストリ・ドメインの法的クリアランスではない。公開名称として確定する前に、対象地域を定めた正式な調査を行うこと。

## 2. 一文でいうと

Mendrel は、**管理メモリ、値指向データ、アフィンな資源、名前付き capability/effect row、構造化並行性、明示的な公開シグネチャ、再現可能ビルド、機械可読な意味 API** を一つの意味モデルで統合する、プロダクション向け静的型付き言語である。

狙いは「LLM が短いコードを書きやすい言語」ではない。狙いは次の三つを同時に小さくすることや。

1. 変更前に理解しなければならない意味の範囲
2. 変更後に壊れ得る範囲
3. 壊れたときに原因へ戻るための探索量

この三つを小さくすると、LLM の修正成功率だけでなく、人間のレビュー、障害対応、移行、監査も楽になる。

## 3. 中核判断

### 3.1 通常の値は GC、希少資源だけアフィン

通常のサービスコードへ全面的な所有権・借用証明を課さない。値と不変データは管理メモリ上に置く。一方、ファイル、ソケット、ロックガード、トランザクション、秘密の開示権のように「複製してはいけない」「確実に終了させたい」ものだけを `resource` としてアフィンに扱う。

### 3.2 外界への権限を capability と effect row にする

時刻、乱数、環境変数、ファイル、ネットワーク、データベース、秘密、プロセス起動は ambient authority にしない。関数の `uses` 節へ名前付き capability として現す。これにより、依存、テスト差し替え、監査、影響解析が同じ情報から得られる。

### 3.3 並行性は lexical scope に閉じる

通常のタスクは親スコープより長生きできない。取消、期限、失敗、観測コンテキストを親子で伝播する。切り離した常駐タスクは、明示的な `Supervisor` capability と lifecycle policy を通す。

### 3.4 テキストを正本にし、意味操作をプロトコル化する

AST やグラフ DB をソースの正本にしない。Git diff、コメント、レビュー、既存エディタとの相性を守るため、正本は canonical formatter を通したテキストにする。

その代わり、コンパイラが lossless CST、型付き HIR、参照グラフ、効果グラフ、テスト影響グラフを保持し、**Mendrel Agent Protocol（MAP）** を通じて snapshot-aware な意味操作を提供する。LLM は生テキストの当てずっぽうな置換ではなく、rename、signature change、hole fill、API diff などをトランザクションとして要求できる。

### 3.5 言語仕様と本番運用を分離しない

`Result`、capability、deadline、wire schema、secret redaction、structured logging、reproducible build、SBOM、provenance、API compatibility は別々の追加ツールではない。同じ型情報と artifact graph から検査・生成する。

## 4. 何を「LLM 向け」と呼ぶか

Mendrel では、LLM 向けという言葉を次の測定可能な性質に限定する。

- 宣言を単独で読んだとき、引数、戻り値、失敗、非同期性、外部権限が分かる
- 同じ意味を持つ書き方が少なく、formatter が一つの形へ正規化する
- 名前解決、型、effect、resource、task lifetime の誤りが安定した diagnostic code と原因グラフで返る
- typed hole に対し、期待型、期待 effect、候補、必要な変換が返る
- repository 全体をプロンプトへ詰め込まず、コンパイラが task-specific context bundle を作れる
- 変更は revision hash 付きで preview され、影響する caller、test、API、wire schema、effect surface が分かる
- release build は incomplete hole、未承認 `unsafe`、未検査 generator output、互換性違反を拒否する
- benchmark で、修正ターン数、幻覚 symbol 数、回帰、レビュー時間を他言語と比較できる

構文が英語っぽい、冗長である、AST を直接出力する、といった性質だけでは「LLM 向け」とは呼ばない。

## 5. 想定する主戦場

初期版の主戦場は次の範囲に絞る。

- バックエンドサービス
- CLI、バッチ、ジョブ
- データ変換・ETL
- 信頼境界を持つ WebAssembly component
- 長期保守される業務ロジック
- 人間と coding agent が共同で変更する中規模から大規模 repository

v1 の主戦場にしないものは、ハードリアルタイム、極小組み込み、OS kernel、device driver、GPU kernel、既存 C++ ABI との無摩擦な置換、証明支援系そのものや。

## 6. ドキュメント構成

| ファイル | 役割 |
|---|---|
| `README.md` | 全体像、設計判断、読み方 |
| `AGENTS.md` | Codex を含む coding agent が従う実装規律 |
| `PROMPT_FOR_CODEX.md` | そのまま Codex に渡せる開始プロンプト |
| `docs/00-executive-decision.md` | 代替案比較と最終決定 |
| `docs/01-language-reference.md` | 表層構文・モジュール・式・宣言 |
| `docs/02-types-effects-capabilities.md` | 型、推論、effect、capability、error、contract |
| `docs/03-runtime-concurrency-memory.md` | メモリ、resource、async、task、supervision、FFI |
| `docs/04-production-toolchain.md` | package、build、供給網、観測、DB、配備 |
| `docs/05-agent-protocol.md` | MAP、diagnostic、typed hole、semantic edit |
| `docs/06-compiler-architecture.md` | compiler/runtime の内部構造と single source of truth |
| `docs/07-roadmap-and-acceptance.md` | 段階実装、各 phase の完了条件 |
| `docs/08-conformance-and-benchmarks.md` | conformance、fuzz、MendrelBench |
| `docs/09-adrs-risks-nongoals.md` | ADR、リスク、棄却案、機能採用基準 |
| `docs/10-formal-kernel.md` | 最小核、判断形式、動的意味論、健全性義務 |
| `docs/11-security-threat-model.md` | 脅威モデルと安全境界 |
| `docs/12-references.md` | 参照した一次資料と採用・不採用点 |
| `docs/13-derived-layers-and-lineage.md` | Onion/ASTER/Klassic/Macro PEG を core 外の公式 layer へ統合する設計 |
| `spec/grammar.ebnf` | v0.1 の機械可読文法骨格 |
| `schemas/diagnostic-v1.schema.json` | machine diagnostic の JSON Schema |
| `schemas/map-v1.schema.json` | MAP envelope の JSON Schema |
| `examples/checkout_service.mnd` | capability、error、deadline、resource の統合例 |
| `examples/Mendrel.pkg` | 宣言的 package manifest の例 |
| `examples/diagnostic.jsonl` | diagnostic schema に適合する具体例 |
| `examples/map-request.json` | MAP schema に適合する request 例 |
| `VALIDATION.md` | pack 自体へ実行した機械検査と既知の限界 |
| `scripts/validate_pack.py` | manifest・EBNF・schema・example の bootstrap 前検査 |
| `MANIFEST.md` | pack 内ファイルの SHA-256 と役割 |

## 7. Codex へ渡す順序

1. repository root にこの pack を配置し、`python scripts/validate_pack.py --strict-schema` を実行する。
2. `PROMPT_FOR_CODEX.md` の内容を最初の指示として渡す。
3. Codex に `README.md`、`AGENTS.md`、`docs/00`、`docs/06`、`docs/07` を先に読ませる。
4. 最初の実装は Phase 0 と Phase 1 だけに限定する。
5. parser、formatter、diagnostic JSON の golden test が安定するまで型検査へ進まない。
6. 各 phase は一つの薄い垂直スライスとして完成させ、巨大な横断実装を避ける。
7. 仕様と実装が衝突した場合、Codex は勝手に仕様を変えず、最小の ADR を提案する。

## 8. 規範語

この pack では、以下の語を規範的に使う。

- **MUST / 必須**: 実装が満たさなければならない
- **MUST NOT / 禁止**: 実装してはならない
- **SHOULD / 推奨**: 強い理由がない限り従う
- **MAY / 任意**: 互換性を壊さない範囲で実装してよい

文書間で矛盾した場合の優先順位は次の通り。

1. `docs/10-formal-kernel.md`
2. `docs/01`〜`docs/05`
3. `docs/11-security-threat-model.md`
4. `docs/06`〜`docs/09`
5. example と説明用コード

矛盾を見つけた実装者は、暗黙にどちらかを選ばず ADR を追加する。

## 9. 完成の定義

Mendrel が「成功した」と呼べるのは、単に self-host したときではない。最低限、次を再現可能な benchmark と production trial で示したときや。

- safe code に未定義動作がない
- data race が型検査を通らない
- function が宣言していない外部権限を使えない
- child task が lexical scope から漏れない
- debug/release で観測可能な意味が一致する
- 同じ source、lockfile、compiler、target、declared inputs から同じ artifact が得られる
- 公開 API、effect surface、wire schema の互換性違反を publish 前に分類できる
- seeded maintenance task で、比較対象より少ない context と修正ターンで正しい patch へ到達する
- 人間の reviewer が、通常の text diff と machine-generated blast-radius report を使って判断できる

## 10. Bootstrap implementation

Phase 0 と Phase 1 の最初の vertical slice は Rust workspace として実行できる。

```sh
cargo run -p mendrelc -- --version
cargo run -p mendrelc -- check crates/mendrel-parser/tests/fixtures/first_slice.mnd --error-format=json
cargo run -p mendrelc -- cst crates/mendrel-parser/tests/fixtures/first_slice.mnd
cargo run -p mendrel-cli -- fmt crates/mendrel-parser/tests/fixtures/first_slice.mnd
cargo run -p xtask -- verify
```

現在の parser subset は、一ファイルの `module` declaration と `pub fn` declaration に限定する。関数は一個以上の型注釈付き identifier parameter と明示 return type を持ち、body には identifier と二項 `+` だけからなる末尾式を一つ置く。qualified path、括弧、単項演算、`-`・`*`・`/`・`%`、空 parameter list / body、`internal`、visibility なし、`async`、`unsafe`、`move` parameter は、後続 slice の構文・意味規則を先取りせず `E-SYNTAX-UNSUPPORTED-0001` で拒否する。lexer は trivia、nested block comment、invalid token を保持し、parser は missing token と unsupported region を CST recovery element として残す。`fmt` は recovery のない CST だけを canonicalize し、malformed source を破壊的に書き換えない。

この bootstrap は AST/HIR、name/type/effect/resource checking、MIR、runtime、backend、package、MAP を実装しない。範囲外の top-level syntax は `E-SYNTAX-UNSUPPORTED-0001` で明示的に拒否する。
