# 06 — Durability, lifecycle, Map, cancellation, HITL

See `spec/CONVENTIONS.md` for the requirement format and the `LIFE-NNN` ID scheme.

---

## Base durability & leases

#### LIFE-001 — Persist-then-ack; resume never re-runs

- **Statement:** After each node, the engine MUST persist state transactionally (outputs, new
  frontier, status) and only then acknowledge the NATS message. On resume after a crash, the
  engine MUST reload persisted outputs and MUST NOT re-execute a node whose side effects were
  already committed.
- **Rationale:** This is `INV-003` and `INV-004` applied concretely: resume is a reload, never
  a replay, of already-committed work.
- **Verification:** Test (crash after persisting a node's output but before ack; assert resume
  reloads the persisted output instead of re-executing the node).
- **Depends on:** INV-003, INV-004

#### LIFE-002 — `running` is a lease, not a status

- **Statement:** Every dispatched node-run MUST carry `leased_until` and `owned_by_worker`.
- **Rationale:** Without a lease, the engine cannot distinguish "running legitimately" from
  "orphaned by a dead worker".
- **Verification:** Inspection (schema: `leased_until` and `owned_by_worker` are set whenever
  `status = 'running'`).
- **Depends on:** DAT-002

#### LIFE-003 — Reconciler reads Postgres exclusively

- **Statement:** The reconciler MUST read only from Postgres — never from NATS — to find
  node-runs whose `status = 'running'` and `leased_until < now()`.
- **Rationale:** This is `INV-002` applied to the one component whose entire job is deciding
  what is stuck.
- **Verification:** Inspection (reconciler code contains no NATS consumer for determining
  orphan state).
- **Depends on:** INV-002

#### LIFE-004 — Lease reclamation uses compare-and-swap

- **Statement:** When the reconciler reclaims an expired lease, it MUST use a compare-and-swap
  update (`UPDATE node_runs SET status='pending', attempt=attempt+1, leased_until=NULL WHERE
  id=? AND leased_until=<value read>`) so that a losing concurrent reclaimer observes zero
  rows updated and steps aside. Only after the CAS succeeds MUST the engine re-dispatch via
  NATS.
- **Rationale:** The database decides who wins; NATS only transports the resulting dispatch —
  this prevents two reconciler instances from double-dispatching the same node-run.
- **Verification:** Test (run two concurrent reconciler passes over the same expired lease;
  assert exactly one re-dispatch occurs).
- **Depends on:** LIFE-003, INV-004

#### LIFE-005 — One reconciler sweep serves three purposes

- **Statement:** The reconciler MUST be a single periodic sweep (triggered by the decoupled
  scheduler) that handles orphaned leases, missed fan-in completion, and exceeded TTLs as three
  queries of one daemon, not three separate daemons.
- **Rationale:** Ack-wait/JetStream redelivery is the fast, message-local safety net;
  the DB reconciler is the slow, state-global safety net — they are complementary, and
  consolidating the state-global sweep into one daemon avoids duplicated polling logic.
- **Verification:** Inspection (single reconciler component implements all three queries).
- **Depends on:** LIFE-004, LIFE-012, LIFE-008

#### LIFE-006 — Heartbeat is optional; the schema supports it either way

- **Statement:** Without a heartbeat, `leased_until` MUST be fixed at dispatch time (bounding
  node duration by the ack-wait). Where a heartbeat is implemented, `leased_until` MUST slide
  forward while the worker heartbeats, allowing long-running nodes. `leased_until` MUST exist
  in the schema regardless of which mode is active, so enabling heartbeat later is a behavior
  addition, not a migration.
- **Rationale:** Ships a simple, viable v1 (fixed lease) without foreclosing a later upgrade to
  sliding leases for long-running nodes.
- **Verification:** Inspection (schema has no heartbeat-specific columns beyond
  `leased_until`; heartbeat logic, if present, only extends `leased_until`).
- **Depends on:** DAT-002, LIFE-002

## Termination bounds

#### LIFE-007 — Three independent termination bounds

- **Statement:** The engine MUST enforce three independent bounds: ack-wait/lease expiry
  (protects against a dead worker), per-node timeout (protects against a runaway node), and
  execution TTL (protects against a blocked workflow, comparing total duration against
  `executions.deadline`).
- **Rationale:** This is `INV-008` made concrete; each bound catches a distinct failure mode
  invisible to the other two.
- **Verification:** Test (trigger each bound independently — dead worker, runaway node, blocked
  workflow — and assert termination in each case).
- **Depends on:** INV-008, DAT-005

