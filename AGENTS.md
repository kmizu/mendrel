# AGENTS.md — Mendrel implementation contract

このファイルは Codex、Claude Code、その他の coding agent に対する repository-wide instruction や。人間の明示的な指示と承認済み ADR が最優先で、それ以外は本書に従う。

## 1. 仕事の進め方

1. 変更前に関連仕様、既存 test、同じ層の実装を読む。
2. 変更目的を一文で書き、触る subsystem と触らない subsystem を明示する。
3. 一回の変更は一つの意味的責務に限定する。
4. まず失敗する test または golden fixture を追加し、その後に実装する。
5. parser から runtime まで一気に骨組みだけ増やす横断 PR を作らない。
6. 公開 API を追加するときは、call site が一つしかなくても private で足りない理由を示す。
7. 「将来使いそう」を理由に abstraction、generic parameter、feature flag、hook を追加しない。
8. 実装完了を主張する前に、指定された verification command を実行し、結果を記録する。

## 2. 変更単位

推奨される一変更の上限は次の通り。

- 一つの grammar production と recovery
- 一つの diagnostic code と fix
- 一つの HIR node と lowering
- 一つの type/effect rule
- 一つの MIR instruction と verifier rule
- 一つの MAP method
- 一つの package/build invariant

複数を同時に変える必要がある場合も、薄い垂直スライスとして end-to-end test を一つ通す。無関係な cleanup は分離する。

## 3. Single source of truth

次の情報を複製して手書きしてはならない。

- keyword と punctuation
- builtin type と builtin function
- diagnostic code、severity、documentation URL、fix applicability
- syntax kind と AST/HIR lowering table
- wire primitive mapping
- MAP method/version table
- runtime intrinsic signature
- edition feature table

生成元を一つ置き、compiler、formatter、docs、schema、test fixture を生成または検査する。生成物は repository に置いてよいが、`mendrel xtask generated --check` で一致を検査する。

## 4. 仕様変更

仕様と実装が合わないとき、実装を都合よく正当化しない。次のいずれかを選ぶ。

- 実装を仕様へ合わせる
- `docs/09-adrs-risks-nongoals.md` に最小 ADR を提案し、人間の承認後に仕様を変更する
- phase の範囲外なら、明示的に未実装として diagnostic を出す

silent fallback、別の意味での受理、release mode だけの緩和は禁止する。

## 5. Parser と formatter

- parser は error-tolerant かつ lossless CST を生成する。
- source span は UTF-8 byte offset を正本とし、line/column は派生値にする。
- recovery node は捨てず、diagnostic と semantic query から参照できるようにする。
- formatter は一つだけで、意味保存・冪等でなければならない。
- formatter option は line width を含め原則固定する。v0.1 では設定項目を持たない。
- parser と formatter の round-trip property test を必須にする。
- コメント、doc comment、空行の意図を lossless CST で保存する。

## 6. Type、effect、resource

- 公開宣言の signature を body から推論して外部契約にしてはならない。
- type inference は declaration boundary を越えない。
- effect/capability row の label と type は両方一致して初めて自動転送できる。
- capability を type だけで ambient lookup しない。
- `resource` の consume/borrow/drop path を MIR verifier で全経路検査する。
- safe code の穴を runtime assertion で黙って埋めない。仕様上 runtime check である contract だけを実行時検査する。
- `unsafe` operation は専用 HIR/MIR node にし、通常 call と区別する。
- `unsafe` block/function/module の span と transitive dependency を artifact metadata に残す。

## 7. 並行性

- unstructured detached task API を stdlib に追加しない。
- task scope、cancellation、deadline、join policy を MIR 上で明示する。
- lock guard、transaction、borrowed resource を `await` 越しに保持できない規則を verifier で検査する。
- cancellation path でも `use` resource cleanup が実行される test を持つ。
- deterministic scheduler と production scheduler は同じ task semantics を共有する。
- test scheduler 固有の成功を production semantics と取り違えない。

## 8. Diagnostic

すべての user-facing error は次を持つ。

- 安定した code。例: `E-TYPE-0017`
- primary span
- 一文の summary
- structured cause graph
- expected/actual type・effect・resource state の該当項目
- recovery suggestion
- fix がある場合は applicability
- human text と JSONL の意味的一致

文字列全文を golden test の唯一の契約にしない。structured fields を schema test し、human rendering は別 golden test にする。

internal compiler error は stack trace をそのまま利用者へ投げず、query key、compiler revision、source revision、redacted crash bundle ID を出す。

## 9. MAP

- MAP request は必ず protocol version と workspace revision を持つ。
- semantic ID は snapshot-local であり、revision なしに永続化しない。
- edit は `plan -> preview -> commit` の三段階にする。
- commit は compare-and-swap で stale revision を拒否する。
- text source が正本。MAP は canonical text patch を生成する。
- preview は diff、diagnostic、affected callers、affected tests、API/effect/wire impact を返す。
- token budget を超える context bundle は無言で truncate せず、除外理由と continuation を返す。
- agent から generator、build、test を実行する際は sandbox policy を明示する。

## 10. Test pyramid

各 subsystem は最低限、次を持つ。

- unit test
- golden/fixture test
- property test または fuzz target
- cross-layer conformance test
- regression test

runtime と codegen は、可能な範囲で interpreter/reference evaluator との differential test を持つ。GC stress、cancellation injection、scheduler exploration、allocation failure simulation を CI profile に含める。

## 11. Performance

性能最適化は profile と benchmark の根拠なしに行わない。

- parser: incremental edit latency と full parse throughput
- type/effect: changed-query count と peak memory
- codegen: compile time、artifact size、runtime throughput
- GC: p50/p95/p99 pause、allocation rate、heap overhead
- MAP: context bundle latency、semantic edit latency
- service: startup、steady-state、tail latency

最適化が diagnostic quality、determinism、debug/release parity を壊す場合は採用しない。

## 12. Dependency policy

- compiler core の新規 dependency は、必要性、maintenance、license、supply-chain risk を ADR へ記録する。
- parser generator、query engine、LLVM binding、Wasm toolingの採用は、薄い adapter boundary を通す。
- production language semantics を upstream library の偶然の挙動へ委ねない。
- MMTk は将来 backend 候補であり、v0.1 の production readiness を依存させない。
- build script に任意コード実行を持ち込まない。

## 13. Verification commands

設計 pack の bootstrap 前には `python scripts/validate_pack.py --strict-schema` を実行する。repository が育った後も、次の command surface を維持する。

```sh
mendrel xtask generated --check
mendrel fmt --check
mendrel lint --all --deny-warnings
mendrel check --workspace --all-targets
mendrel test --workspace
mendrel test --profile deterministic-concurrency
mendrel test --profile gc-stress
mendrel fuzz --smoke
mendrel conformance
mendrel api diff --against baseline/api.json
mendrel schema diff --against baseline/schema.json
mendrel effect diff --against baseline/effects.json
mendrel build --release --reproducible
mendrel artifact verify target/release/*.mra
```

bootstrap 中は同等の `cargo xtask ...` command を使ってよいが、CI job 名と意味は上記へ揃える。

## 14. Completion report

変更を終えた agent は、最後に次だけを簡潔に報告する。

- 何を変えたか
- どの invariant を満たすためか
- 実行した test と結果
- 残る既知の制約
- API/effect/wire/unsafe surface の増減
