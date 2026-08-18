# 03. Runtime, memory, resources, and concurrency

## 1. Runtime philosophy

Mendrel runtime は、低レベル制御を全面的に source programmer へ押し付けず、production で必要な failure/lifetime/observability を隠さない。

中核 invariant:

1. safe code に未定義動作がない
2. safe code は data-race-free
3. ordinary heap value は tracing GC で管理される
4.決定的終了が必要なものは `resource`
5. child task は parent scope より長生きしない
6. cancellation/deadline は task tree と capability call に伝播する
7. panic/failure は task/supervisor boundary で構造化される
8. debug/release の意味は同じ
9. runtime event は trace/metric/log と関連付け可能
10. backend 差が source semantics を変えない

## 2. Heap model

### 2.1 Managed heap

ordinary record、enum、list、map、closure、immutable graph は managed heap に置ける。

source semantics は collector strategy に依存しない。runtime backend は次を満たす。

- precise root tracking
- moving collection を許す
- concurrent collection を許す
- generational collection を許す
- object address を safe code へ露出しない
- finalizer/resurrection を持たない
- weak reference を明示型にする
- allocation failure の policy を固定する
- pause と heap pressure を telemetry へ出す

MVP は単純な stop-the-world precise collector でよい。production profile は concurrent generational compacting collector を目標とする。

### 2.2 No finalizer

user-defined finalizer は禁止。

理由:

- 実行時刻が非決定
- cycle と resurrection
- cancellation/shutdown との競合
- GC thread での blocking
- agent/reviewer が lifetime を局所確認できない

外部資源は `resource` と `use` を使う。

### 2.3 Weak reference

```mendrel
let weak: Weak<Node> = Weak.from(node);
match weak.upgrade() {
    Some(strong) => use_node(strong),
    None => Unit,
}
```

weak collection は nondeterministic なので、pure function と deterministic test での利用を制限/注記する。cache API は weak semantics を型名へ出す。

### 2.4 Object identity

ordinary record/enum に観測可能な identity はない。reference equality は禁止。

identity が必要なら:

- stable domain ID を record field に持つ
- mutable identity は `object`
- external identity は `resource`
- graph algorithm の temporary identity は arena/index API

GC address を identity にしない。

## 3. Allocation and representation

compiler は escape analysis、scalar replacement、stack allocation、unboxing を行ってよい。ただし observable semantics は不変。

`mendrel explain allocation <symbol>` は次を返す。

- source allocation site
- lowered representation
- stack/heap/arena decision
- escape reason
- estimated size/alignment
- boxing reason
- generic specialization reason

production artifact は allocation profile map を optional section に持てる。

## 4. Resource model

### 4.1 Affine ownership

resource value は 0 回または 1 回 consume できる。正常構築された resource は scope 終了までに次のいずれかになる。

- explicit `close(move self)`
- 別 owner へ `move`
- `use` cleanup path により close/drop
- panic/cancel unwinding の cleanup stack により close/drop
- process abort のため cleanup 不可能

exactly-once ではなく affine（at most once）を型規則にし、`use` construct が normal/cancel/panic path の deterministic cleanup を保証する。

### 4.2 `use`

```mendrel
use transaction = database.begin().await?;
transaction.insert(order).await?;
transaction.commit().await?;
```

commit が resource を consume した後、implicit rollback は起きない。commit 前に scope を抜ければ cleanup policy が rollback/close を行う。

resource declaration は cleanup operation と failure policy を定義する。

```mendrel
resource Transaction {
    @cleanup
    async fn rollback(move self) -> Result<Unit, DbError>;

    async fn commit(move self) -> Result<Unit, DbError>;
}
```

cleanup failure は捨てない。

- 正常 return 中の cleanup failure は function result へ合成
- panic/cancel 中の cleanup failure は suppressed cause として report
- 複数 cleanup failure は ordered aggregate
- shutdown deadline 超過は supervisor event

### 4.3 Borrowed resource

