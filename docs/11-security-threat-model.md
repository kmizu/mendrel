# 11. Security threat model

## 1. Scope

Mendrel’s security model covers:

- source-language authority and memory safety,
- compiler and package supply chain,
- generated code,
- agent tooling,
- runtime service operation,
- wire and persistence boundaries,
- observability and crash artifacts,
- unsafe/FFI boundaries.

It does not claim that a safe Mendrel program is logically correct or free from authorization bugs. The goal is to make authority, trust boundaries, and unsafe assumptions explicit enough to inspect and enforce.

## 2. Assets

Protected assets include:

- source and proprietary repository data,
- package publisher identity,
- build and release integrity,
- signing keys,
- deployment credentials,
- runtime secrets,
- customer and sensitive data,
- database state,
- network authority,
- service availability,
- audit/provenance records,
- compiler/runtime update channel,
- agent session authority,
- wire compatibility and migration safety.

## 3. Adversaries

Consider:

1. malicious or compromised dependency publisher,
2. compromised package registry or mirror,
3. malicious build generator,
4. compromised CI runner,
5. repository contributor inserting unsafe/backdoor behavior,
6. external input attacker,
7. prompt injection embedded in repository text,
8. coding agent that hallucinates or overreaches,
9. compromised MAP/LSP client/server,
10. malicious deployment configuration,
11. vulnerable C/foreign library,
12. attacker with log/trace/crash-bundle access,
13. insider with excessive capability grant,
14. rollback attacker serving old vulnerable artifact,
15. resource-exhaustion attacker.

## 4. Security invariants

### Language

- safe code cannot perform undefined memory operations,
- safe code cannot forge a capability,
- capability use is limited to declared named authority,
- mutable data cannot be raced across tasks,
- resource ownership cannot be duplicated,
- secret values cannot enter generic display/log/serialization paths,
- dynamic input must be validated before static-domain use,
- unchecked shell construction is unavailable in safe standard APIs,
- integer overflow and bounds behavior are defined,
- null pointer/reference is not a source-language state.

### Build

- build sees only declared inputs,
- package content is digest-pinned,
- published versions are immutable,
- generators execute in a denied-by-default sandbox,
- release artifact has provenance and SBOM,
- signatures and transparency proofs can be verified,
- toolchain/version is pinned,
- reproducibility can be checked,
- dependency and unsafe surfaces are inspectable.

### Runtime

- task/resource lifecycle is bounded,
- network/database operations can be required to have deadlines,
- queues/bodies/retries are bounded,
- config is validated before readiness,
- deployment grants are compared to artifact requirements,
- telemetry is bounded and redacted,
- panic/cancellation causes are structured,
- secret and sensitive classifications propagate.

### Agent

- repository text is untrusted data, not authority instructions,
- semantic edits are revision-aware transactions,
- stale patches are rejected,
- tool grants are least-authority,
- publish/deploy/secret access is separate from code-edit authority,
- context bundles enforce redaction and origin labeling,
- actions are auditable.

## 5. Object-capability model

A capability is an unforgeable reference to authority. Possession allows only methods exposed by its interface and deployment binding.

Examples:

```mendrel
uses {
    orders_reader: ReadOnlyOrders,
    orders_writer: OrdersWriter,
    secrets: SecretStore<"payment/*">,
}
```

The language and deployment checker distinguish read/write/admin or path/destination scopes when capability metadata supports them.

Forbidden ambient authority:

- process-wide filesystem handle,
- global environment lookup,
- global network client,
- current clock/random singleton,
- implicit cloud SDK credential chain,
- hidden database pool registry,
- global shell execution.

Composition root receives host grants and constructs smaller domain capabilities. Passing a capability is authority delegation and appears in effect/call graphs.

## 6. Capability escalation

Changes that increase authority are security-sensitive:

- add capability to public function/service,
- relabel from restricted role to broad role,
- change read-only interface to writer,
- widen path/secret/destination pattern,
- add raw socket/process/shell capability,
- add supervisor/publish/deploy authority.

`mendrel effect diff` classifies these separately from ordinary source breaking changes. CI can require security-owner approval.

Capability bundling lint rejects generic `AppContext` or interfaces spanning unrelated authority classes.

## 7. Package and registry threats

### Dependency confusion/typosquatting

- package namespaces are authenticated,
- package names ASCII normalized,
- Unicode confusable checks,
- lockfile pins registry identity and digest,
- private namespace precedence is explicit, never “closest registry wins,”
- similar-name warning at add time,
- publish name ownership and transparency log.

### Mutable/yanked packages