#### LIFE-008 — Exceeded TTL is a kill with `motif=ttl_exceeded`

- **Statement:** When an execution's TTL is exceeded, the engine MUST treat it as a kill,
  inheriting all kill semantics defined in `LIFE-016`–`LIFE-017`, with `motif = 'ttl_exceeded'`.
- **Rationale:** Reuses the already-audited, RBAC-gated kill semantics instead of introducing a
  second termination code path.
- **Verification:** Test (an execution exceeding its TTL transitions through the same audit and
  state-machine path as an operator kill, with `motif='ttl_exceeded'`).
- **Depends on:** LIFE-007, LIFE-016, LIFE-017

## Map — durable fan-out

#### LIFE-009 — One durable node-run per Map item

- **Statement:** A `Map` state MUST create one durable node-run per item, keyed by
  `iteration_index`, enabling per-item retry (item N replayed alone via `attempt`) and
  per-item resume.
- **Rationale:** This is what makes Map durable at item granularity instead of all-or-nothing.
- **Verification:** Test (fail item N in a Map of size M; assert only item N is retried, not
  the whole Map).
- **Depends on:** DAT-002

#### LIFE-010 — Map concurrency is bounded by a semaphore enforced at dispatch

- **Statement:** `Map` concurrency MUST be bounded by a semaphore (`MaxConcurrency`) enforced
  at dispatch time, with a timeout applied per item.
- **Rationale:** Bounds resource usage from a single Map state regardless of collection size.
- **Verification:** Test (a Map with `MaxConcurrency=N` never has more than N items
  simultaneously dispatched).
- **Depends on:** LIFE-009

#### LIFE-011 — Map v1 has an explicit item-count bound

- **Statement:** The engine MUST define an explicit v1 bound on the number of durable items a
  single `Map` state supports. Pagination of the source collection beyond that bound MUST be
  delegated to the pipeline author via a paginated sub-pipeline; distributed batch Map is
  reserved for a future version and MUST NOT be assumed to exist in v1.
- **Rationale:** An unbounded Map would create unbounded per-item leases/rows; making the
  boundary explicit avoids silent degradation at scale.
- **Verification:** Inspection (the bound is documented and enforced at compile/dispatch time,
  rejecting a Map declared beyond it).
- **Depends on:** LIFE-009

#### LIFE-012 — Fan-in completion is decided by CAS on `map_state.fan_in_status`

- **Statement:** For `Map` and `Parallel`, the engine MUST decide fan-in completion via an
  idempotent compare-and-swap on `map_state.fan_in_status` (`pending -> aggregating`): the
  winner aggregates, all others abstain. The push path (the last item to complete attempts the
  CAS immediately) and the pull path (the reconciler catches a Map whose items are all
  terminal but whose fan-in CAS never fired) MUST both exist.
- **Rationale:** No participant can locally know it is "the last one"; the CAS is the one
  irreducible synchronization point, and the push/pull duo mirrors the fast/slow safety-net
  pattern used for orphaned leases.
