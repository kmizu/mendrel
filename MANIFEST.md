# Mendrel Design Pack Manifest

> Generated from the exact bytes in this directory. `MANIFEST.md` itself is omitted to avoid a self-referential digest.

- Pack revision: design draft 0.2
- File count (excluding this manifest): 27
- Digest: SHA-256

| Path | Bytes | SHA-256 | Role |
|---|---:|---|---|
| `AGENTS.md` | 8,504 | `cc13558ccc8061ae56448f3638dfcc624b48b44bd2714a189d547bccf078489c` | coding agent の repository-wide 実装規律 |
| `PROMPT_FOR_CODEX.md` | 5,264 | `f294f80204f91318a3d67a5f4a10381e5283fc758c7bac9038182d8875fef66a` | Codex bootstrap prompt |
| `README.md` | 8,070 | `186c8a6872d93a78228568036086960f505adf462eb1e2324864c7ba28a00e52` | user-facing benefits, target experience, status, and quick start |
| `VALIDATION.md` | 1,992 | `d91c579b4f28f98548a157a4b6a39dd772f0fbbc5f226df962a31f20674edb8b` | pack 検証結果と deliberate limits |
| `docs/00-executive-decision.md` | 12,920 | `9ccae8a1a7dddb1d614f93ef3f2a523b90397494dc5d89ee3f6d4ab812ab2d4a` | 目的関数、代替案比較、最終決定 |
| `docs/01-language-reference.md` | 20,949 | `9c7f3be9e01d11f0417d988af7178267019579269e49ee4ca0ca9369010ac9c6` | 表層言語リファレンス |
| `docs/02-types-effects-capabilities.md` | 19,716 | `2c710e5708079dca239089f9c26b70dc92cdb84e7505530f78ca4e31b377a82c` | 型・効果・能力・エラー・契約 |
| `docs/03-runtime-concurrency-memory.md` | 17,424 | `3f0237bfd1171f6358932df0cb4b7c35fdc9019e5f35751c315f295dd46adf97` | GC・resource・並行性・runtime |
| `docs/04-production-toolchain.md` | 18,659 | `9d19ed68bf5b76fb17f47545618b668e02c381cb752c00c3b60a43082d09e94c` | build・package・運用・供給網 |
| `docs/05-agent-protocol.md` | 17,395 | `cd7b1ef8bc66bdf6c5dedd8360b4f1326debb026373854feb151ce517bb42493` | MAP・diagnostic・semantic edit |
| `docs/06-compiler-architecture.md` | 17,139 | `e735bb741efe333548ed147a3bb9c3eb852a732d3a63c96b18ab2e8240befa4a` | compiler/runtime architecture |
| `docs/07-roadmap-and-acceptance.md` | 12,772 | `cf5d3e4779ede4ece7e2fdcf78206b211689e990c9614d7d8f95740d87e9cd60` | phase、完了条件、導入順序 |
| `docs/08-conformance-and-benchmarks.md` | 13,999 | `1d79df607cd0f27a061bff88bb81bfc4a06acbbef8f4f8efb3ed1ba194012510` | conformance、fuzz、MendrelBench |
| `docs/09-adrs-risks-nongoals.md` | 18,214 | `f543eaccc5fdc20948bad72beb57973e7c672d644a262b764a730685f66b0596` | ADR、リスク、棄却案、kill criteria |
| `docs/10-formal-kernel.md` | 22,592 | `493546abf6194d934942dbc8b3cd34e0efe73895e18ff5f7d1d25da418d234d1` | 規範的意味核と健全性義務 |
| `docs/11-security-threat-model.md` | 15,249 | `10467080c7cdf220995a2a09f7a9de47059a4bfd6ec7ba200909045252ce9034` | 脅威モデルと trust boundary |
| `docs/12-references.md` | 14,363 | `357fceb14b38abbadad2eb7552053be027407d0c534c97f15c4954b8863089ba` | 一次資料と design lineage |
| `docs/13-derived-layers-and-lineage.md` | 15,358 | `6d3bcf7d5a4d6c5b2a1f92c41ffa8f561df2ae2fd74aa35bf879c1a427037cb1` | Onion・ASTER・Klassic・Macro PEG の derived layer 統合 |
| `docs/internal/design-pack-overview.md` | 5,092 | `ab129669a441e2c953278111adb2f27f16fec8f3d2a05af5f0301a4f6d8427e7` | implementation-facing design-pack index and bootstrap boundary |
| `examples/Mendrel.pkg` | 1,039 | `4ae311ad8ad416e59786c41542251b2ceb7b1e485c610e3f1c9a816401a5f3ef` | declarative package manifest example |
| `examples/checkout_service.mnd` | 3,290 | `d4c5615d8b63f6c55876349bbe1416641ebc4b369324a824b36eacfc1673cebb` | production-oriented Mendrel example |
| `examples/diagnostic.jsonl` | 1,836 | `d2766ee036d4938c7002c0ce6e6ef2fc0aca033bb0b68306af7bcc3cac0870e0` | schema-valid diagnostic example |
| `examples/map-request.json` | 416 | `e3d8dc18262ef1c90f21fa32e4f6f0c0b19c6a1d8a564834f3841ce69c37eb05` | schema-valid MAP request example |
| `schemas/diagnostic-v1.schema.json` | 9,131 | `816160a39a6e0f7dafde419d8cbe827bd44384f54d3d95bb243dba702e8cf46e` | diagnostic JSON Schema |
| `schemas/map-v1.schema.json` | 4,751 | `51e7dbb5295abbe38fa3663f4ca850fc598b8695d8df7864d2b9a6d18cafae9d` | MAP envelope JSON Schema |
| `scripts/validate_pack.py` | 8,023 | `5f439da4cb897a9d3e703657344c6f80e1e8f2b5d688e5b6725e69f2bdc40e4f` | pack manifest・EBNF・schema・example validator |
| `spec/grammar.ebnf` | 17,002 | `f2b85550ffb624f3b0469e6716b929a8ecdb1d8dae4cc37ca44ab9e98e7a1dad` | source/manifest grammar skeleton |

## Verification

```sh
python scripts/validate_pack.py --strict-schema
```

The validator checks path coverage before checking the individual size and digest, so an unlisted file is also a failure.