`&Resource` は lexical call/region だけ。borrow は owner より長生きしない。borrowed handle を task spawn、heap closure、return、await 越しに保持できないのが既定。

zero-copy async API が必要な場合、owned buffer/resource segment を move する。

### 4.4 Resource state machine

resource は typestate を optional nominal type で表せる。

```mendrel
resource Socket<State> { ... }

opaque type Connected;
opaque type Closed;
```

state transition は resource consume と new resource return。

```mendrel
fn connect(move socket: Socket<Unconnected>)
    -> Result<Socket<Connected>, ConnectError>;
```

arbitrary type-level state computation は避け、finite nominal state に限定する。

## 5. Mutation and isolation

### 5.1 Local mutation

`object` と mutable collection は task-local。compiler は task ID を runtime field に入れる必要はなく、static escape analysis で禁止する。

### 5.2 Owned graph

大量データを copy せず task へ移すため `Owned<T>` を使う。

```mendrel
let payload: Owned<MutableBuffer> = buffer.freeze_ownership()?;
let task = tasks.spawn move payload {
    process(move payload)
};
```

`freeze_ownership` の検査:

- graph 内に外部 mutable alias がない
- non-send resource/capability を含まない
- borrowed reference を含まない
- weak backedge policy が明確
- object graph の root ownership が一意

成功後、元 alias は利用不能。移動先で再び local mutation できる。

### 5.3 Shared mutable state

共有 mutable state は標準では次の優先順位。

1. immutable snapshot と message passing
2. actor
3. atomic primitive
4. `Mutex<T>`/`RwLock<T>` resource guard
5. unsafe/FFI

lock guard は affine、not `SuspendSafe`。guard を `await` 越しに保持すると compile error。

lock ordering metadata を optional に持ち、deadlock lint/model test へ利用する。

## 6. Structured concurrency

### 6.1 Task scope

```mendrel
scope tasks {
    let inventory_task = tasks.spawn {
        inventory.reserve(items).await
    };

    let payment_task = tasks.spawn {
        payments.authorize(total).await
    };

    let inventory = inventory_task.await?;
    let payment = payment_task.await?;
    Ok(Pair(inventory, payment))
}
```

scope exit invariant:

- child は全て completed / joined / cancelled and joined
- child resource cleanup 完了
- child panic/failure は join policy に従い parent result/report へ集約
- child trace span は parent へ関連付く
- child は scope 外から参照できない

task handle は scope-indexed。source programmer に lifetime parameter を書かせず、compiler-owned region ID で検査する。

### 6.2 Join policy

scope declaration は policy を明示できる。

```mendrel
scope tasks policy fail_fast {
    // first error cancels siblings
}

scope tasks policy collect_all {
    // returns ordered aggregate
}

scope tasks policy supervise {
    // child failure is event; restart policy required
}
```

default は `fail_fast`。policy がない implicit detached behavior はない。

### 6.3 Cancellation

cancellation は cooperative だが、runtime/capability API は cancellation point を規定する。

cancellation point:

- `await`
- channel send/receive
- sleep/timer
- blocking capability operation
- explicit `cancel.check()`
- allocation safepoint は implementation detailであり、source-level cancellation point にしない

cancellation は `Cancelled` cause を持ち、deadline、parent failure、manual cancel、shutdown を区別する。

catch-and-ignore は可能でも lint 対象。task scope を退出する前に child cancellation を join する。

### 6.4 Deadline

task context は optional absolute monotonic deadline を持つ。

```mendrel
within 500.milliseconds {
    http.get(url).await
}
```

規則:

- nested deadline は親と指定値の最小
- outbound network/database call は finite deadline を要求する production lint
- timeout は operation-level error と cancellation cause を区別
- retry は残り deadline budget を消費
- wall-clock change の影響を受けない
- deadline は trace attribute として伝播する

### 6.5 No detached task

次は存在しない。

```text
spawn_detached(...)
fire_and_forget(...)
global_executor.submit(...)
```