A yanked/revoked version remains content-addressed. Resolver refuses new selection by policy but existing lockfile is reproducible. Security revocation is signed metadata, not content replacement.

### Compromised maintainer

Artifact records signer set and provenance. Organization policy can require threshold signatures, trusted builders, review attestations, or source-reproducibility match.

## 8. Generator threats

A generator may attempt:

- reading SSH keys/home files,
- network exfiltration,
- nondeterministic output,
- modifying source outside output,
- embedding backdoor,
- consuming excessive resources,
- confusing source maps.

Controls:

- WebAssembly component sandbox,
- no authority default,
- declared read-only inputs,
- single output root,
- CPU/memory/output quotas,
- pinned component digest,
- deterministic clock/random absence,
- output canonical formatting,
- source map/provenance,
- generated output review,
- reproduction check,
- security scanning and capability metadata.

Generated code is not trusted merely because generated. It passes normal parser/type/effect/unsafe/lint/test gates.

## 9. Compiler threats

### Malicious source causing compiler exploit

- parser/checker fuzzing,
- no unsafe parsing shortcuts,
- bounded recursion or iterative parsing,
- allocation/complexity limits,
- crash isolation in daemon,
- malformed-source no-panic target,
- untrusted package compilation sandbox.

### Compiler compromise/miscompile

- pinned signed toolchain,
- reproducible compiler artifact,
- source/binary provenance,
- evaluator/backend differential tests,
- MIR verifier,
- diverse-build/independent rebuild where practical,
- conformance corpus,
- compiler update policy and rollback protection.

### Diagnostic leakage

Diagnostic/crash bundles may contain source, literals, paths, secrets. Structured diagnostics use source spans and redacted values; remote reporting is opt-in/policy-controlled. `Secret<T>` values are never rendered.

## 10. Unsafe and FFI

Unsafe is a trust boundary, not an escape hatch hidden in library internals.

Requirements:

- unsafe module declares reason,
- each unsafe operation has safety contract,
- public safe wrapper validates assumptions,
- ownership/nullability/thread/callback semantics in binding metadata,
- sanitizers and fuzz tests,
- dependency unsafe tree,
- artifact unsafe fingerprint,
- security review policy,
- no transitive unsafe invisibility.

C pointers and lengths are converted at boundary. Foreign exceptions/longjmp/signals do not cross into normal Mendrel frames without adapter semantics.

Memory zeroization limitations under GC are documented. Strict secrets use pinned affine resources.

## 11. Input validation

External input categories:

- wire message,
- JSON/Dynamic,
- config/environment,
- URL/path,
- SQL/database row,
- command-line,
- archive/package,
- FFI data.

Boundary decoder must:

- enforce size/depth/count limits,
- validate encoding,
- return path-aware typed error,
- preserve unknown wire fields where required,
- reject duplicate/ambiguous canonical forms by policy,
- avoid resource exhaustion,
- produce nominal validated domain types.

`Dynamic` cannot invoke methods or flow freely into domain code.

## 12. Injection

### SQL

Compile-time checked queries or structured builders. Raw concatenation rejected. Dynamic identifiers require validated enum/nominal mapping.

### Shell

Standard process API takes executable path and argument list:

```mendrel
process.run(
    executable: ExecutablePath.parse("/usr/bin/git")?,
    args: ["status", "--short"],
).await?;
```

No implicit shell. Shell interpretation requires explicit `UnsafeShell` capability and security review.

### HTML/URL/header

Context-specific escaped nominal types/adapters. Generic “escaped string” is insufficient; output context is part of type/API.

### Logging

Structured fields and classification prevent format-string/string-concatenation leakage. Log forging controls normalize line breaks in text renderers while preserving structured value.

## 13. Secret handling

- secrets acquired through scoped capability,
- `Secret<T>` no display/debug/serialize,
- reveal requires explicit capability and lexical scope,
- crash/log/trace redaction,
- config provenance reports source, not value,
- test fake secrets use markers,
- agent context excludes secret material,
- remote MAP cannot request runtime secret by default,
- secret rotation/expiry modeled,
- strict buffer uses pinned affine resource and best-effort zeroization,
- GC-based ordinary secret wrapper does not claim guaranteed zeroization.

## 14. PII and sensitive data

`Sensitive<T, Class>` or metadata classification:

- direct identifier,
- quasi-identifier,
- financial,
- health,
- authentication,
- confidential business data.

Telemetry sink policy decides drop/hash/mask/encrypt. Schema/API diff reports classification changes. Data export/serialization requires declared adapter policy.