- **Verification:** Test (concurrent completions racing for fan-in; assert exactly one
  aggregation) + Test (force all items terminal without the push CAS firing; assert the
  reconciler's pull path completes the fan-in).
- **Depends on:** DAT-006, LIFE-005

#### LIFE-013 — Default Parallel failure policy is "collect"

- **Statement:** By default, `Parallel` MUST wait for all branches to reach a terminal state
  before raising an error that names the failing branches (enabling targeted replay).
  Fail-fast MUST be available only as an explicit opt-in field per `Parallel` state.
- **Rationale:** Fail-fast-by-default would cancel in-flight branches and leave their side
  effects half-done; "collect" is the durability-consistent default.
- **Verification:** Test (a `Parallel` with one failing branch and others still running waits
  for all branches before surfacing the named failure, unless fail-fast is explicitly set).
- **Depends on:** LIFE-012

## Cancellation & kill

#### LIFE-014 — Cooperative cancellation stops at safe node boundaries

- **Statement:** By default, an "interruptible" stop MUST occur at safe node boundaries: the
  engine signals cancellation, and the node finishes its current I/O or reaches its declared
  safe point. A node MAY declare a section as non-interruptible-in-flight, overriding the
  default.
- **Rationale:** Stopping mid-I/O by default would create more uncertain-outcome side effects
  than necessary; safe-boundary stopping is the conservative default.
- **Verification:** Test (issue a cooperative cancellation during a node's declared
  non-interruptible section; assert the node completes that section before stopping).

#### LIFE-015 — Hard interruption uses epoch-interruption and yields `interrupted`

- **Statement:** A hard interruption (operator kill or TTL) MUST use epoch-interruption to
  terminate the module immediately. When the node's I/O may already have been dispatched, the
  engine MUST set the terminal state to `interrupted` (side effect uncertain), never
  `cancelled`.
- **Rationale:** The engine must expose uncertainty about a possibly-completed external effect
  rather than mask it behind a state implying clean cancellation.
- **Verification:** Test (hard-kill a node mid-I/O; assert the resulting state is `interrupted`,
  not `cancelled`).
- **Depends on:** PROTO-006, DAT-003

#### LIFE-016 — Operator kill is audited, RBAC-gated, and terminal-non-retryable

- **Statement:** An operator kill MUST be audited as a first-class, high-consequence action
  (who, when, which execution, mandatory `motif`), committed atomically with the action; MUST
  require the dedicated `execution:kill` RBAC permission (distinct from launch/edit); and MUST
  produce a terminal, non-retryable state (`killed`, distinct from `failed`).
- **Rationale:** Kill is the highest-consequence operator action in the system and must carry
  correspondingly strict controls.
- **Verification:** Test (attempt a kill without `execution:kill` permission; assert denial) +
  Inspection (kill path writes an `audit_log` entry with mandatory `motif` in the same
  transaction as the state change).
- **Depends on:** DAT-003, DAT-008, OBS-003, MCP-004

#### LIFE-017 — Kill is hierarchical via scoped NATS subject

- **Statement:** The cancellation signal MUST support hierarchical scope via a NATS subject
  shaped `cancel.{tenant}.{pipeline}.{execution}`, so that killing an entire pipeline or an
  entire tenant is achieved by publishing on the parent prefix. The signal's source of truth
  MUST be the database, with NATS used only for latency, mirroring the audit pattern.
- **Rationale:** Hierarchical kill (pipeline-wide, tenant-wide circuit breaker) falls out for
  free from a well-scoped subject hierarchy, without a bespoke bulk-kill code path.
- **Verification:** Test (publish a kill on a pipeline-level prefix; assert every execution
  under that pipeline receives the kill signal).
- **Depends on:** LIFE-016, INV-002

#### LIFE-018 — No automatic compensation on cancellation or kill

- **Statement:** The engine MUST NOT automatically compensate or roll back a side effect that
  has already been emitted when cancelling or killing an execution.
- **Rationale:** Restates `INV-011` in the cancellation/kill context, since it is the place this
  temptation is strongest.
- **Verification:** Inspection (kill/cancel code path contains no compensating-call logic).
- **Depends on:** INV-011

## Human-in-the-loop (`Wait-on-signal`)

#### LIFE-019 — HITL authorization is an HMAC-signed, presigned-style token

- **Statement:** A `Wait-on-signal` state's unlock authorization MUST be an HMAC-signed token
  over `(execution_id, node_id, iteration_index, nonce, expiry, key_id)`, verified by the
  engine before acting. Possession of a valid signature MUST grant the right to unblock that
  specific wait and MUST NOT be transferable to another execution.
- **Rationale:** Modeled on S3 presigned URLs: an unforgeable capability rather than a
  database-checked permission list.
- **Verification:** Test (a signature computed for execution A is rejected when presented
  against execution B's wait).
- **Depends on:** DAT-010

#### LIFE-020 — Signal consumption is idempotent, single-use

- **Statement:** Signal consumption MUST use the same idempotent CAS pattern as fan-in
  (`waiting -> signaled`): the first valid call wins, every subsequent call MUST be a no-op.
- **Rationale:** Reuses the proven CAS pattern from `LIFE-012` instead of a new
  single-use-token mechanism.
- **Verification:** Test (present the same valid signal twice; assert only the first call has
  an effect).
- **Depends on:** LIFE-012, LIFE-019

#### LIFE-021 — Signal deadline expiry routes to `Catch`

- **Statement:** `wait_deadline` MUST be checked by the reconciler; when exceeded, the engine
  MUST route execution to the state's `Catch` branch as a timeout.
- **Rationale:** Reuses the existing reconciler sweep and the existing `Catch` error-routing
  mechanism instead of a bespoke HITL timeout path.
- **Verification:** Test (a signal wait past its `wait_deadline` is caught by the reconciler
  and routed to `Catch`).
- **Depends on:** LIFE-005, DAT-010, EXE-004

#### LIFE-022 — Zero secret storage; signature is recomputed

- **Statement:** `signals` MUST NOT store the token. The signature MUST be recomputable from
  persisted state `(execution_id, node_id, iteration_index, wait_deadline)` plus the signing
  key, server-side, the same way S3 recomputes presigned-URL signatures.
- **Rationale:** Eliminates an entire class of token-storage leak risk by never storing the
  thing that would need protecting.
- **Verification:** Inspection (schema: no token column exists in `signals`; verification code
  recomputes the expected signature rather than comparing against a stored one).
- **Depends on:** DAT-010, LIFE-019

#### LIFE-023 — Signing key is rotatable via `key_id`

- **Statement:** The HITL signing key MUST be rotatable via a `key_id` selector (itself
  unsigned), with the N most recent keys kept active for verification.
- **Rationale:** Allows key rotation without invalidating in-flight signed tokens issued under
  a previous key.
- **Verification:** Test (rotate the active signing key; assert a token signed under the
  previous key, within the retained window, still verifies).
- **Depends on:** LIFE-019, DAT-010

#### LIFE-024 — V1 is single-signal, single-waiter

- **Statement:** In v1, a `Wait-on-signal` state MUST support exactly one signal unblocking
  exactly one waiter. Quorum semantics (multiple signals with a threshold) MUST be reserved for
  a future `signal_group` construct and MUST NOT be implemented in v1.
- **Rationale:** Keeps v1 scope bounded; quorum adds lease-per-signal and threshold-CAS
  complexity that is not yet justified.
- **Verification:** Inspection (schema/state definition has no `signal_group` construct wired
  in v1).

#### LIFE-025 — HITL payload passes through field-shaping

- **Statement:** The payload delivered by a signal MUST pass through the same field-shaping
  mechanism as any other state input (`EXE-005`), since a human unblocking a wait is injecting
  data, not just sending a ping.
- **Rationale:** Keeps HITL data ingestion consistent with the rest of the state input/output
  model instead of being a special case.
- **Verification:** Test (a signal payload is shaped via `InputPath`/`Parameters` equivalents
  before being made available to subsequent states).
- **Depends on:** EXE-005

## Webhook ingestion

#### LIFE-026 — Persist before acking the caller

- **Statement:** Webhook ingestion MUST persist the incoming trigger durably before responding
  `200` to the caller. The order MUST be: receive, write durably, respond `200`,
  correlate/trigger asynchronously.
- **Rationale:** Acking before persisting risks losing a trigger if the process crashes between
  the ack and the write.
- **Verification:** Test (crash the ingestion process after receipt but before the durable
  write; assert no `200` was sent and the caller's retry is not lost).
- **Depends on:** INV-001, INV-003

#### LIFE-027 — Webhook deduplication

- **Statement:** Webhook ingestion MUST deduplicate using the provider's idempotency key when
  available, falling back to a hash of `(payload, source)`, and MUST debounce repeated
  deliveries.
- **Rationale:** Providers commonly redeliver webhooks; deduplication prevents the same
  external event from triggering multiple executions.
- **Verification:** Test (deliver the same webhook payload twice; assert only one execution is
  triggered).
- **Depends on:** LIFE-026

## Replay

#### LIFE-028 — Replay creates a new linked run, never mutates history

- **Statement:** A replay MUST create a new run linked via `replay_of`. It MUST NOT mutate the
  original run. The engine MUST support three replay granularities: full execution, from a
  given node (reloading upstream outputs), and failed items only.
- **Rationale:** Keeps history immutable, consistent with the append-only audit log, and
  matches proven prior art (GitLab-style replay).
- **Verification:** Test (replay an execution at each of the three granularities; assert the
  original run's rows are unchanged and a new run with `replay_of` set is created).
- **Depends on:** DAT-005, DAT-008

## Scheduler

#### LIFE-029 — Scheduler is a decoupled, stateless-trigger component

- **Statement:** The scheduler MUST run as a component separate from the control plane's
  business logic (not embedded in a Docker cron or a Kubernetes CronJob). The external trigger
  MUST be dumb and stateless: it emits a deterministic JetStream event
  (`schedule_id + window`, never a random UUID) and then exits; the control plane owns the
  logic and deduplication (via `Nats-Msg-Id`). On scheduler scale-out, leader election MUST run
  at startup and claims MUST be made atomically in the database.
- **Rationale:** A deterministic event key makes redelivery/duplication safe to deduplicate;
  keeping the trigger stateless keeps the scheduler itself easy to scale and reason about.
- **Verification:** Test (fire the same schedule window twice from two scheduler replicas;
  assert exactly one triggered execution).
- **Depends on:** INV-002, DAT-005