長寿命処理は `Supervisor` capability を通す。

## 7. Supervisor and services

### 7.1 Supervisor

```mendrel
pub capability Supervisor {
    async fn start<Service, E>(
        spec: ServiceSpec<Service, E>,
    ) -> Result<ServiceHandle, SupervisorError>
    where {
        Service: Send,
        E: ServiceError,
    };
}
```

`ServiceSpec` に必要:

- stable service name
- start function
- restart policy
- max restart intensity
- shutdown deadline
- dependency order
- health/readiness probes
- resource budget
- telemetry classification

restart policy:

- never
- on_transient
- always
- exponential_backoff with jitter capability
- circuit_break after threshold

jitter/randomness は supervisor runtime capability によって注入され、deterministic test で seed 固定できる。

### 7.2 Service lifecycle

runtime は次を構造化する。

1. config validation
2. dependency acquisition
3. startup
4. readiness publish
5. request acceptance
6. drain signal
7. child cancellation
8. resource cleanup
9. termination report

signal handling を各 application が ad-hoc に書かない。service declaration と supervisor が platform adapter へ lowering する。

## 8. Actor standard abstraction

actor は長寿命 mutable state のための **stdlib abstraction** であり、v1 core syntax に専用 `actor` declaration は置かない。専用構文を先に固定すると、mailbox、reply、reentrancy、persistence、supervision の policy を言語核へ抱え込み、task abstraction と重複するからや。

```mendrel
pub record AddItem {
    item: Item,
}

pub enum CartMessage {
    Add {
        request: AddItem,
        reply: ReplyPort<Result<CartSnapshot, CartError>>,
    },
}

pub async fn handle_cart(
    state: &mut Cart,
    message: CartMessage,
) -> Result<Unit, CartError> {
    // serialized access inside the actor task
}

let cart = Actor.spawn::<Cart, CartMessage>(
    initial: Cart.empty(),
    capacity: 256,
    handler: handle_cart,
);
```

標準 `Actor<State, Message>` が保証する。

- mailbox message は `Send`
- state は actor task 内
- one message at a time が既定
- blocking/await 中の reentrancy policy は constructor policy で固定
- mailbox capacity/backpressure は必須
- supervision と persistence は capability/library policy
- remote actor location transparency は v1 にない

actor-only language にはしない。request-local parallelism は structured task の方が自然や。専用構文は、複数の production corpus で boilerplate と誤用が反復し、通常 API・derive・generator では解けないと実証された場合だけ edition RFC で再検討する。

## 9. Channel and backpressure

channel は capacity を必須にする。

```mendrel
let channel = Channel.bounded::<Event>(capacity: 256);
```

unbounded channel は `UnboundedMemory` capability/unsafe policy を要求する。

send policy:

- wait
- reject
- drop_newest
- drop_oldest
- sample

drop policy は型/constructor で明示し、metrics を自動計測。silent loss は禁止。

stream API は cancellation、backpressure、close cause を持つ。producer/consumer task lifetime は scope に結び付く。

## 10. Deterministic concurrency testing

test runtime は同じ task semantics を使い、scheduler selection だけを制御する。

features:

- virtual monotonic clock
- deterministic random
- seeded runnable-task order
- recorded schedule
- replay token
- bounded schedule exploration
- cancellation injection
- network/database fake delay/failure injection
- deadlock/livelock detection
- resource leak check
- race-sensitive assertion

```mendrel
@test(schedule: explore(max_steps: 200, seeds: 1000))
async fn reservation_is_atomic() uses {
    inventory: FakeInventory,
    clock: VirtualClock,
} {
    // ...
}
```

failure report は seed だけでなく、scheduler decision trace と capability fake event log を持つ。

## 11. Blocking operations

async runtime thread を blocking してはならない。

- filesystem/network/database は async capability
- CPU-heavy work は `CpuPool` capability と budget
- legacy blocking FFI は `BlockingPool` capability
- blocking annotation を compiler/runtime metadata に持つ
- async task から blocking call すると error/lint
- pool saturation と queue time を telemetry へ出す