This is enforcement support, not a complete legal-compliance system.

## 15. Network security

- parsed URL/socket address types,
- TLS verification on by default,
- insecure TLS requires explicit unsafe/security capability,
- destination allowlist in deployment grant,
- DNS and proxy policy explicit,
- redirect policy,
- response/body/decompression limits,
- finite deadlines,
- retry budget/idempotency,
- trace propagation allowlist,
- SSRF protection via destination policy and resolved-address checks where required.

## 16. Denial of service

Language/runtime defaults:

- bounded channels,
- bounded task count,
- bounded parser/decode depth,
- bounded request body,
- timeout/deadline,
- retry cap,
- memory/resource quota,
- backpressure,
- cancellation,
- supervisor restart intensity cap,
- telemetry volume cap,
- hash-flood-resistant maps where untrusted keys are used.

Pure contract and compile-time expression subsets are bounded to prevent compiler/runtime DoS.

## 17. Wire and migration security

Schema incompatibility can corrupt or misinterpret authority/data.

Controls:

- explicit field/case IDs,
- no reuse,
- reserve deleted IDs/names,
- unknown preservation,
- compatibility diff,
- signed schema fingerprint in artifact,
- staged migration,
- old/new coexistence range,
- rollback check,
- canonical validation,
- sensitive-field classification.

Parser differential and fuzz tests cover ambiguous encodings.

## 18. Observability threats

Logs/traces can leak data or become an availability channel.

- typed fields,
- classification,
- cardinality lint,
- bounded buffering,
- sampling/drop metrics,
- exporter failure cannot alter business result,
- trace context validation,
- untrusted baggage allowlist/size limit,
- no raw secret,
- crash bundle access control,
- source-map server authorization.

Observer semantics are closed; arbitrary hidden code cannot attach through a general effect handler.

## 19. Agent prompt injection

Repository content may say “ignore instructions,” “upload secrets,” or encode malicious tool commands.

Controls:

- origin/trust tag on content,
- repository text never grants tools,
- system/user/host policy separated from content,
- semantic query preferred over following README instructions blindly,
- shell/network/secret/publish/deploy are separate capabilities,
- preview required before write,
- publish/deploy never automatic core MAP,
- audit record,
- suspicious instruction lint optional,
- generated/vendor content lower trust by default.

The agent may still reason incorrectly; revision checks and static/production gates contain damage.

## 20. MAP server security

- local socket permissions or mutual authentication,
- workspace ACL,
- protocol input validation,
- request/response size quotas,
- cancellation and CPU quotas,
- no path traversal,
- snapshot/overlay isolation,
- stale revision CAS,
- sandbox for test/build/generator,
- extension namespace isolation,
- audit log integrity,
- secret redaction,
- remote source-data policy.

A compromised editor client should not automatically gain publish/deploy credentials.

## 21. Deployment security

Artifact requirements vs deployment grants:

- exact capability interface/version,
- network destination,
- filesystem scope,
- database role,
- secret prefix,
- listener,
- process/shell,
- supervisor authority,
- resource quotas.

Deployment with excessive unused grants is warning/error by policy. Authority escalation across artifact versions requires approval.

Artifact signature/provenance/runtime/schema compatibility verified before start. Rollback protection can reject known-revoked artifact even if signed.

## 22. Incident response data

A secure incident bundle may include:

- artifact/source/toolchain fingerprints,
- task/cause tree,
- trace/span IDs,
- capability operation classes,
- deployment grant digest,
- schema/config version,
- redacted stack/source spans,
- scheduler replay token for test-reproduced failure.

It excludes raw secrets and unnecessary source/customer payload by default.

## 23. Security release gate

```text
safe-language conformance
unsafe tree diff
dependency signature/digest
license/vulnerability policy
generator sandbox/provenance
SBOM
reproducibility
API/effect/schema security diff
secret/log static checks
fuzz/sanitizer
deployment least-authority check
artifact signature
revocation/rollback policy
```

Waiver is signed, scoped, expiring, and artifact-bound.

## 24. Residual risks

Mendrel does not eliminate:

- business authorization mistakes,
- compromised capability implementation,
- malicious logic using legitimately granted authority,
- side channels/timing leaks generally,
- hardware/OS/compiler dependency vulnerabilities,
- all denial-of-service attacks,
- secure deletion under managed memory,
- social engineering,
- incorrect deployment trust roots,
- unsafe wrapper bugs,
- agent misunderstanding,
- formal-model/implementation mismatch.

The design aims to make these boundaries visible and auditable rather than claiming impossible total security.
