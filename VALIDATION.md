# Mendrel design pack validation report

- Pack revision: design draft 0.2
- Validation date: 2026-08-18
- Validator: `scripts/validate_pack.py`
- Strict command: `python scripts/validate_pack.py --strict-schema`

## Checks

The validator checks:

1. every non-manifest file is listed exactly once in `MANIFEST.md`;
2. byte size and SHA-256 digest match;
3. every EBNF nonterminal reference resolves;
4. no non-root EBNF production is orphaned;
5. JSON and JSONL parse;
6. both JSON Schemas meta-validate under Draft 2020-12;
7. diagnostic and MAP examples validate against their schemas;
8. Markdown code fences are balanced;
9. unresolved implementation-placeholder markers are absent;
10. the production example does not contradict the no-shadowing and receiver-move conventions checked by the pack validator.

## Result

The final distributed pack passes the strict command with zero manifest mismatch, zero undefined/orphan EBNF production, and schema-valid examples.

## Deliberate limits

This validation does **not** claim that Mendrel is implemented.

- `spec/grammar.ebnf` has structural consistency, but no reference parser exists yet.
- `.mnd` examples have pack-level consistency checks, but cannot be compiled until Phase 1+ is implemented.
- formal soundness statements in `docs/10-formal-kernel.md` are proof obligations, not completed proofs.
- production GC, runtime, LLVM/Wasm backends, package registry, MAP daemon, and compatibility classifiers remain design commitments and acceptance criteria.
- reference links were selected from primary papers, standards, and official documentation, but future link availability is outside the artifact guarantee.
- the name **Mendrel** has only a preliminary collision search; trademark, registry, corporate-name, and domain clearance remain a pre-publication legal task.

A validator failure must be fixed by updating the underlying file and regenerating `MANIFEST.md`; weakening or deleting the check is not an acceptable repair.