## 12. Panic and failure propagation

task completion は内部的に次の sum type。

```text
Completed(T)
Failed(E)
Cancelled(CancelCause)
Panicked(PanicReport)
Aborted(RuntimeAbort)
```

application が直接 `RuntimeAbort` を recover できない場合も、supervisor/artifact report は区別する。

`PanicReport`:

- stable panic kind
- source span/symbol
- task tree path
- trace/span IDs
- capability operation in flight
- cleanup failures
- redacted stack
- compiler/runtime build IDs
- source/artifact fingerprint

## 13. GC and concurrency interaction

- safepoint protocol は runtime internal
- task cancellation と GC safepoint を同一視しない
- resource cleanup は GC callback で走らない
- moving GC 時の FFI pointer は `Pinned<T>` resource
- stack map は codegen single source から生成
- concurrent collector write barrier は compiler/runtime ABI に versioned contract
- actor/task isolation metadata は optimizer が利用できるが、soundness は optimizer に依存しない

## 14. Collector roadmap

### Bootstrap

- precise stop-the-world mark/sweep または mark/compact
- single heap
- deterministic stress mode
- root map verification
- heap verifier
- no finalizer

### Production v1 target

- generational
- concurrent marking
- optional compaction
- per-thread/task allocation buffer
- pause budget telemetry
- heap size policy
- container memory limit awareness
- snapshot/crash inspection support

### Future backend

MMTk 等の toolkit を adapter 経由で評価できる。ただし toolkit の production readiness を言語リリースの前提にしない。collector-specific API を source language へ漏らさない。

## 15. FFI

### 15.1 C FFI

FFI は `unsafe module` 内だけ。

```mendrel
unsafe module ffi.zlib
reason "Audited zlib C ABI"
{
    extern "C" fn compress(...);
}
```

binding は header/schema から sandbox generator で生成するのを推奨。

規則:

- raw pointer、union、variadic、callback は unsafe
- ownership transfer を binding metadata へ明記
- nullable pointer は `Option<NonNull<T>>`
- buffer は pointer+length を typed slice/view に変換
- callback lifetime を explicit resource token にする
- foreign thread callback は runtime attach API
- foreign exception/longjmp を境界外へ出さない
- ASan/UBSan-compatible build profile を用意
- native debug symbol と source map を保持
- unsafe wrapper は safe contract test/fuzz を持つ

### 15.2 Stable component boundary

stable plugin/service ABI は WebAssembly Component Model/WIT を優先する。

- import/export contract
- records/variants/options/results/resources
- canonical ABI
- sandboxed authority
- versioned world/interface
- language-neutral binding

native ABI stability は約束しない。process-local performance が必要なら same-version plugin と artifact fingerprint matching を要求する。

## 16. Debug/release parity

禁止例:

- release だけ integer overflow wrap
- release だけ bounds check を根拠なく除去
- debug だけ contract/assert が有効
- release だけ data-race/alias UB
- unordered iteration が optimizer により意味変更
- panic strategy が error result を変える

最適化により check を消すには、compiler が同一 semantics を証明する。`debug_assert` は名前で release omission を示す例外。

## 17. Runtime limits and quotas

service/task scope は budget を持てる。

- memory
- CPU time
- wall/deadline
- task count
- open resource count
- channel capacity
- outbound concurrency
- log/trace volume

budget violation は structured cause。resource exhaustion を OOM/panic 一択にせず、可能な boundary では typed `LimitExceeded` にする。

## 18. Runtime non-goals v1

- hard real-time guarantee
- lock-free everything
- transparent distributed shared memory
- location-transparent actor
- arbitrary stack capture/continuation
- user finalizer
- object resurrection
- green-thread stack introspection API
- hot code replacement with state migration
- stable native ABI across compiler versions
- manual allocator selection per ordinary type
- general unsafe pointer arithmetic in application code
