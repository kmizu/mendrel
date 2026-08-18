# 10. Formal kernel and soundness obligations

## 1. Status and purpose

This document is the normative semantic kernel for Mendrel draft 0.1. It is precise enough to constrain the implementation architecture, conformance tests, and later mechanization. It does **not** claim that the metatheory has already been machine-proved.

Before language 1.0, the core below MUST have:

1. an executable reference semantics,
2. a correspondence test against HIR/MIR,
3. a mechanized model for the central safety theorems,
4. a documented trusted computing base.

Lean 4 is the preferred mechanization target because the model includes inductive syntax, row relations, affine state transitions, and small-step task semantics. The implementation remains Rust; the proof model is not the compiler implementation.

## 2. Semantic layers

The language is specified in three layers.

### K0 — Pure value calculus

- immutable values
- functions
- nominal records/enums
- pattern matching
- `Option`/`Result`
- checked primitive operations
- local type inference erased before semantics

### K1 — Authority and affine resources

- named capabilities
- effect rows
- resource acquire/borrow/move/consume/cleanup
- panic and recoverable result
- contracts

### K2 — Structured tasks

- async suspension
- lexical task scope
- spawn/join
- cancellation/deadline
- supervisor boundary
- task-local mutable objects and isolated transfer

The compiler MAY desugar surface syntax into this kernel, but user diagnostics MUST map back to source constructs.

## 3. Core types

Let:

- \(a\) range over type variables,
- \(N\) range over nominal type constructors,
- \(\kappa\) range over capability interfaces,
- \(r\) range over resource constructors,
- \(\ell\) range over capability labels,
- \(\rho\) range over effect-row variables,
- \(\sigma\) range over task-scope tokens.

Core types:

\[
\begin{aligned}
\tau ::= {}&
\mathbf{Unit}
\mid \mathbf{Bool}
\mid \mathbf{Int}_w
\mid \mathbf{Float}_w
\mid \mathbf{Text}
\mid \mathbf{Bytes}
\mid a \\
&\mid N\langle \bar{\tau}\rangle
\mid \tau_1 \times \cdots \times \tau_n \\
&\mid \mathbf{Option}\langle\tau\rangle
\mid \mathbf{Result}\langle\tau,\epsilon\rangle \\
&\mid (\bar{\tau}) \xrightarrow{e} \tau \\
&\mid \mathbf{Cap}\langle\kappa\rangle
\mid \mathbf{Res}\langle r,\bar{\tau},q\rangle \\
&\mid \mathbf{Owned}\langle\tau\rangle
\mid \mathbf{Task}\langle\sigma,\tau,\epsilon\rangle
\mid \mathbf{Never}
\end{aligned}
\]

Here \(q\) is a finite typestate when a resource declares one.

`object` types are modeled in K2 as task-owned heap references:

\[
\mathbf{Obj}\langle o,\sigma\rangle
\]

They cannot be named with an arbitrary \(\sigma\) in surface syntax; the compiler owns scope tokens.

## 4. Effect rows

A closed effect row is a finite map from semantic labels to capability interfaces:

\[
e = \{\ell_1 : \kappa_1, \ldots, \ell_n : \kappa_n\}
\]

An open row is:

\[
e = \{\ell_1 : \kappa_1, \ldots, \ell_n : \kappa_n \mid \rho\}
\]

Properties:

- labels are unique,
- order is semantically irrelevant,
- source order is retained only for rendering,
- equality compares label and interface,
- subeffecting is finite-map inclusion plus row-variable constraints,
- duplicate-label unification is rejected in Mendrel even if an underlying row calculus could represent it,
- a label rename is an effect-surface change, not alpha-equivalence.

Define:

\[
e_1 \preceq e_2
\]

when every entry in \(e_1\) occurs with a compatible capability interface in \(e_2\), subject to row-variable constraints.

## 5. Contexts

Typing uses the following contexts.

- \(\Sigma\): global signatures, nominal declarations, trait implementations, capability/resource method signatures.
- \(\Gamma\): unrestricted immutable bindings.
- \(\Delta\): affine owned bindings and their current typestate.
- \(B\): active lexical borrows.
- \(A\): available named capabilities.
- \(\sigma\): current lexical task scope.
- \(P\): current safety profile (`safe` or an audited unsafe provenance).

The central judgment is:

\[
\Sigma;\Gamma;\Delta;B;A;\sigma;P
\vdash
e : \tau
\;\dashv\;
\Delta'
\;!\;
\varepsilon
\]

Read:

> Under signatures \(\Sigma\), unrestricted context \(\Gamma\), affine state \(\Delta\), borrow state \(B\), available capabilities \(A\), task scope \(\sigma\), and safety provenance \(P\), expression \(e\) produces type \(\tau\), leaves affine state \(\Delta'\), and may perform capability effects \(\varepsilon\).

The effect output records authority actually required by the expression. A named function body is accepted only if:

\[
\varepsilon \preceq e_{\text{declared}}
\]

and every capability entry in \(\varepsilon\) is available through \(A\) under the same label or an explicit source-level remapping.

## 6. Unrestricted values

A type belongs to unrestricted context only if it is not affine.

Define predicate:

\[
\operatorname{Unrestricted}(\tau)
\]

This includes:

- primitive immutable values,
- immutable record/enum whose fields are unrestricted,
- immutable persistent collections of unrestricted values,
- ordinary function closures that capture only unrestricted values,
- capability handles only when the capability declaration permits share/copy semantics.

It excludes:

- `resource`,
- `Owned<T>`,
- task handles,
- lexical borrow tokens,
- pinned memory,
- strict secret buffers,
- local mutable objects.

Capability handles are authority-bearing even when physically copyable. Copyability does not grant construction authority; only existing handles may be propagated according to their marker policy.

## 7. Affine variable rules

### 7.1 Move

If \(x:\tau \in \Delta\):

\[
\frac{
x:\tau \in \Delta
}{
\Sigma;\Gamma;\Delta;B;A;\sigma;P
\vdash
\mathbf{move}\;x : \tau
\dashv
\Delta \setminus \{x\}
!
\varnothing
}
\]

A later use of \(x\) is ill-typed.

### 7.2 Borrow

Shared borrow:

\[
\frac{
x:\tau \in \Delta
\quad
\operatorname{BorrowableShared}(\tau)
\quad
\operatorname{NoExclusiveBorrow}(x,B)
}{
\Sigma;\Gamma;\Delta;B;A;\sigma;P
\vdash
\&x : \mathbf{Borrow}\langle\tau,b\rangle
\dashv
\Delta
!
\varnothing
}
\]

where fresh borrow token \(b\) is added to \(B\) for the lexical subjudgment.

Exclusive borrow requires no active borrow of \(x\). Borrow values cannot be stored in ordinary heap values, returned, spawned, or survive beyond their compiler-owned region unless a built-in scoped view form explicitly models the region.

### 7.3 Branch join

For `if`/`match`, all reachable branches MUST leave join-compatible affine states:

\[
\operatorname{JoinAffine}(\Delta_1,\ldots,\Delta_n)=\Delta_j
\]

Draft 0.1 defines join-compatible as:

- the same live affine bindings,
- the same resource typestate for each live binding,
- no live branch-local borrow,
- branch-local affine values consumed or moved into the result.

Programs needing different resource states across branches MUST encode the state difference in a returned nominal enum/resource typestate rather than relying on hidden flow merging.

This restriction intentionally favors explainability over maximal flow sensitivity.

## 8. Function typing

A declared function has signature:

\[
f :
(\bar{\tau})
\xrightarrow{e_d}
\mathbf{Result}\langle\tau,\epsilon\rangle
\]

or a non-`Result` return where no recoverable error is exposed.

The body is checked with capability context \(A=e_d\). If body effect is \(e_b\), require \(e_b \preceq e_d\).

Public/module-level declaration MUST not infer \(e_d\) from \(e_b\). A compiler fix may materialize a candidate declaration, but source acceptance requires the declaration.

Function calls:

\[
\frac{
\Sigma(f)=(\bar{\tau})\xrightarrow{e_f}\tau
\quad
\bar{e_i}:\bar{\tau}
\quad
e_f \preceq A
}{
\Sigma;\Gamma;\Delta;B;A;\sigma;P
\vdash
f(\bar{e_i}) : \tau
\dashv
\Delta'
!
(\bigcup_i \varepsilon_i)\cup e_f
}
\]

Affine argument modes determine how \(\Delta\) changes. Evaluation is left-to-right, so the affine/effect state is threaded through arguments in source order.

## 9. Capability calls

Suppose capability interface \(\kappa\) declares method:

\[
m : (\bar{\tau}) \xrightarrow{e_m} \tau
\]

A call through semantic label \(\ell\):

\[
\ell.m(\bar{v})
\]

is legal only if:

\[
A(\ell)=\kappa'
\quad\text{and}\quad
\kappa' \leq \kappa
\]

The effect includes \(\ell:\kappa\), regardless of whether the capability implementation is local, fake, remote, or optimized away.

There is no rule that searches \(A\) by capability type alone. An explicit source remapping creates a lexical renaming relation for a call:

\[
\ell_{\text{callee}} \mapsto \ell_{\text{caller}}
\]

and this relation is included in HIR/MAP explanation metadata.

## 10. Error propagation

`Result<T,E>` is an ordinary nominal sum with privileged `?` lowering.

For:

```mendrel
let x = e?;
k(x)
```

the kernel lowering is observationally equivalent to:

```mendrel
match e {
    Ok(x) => k(x),
    Err(error) => return Err(convert_error(error)),
}
```

Requirements:

- `convert_error` is identity, explicit mapping, or the unique permitted one-step `FromError`,
- all affine cleanup edges between `e` and function exit run before returning `Err`,
- capability effects of `e` and conversion are included,
- conversion cause is retained for diagnostics.

No exception stack is part of recoverable semantics.

## 11. Resource acquisition and `use`

Surface:

```mendrel
use r = acquire()?;
body
```

Kernel behavior:

1. evaluate `acquire`,
2. on error, return without introducing `r`,
3. on success, add \(r:\mathbf{Res}\langle...\rangle\) to \(\Delta\),
4. evaluate body,
5. if body has not consumed/moved `r`, execute declared cleanup,
6. combine body and cleanup outcomes according to deterministic cause rules.

Typing is expressed with an internal cleanup judgment:

\[
\operatorname{CloseScope}(r,\Delta_b,\Delta_o)
\]

which either:

- observes that `r` was consumed,
- inserts cleanup and removes `r`,
- rejects an invalid resource state.

`use` MUST establish that `r` is absent from the outer affine context.

### Outcome combination

Let an evaluation outcome be:

\[
o ::= \operatorname{Value}(v)
\mid \operatorname{Error}(e)
\mid \operatorname{Cancelled}(c)
\mid \operatorname{Panic}(p)
\mid \operatorname{Abort}(a)
\]

Cleanup returns either success or cleanup error \(c_e\).

- `Value(v)` + cleanup error → `Error` when function error policy can represent it, otherwise structured panic `CleanupFailure`.
- `Error(e)` + cleanup error → primary `Error(e)` with suppressed cleanup cause.
- `Cancelled(c)` + cleanup error → cancellation with suppressed cleanup cause.
- `Panic(p)` + cleanup error → panic report with suppressed cleanup cause.
- `Abort(a)` makes no cleanup guarantee.

The exact recoverable conversion is resource/function-signature dependent and MUST be explicit in lowering metadata.

## 12. Panic

Panic is not a capability effect and is not recoverable by ordinary application expressions. It is an abnormal typed runtime outcome handled at task/supervisor/process boundaries.

The static system aims to eliminate ordinary panic sources, but operations such as explicit `expect`, contract violation, impossible unsafe state, and resource cleanup failure may panic.

A function may carry panic metadata in the API artifact for audit, but Mendrel does not add checked-exception-style panic rows to every signature in draft 0.1.

This is a deliberate compromise: recoverable errors remain explicit while invariant failure remains observable and structured without infecting ordinary call signatures.

## 13. Contracts

A contract expression is checked under:

\[
\Sigma;\Gamma;\varnothing;\varnothing;\varnothing;\sigma;\text{safe}
\]

and must have type `Bool`, empty capability effect, and belong to the terminating contract subset.

A function precondition \(pre\) and postcondition \(post\) induce dynamic checks unless discharged under an accepted proof artifact.

Semantic rule:

- evaluate `pre` before body-visible effects,
- on false, produce `ContractViolation`,
- evaluate body,
- on normal/recoverable result where specified, evaluate `post` with `old` snapshot and result,
- on false, produce `ContractViolation`,
- cleanup semantics still apply.

Contract evaluation cannot itself access resources/capabilities or mutate state.

## 14. Task scopes

### 14.1 Scope token

Entering:

```mendrel
scope tasks { body }
```

creates fresh \(\sigma_c\), a child of current scope \(\sigma_p\).

Task handles created inside have type:

\[
\mathbf{Task}\langle\sigma_c,\tau,\epsilon\rangle
\]

No value whose type mentions \(\sigma_c\) may escape the scope result.

Surface programmers cannot name or quantify over arbitrary scope tokens.

### 14.2 Spawn capture

A spawned closure partitions captures into:

- shared capture \(x:\tau\), requiring `Share(τ)`,
- moved capture \(x:\tau\), requiring `Send(τ)` and consuming affine ownership when applicable,
- borrowed capture, forbidden.

The child effect row \(e_c\) MUST be a subset of capabilities delegated to the child. Delegation preserves labels unless explicitly remapped.

The child receives no ambient access to capabilities present only in the parent implementation.

### 14.3 Await

At an `await` suspension:

- every active borrow in \(B\) MUST be `SuspendSafe`; draft 0.1 ordinary lexical borrows are not,
- every live affine value stored in the suspended frame MUST implement `SuspendSafe`,
- every local mutable object in the frame remains owned by the same task,
- no lock guard/transaction borrow may cross unless its declaration safely implements `SuspendSafe`, which standard guards do not.

The compiler may move safe owned resources into the async state machine.

### 14.4 Scope exit

A task scope cannot produce a normal outcome until every child is in a terminal state and its resources have been cleaned.

For fail-fast policy:

1. first non-success outcome selects the primary cause by deterministic order,
2. unfinished siblings receive cancellation,
3. all siblings are joined,
4. outcomes and cleanup failures are aggregated,
5. parent receives the structured result.

Collect-all policy waits for all children and returns an ordered aggregate.

## 15. Cancellation and deadlines

Cancellation is an injected task outcome at specified cancellation points, not an arbitrary asynchronous exception at every instruction.

A deadline is an absolute monotonic instant in task context. Nested deadline composition:

\[
d_{\text{child}} =
\min(d_{\text{parent}}, d_{\text{requested}})
\]

where absence is \(+\infty\).

Capability methods declared cancellable observe task cancellation/deadline according to their interface contract. A method that ignores cancellation must say so in capability metadata and is rejected by latency-sensitive policy where finite cancellation is required.

Cancellation runs lexical cleanup before scope completion unless the runtime aborts.

## 16. Mutable object ownership

Each local object allocation is tagged in the semantic model with owner task \(t\).

Allowed:

- owner task reads/writes,
- immutable snapshot creation,
- unique isolation into `Owned<T>`,
- transfer of `Owned<T>` to another task.

Forbidden in safe code:

- direct reference from two tasks to mutable object,
- shared mutable alias captured by spawn,
- object reference sent through channel,
- mutation through a `Share` value.

`freeze_ownership` succeeds only when the reachable mutable object graph has no external aliases and all contained values are transferable. The implementation may use static construction discipline, ownership bits, or a hybrid verification mechanism, but the observable success/failure contract MUST match the model.

## 17. Small-step runtime configuration

A simplified runtime configuration:

\[
\langle H, R, Q, S, D, e \rangle
\]

where:

- \(H\): managed heap and task ownership metadata,
- \(R\): live resource table and typestate,
- \(Q\): task tree and runnable queues,
- \(S\): lexical cleanup stacks,
- \(D\): task deadlines/cancellation state,
- \(e\): current expression or machine state.

Transition:

\[
\langle H,R,Q,S,D,e\rangle
\rightarrow
\langle H',R',Q',S',D',e'\rangle
\]

The production scheduler may choose among runnable child transitions, but:

- task-tree parentage,
- scope-exit join,
- resource cleanup,
- cancellation propagation,
- result aggregation

are scheduler-independent semantic constraints.

The deterministic test scheduler records choices in \(Q\); replay fixes them.

## 18. Garbage collection abstraction

The semantic heap treats ordinary object reachability abstractly. A GC step:

\[
\operatorname{GC}(H,\operatorname{Roots}(Q,S,R)) = H'
\]

must preserve all reachable semantic values and may reclaim unreachable managed objects.

GC MUST NOT:

- reclaim live resources solely because wrappers are unreachable without executing lexical resource semantics,
- run user code,
- expose address change,
- change equality/hash semantics,
- inject cancellation,
- alter pure results.

Resource table entries are owned by lexical/resource runtime state, not finalized by heap reachability.

## 19. Unsafe boundary

Unsafe primitives have judgments available only when \(P\) contains an audited unsafe provenance.

\[
P \vdash \operatorname{unsafe\_op}
\]

The safe wrapper theorem obligation is local:

> If callers satisfy the documented safe precondition, the unsafe implementation preserves Mendrel’s safe-language invariants.

Compiler acceptance of `unsafe` is not proof of that obligation. Artifact metadata identifies the trust boundary for audit and fuzzing.

## 20. Core soundness statements

The following are 1.0 proof obligations.

### 20.1 Progress modulo declared outcomes

A closed, well-typed safe program is either:

- a value,
- a recoverable `Result` error,
- waiting at a well-defined async/capability operation,
- cancelled,
- panicked with a structured report,
- terminated by a specified runtime abort,
- or can take a semantic step.

It does not get stuck on an undefined operation.

### 20.2 Preservation

If:

\[
C \text{ is well typed}
\quad\text{and}\quad
C \rightarrow C'
\]

then \(C'\) is well typed under the corresponding evolved heap/resource/task contexts.

### 20.3 Capability confinement

For a well-typed function with declared row \(e_d\), every dynamic capability invocation is authorized by an entry reachable through \(e_d\) and explicit delegation/remapping.

No safe evaluation creates new authority.

### 20.4 Affine resource safety

In non-aborting executions:

- a resource owner is never duplicated,
- a consumed resource is never used again,
- cleanup/consume occurs at most once,
- a `use`-bound resource cannot escape without explicit move,
- scope exit accounts for every acquired resource.

### 20.5 Structured task lifetime

Every ordinary child task terminates or is cancelled and joined before its lexical task scope exits. A task handle cannot outlive its scope token.

### 20.6 Data-race freedom

Under `Share`/`Send` derivation, task-local object ownership, affine transfer, and safe standard synchronization, two tasks cannot perform conflicting unsynchronized accesses to the same mutable location.

### 20.7 Pure determinism

A closed pure expression with deterministic primitive operations and identical explicit inputs evaluates to the same value or deterministic panic across scheduler/build profiles.

Allocation addresses, GC timing, and telemetry are not observable pure outputs.

### 20.8 Debug/release semantic equivalence

For a program not using explicitly profile-sensitive diagnostic constructs, debug and release builds have bisimilar observable outcomes:

- value/error/panic class,
- capability operation sequence subject to allowed observer events,
- resource/task lifetime,
- wire output.

Timing and performance are excluded.

### 20.9 Build determinism

Given the same normalized declared build inputs, the artifact bytes and semantic metadata digest are equal.

This is a toolchain theorem/verification property rather than a source-calculus theorem, but it is part of Mendrel’s production semantics.

## 21. Trusted computing base

The initial TCB includes:

- parser/AST-HIR lowering correctness,
- type/effect/resource/task checker,
- MIR lowering and verifier,
- code generator,
- runtime and GC,
- platform/FFI shims,
- cryptographic/hash/signature implementations used by artifacts,
- wire adapters,
- optional proof checker for proof-elided contracts.

Risk reduction:

- Rust implementation,
- generated single-source tables,
- independent MIR verifier,
- evaluator/backend differential tests,
- sanitizer and fuzzing,
- conformance suite,
- mechanized kernel,
- reproducible builds,
- small unsafe modules,
- external review.

LLVM and OS/runtime libraries remain external trusted dependencies for native execution; artifacts record their versions/digests.

## 22. Mechanization plan

### Layer A — K0

Prove:

- substitution,
- progress,
- preservation,
- match exhaustiveness correspondence,
- checked arithmetic totality modulo structured panic.

### Layer B — K1

Add:

- finite effect rows,
- capability availability,
- affine context threading,
- resource state machine,
- cleanup outcomes,
- contract subset.

Prove:

- capability confinement,
- no double consume/use-after-move,
- use-scope accounting,
- effect-row soundness.

### Layer C — K2

Add:

- scope tokens,
- task tree,
- Send/Share assumptions,
- spawn/join/cancel,
- deterministic scheduler trace.

Prove:

- handle non-escape,
- child lifetime,
- cancellation cleanup,
- race-freedom for modeled mutable heap.

### Compiler correspondence

Define a verified or property-tested translation relation:

\[
\text{Typed HIR} \sim K
\]

and:

\[
\text{MIR execution} \sim K
\]

A fully verified compiler is not a 1.0 requirement, but each lowering rule MUST have conformance/differential coverage tied to the formal rule ID.

## 23. Deliberate simplifications

Draft 0.1 makes these formal restrictions intentionally.

- branch affine states must join exactly
- no user-visible lifetime variables
- no general effect handler
- no exception stack
- no finalizer
- no detached task
- no inheritance/subtyping hierarchy
- no arbitrary reflection
- no weak-memory atomics in the initial kernel
- no distributed semantics
- no hard-real-time guarantee

Atomics, lock memory ordering, FFI memory, and optimized GC barriers require an extended memory model before standardization. Until then, only compiler-provided safe atomic APIs with documented semantics may exist, and their conformance is part of the runtime extension rather than assumed by this core.
