# 00 — Non-negotiable invariants

These rules outrank every consideration of simplicity or speed. If code appears to require
violating one of them, the code is wrong — not the invariant.

All requirements in this file use `MUST` / `MUST NOT` by construction: an invariant that could
be downgraded to `SHOULD` would not be an invariant. See `spec/CONVENTIONS.md` for the ID
scheme, EARS patterns, and the requirement → issue → PR workflow.

---

#### INV-001 — Durable / best-effort boundary

- **Statement:** The engine MUST classify every mechanism as either durable (transactional,
  Postgres-backed, source of truth — e.g. execution state, audit, leases, timers, versions) or
  best-effort (transient, recoverable — e.g. NATS dispatch, application logs, notifications,
  real-time progress). No mechanism MAY straddle both categories.
- **Rationale:** A fact MUST be born durable and only afterwards be derived into the
  best-effort layer, never the reverse — otherwise a lost best-effort message becomes a lost
  fact.
- **Verification:** Inspection (design review of each new mechanism against this
  classification before merge).

#### INV-002 — Postgres is the only source of truth

- **Statement:** No component MUST read NATS to determine what is true. NATS MUST carry only
  the notification of a fact that is already durable. The reconciler MUST read Postgres, never
  the stream.
- **Rationale:** A message bus without persistence and ordering guarantees strong enough to be
  a source of truth would silently reintroduce split-brain state.
- **Verification:** Inspection (code review: no component branches logic on NATS message
  content as ground truth).
- **Depends on:** INV-001

#### INV-003 — Durability at node boundaries

- **Statement:** After each node, the engine MUST persist state transactionally (outputs, new
  frontier, status) and only then acknowledge the NATS message. When the engine crashes, it
  MUST reload state from Postgres and re-enqueue from the persisted frontier. The engine MUST
  NOT expose a consistent state in the middle of a node's execution.
- **Rationale:** The node boundary is the only interruption/resume point the engine can
  reason about safely; state consistency cannot be guaranteed mid-node.
- **Verification:** Test (crash-injection test: kill the worker mid-node, assert resume
  reloads persisted state without corrupting the frontier).

#### INV-004 — Mandatory internal idempotence

- **Statement:** The engine MUST identify every node-run by the stable key `(execution_id,
  node_id, iteration_index, attempt)` and MUST implement resume as an upsert conditioned on
  that key. Pipeline authors MUST NOT be relied upon to implement this themselves.
- **Rationale:** This is a database upsert, not distributed coordination — pushing the
  responsibility to pipeline authors would make correctness optional.
- **Verification:** Test (replaying the same node-run key twice MUST NOT produce two
  committed outcomes).

#### INV-005 — At-least-once delivery with a reception guard

- **Statement:** When a worker receives a node-run message, it MUST first check whether
  `(execution_id, node_id, iteration_index, attempt)` is already in a terminal state; if so,
  it MUST acknowledge and discard the message without re-executing. When a provider supports
  an idempotency key, the engine MUST propagate a stable key derived from the node-run ID to
  that provider as a best-effort measure against duplicate external side effects.
- **Rationale:** This reception guard — not the reconciler — is what makes at-least-once
  delivery safe; the reconciler only handles orphaned leases, not ordinary redelivery.
- **Verification:** Test (deliver the same message twice to a worker; assert exactly one
  execution and one ack for the duplicate).
- **Depends on:** INV-004

#### INV-006 — Closed set of control primitives

- **Statement:** The core MUST own a closed set of control primitives — `Task`, `Choice`,
  `Wait`, `Parallel`, `Map`, `Pass`, `Succeed`, `Fail`, plus the modifiers `Retry`, `Catch`,
  `Timeout`. Plugins MUST NOT introduce a new control primitive; a plugin MUST only add Tasks
  (leaf nodes) or macros that compile down to this closed set.
- **Rationale:** The engine must understand the execution semantics of every node type to
  guarantee durability and verifiability; an open-ended primitive set would break that
  guarantee.
- **Verification:** Inspection (schema/compiler review: the IR's state-type enum MUST reject
  any type outside this set; a plugin manifest MUST NOT be able to register a new one).

#### INV-007 — Secret is a taint-tracked type

- **Statement:** Secret material and fields annotated `sensitive: true` MUST NOT circulate in
  a loggable, plain-text form anywhere in the engine. `Secret<T>` MUST render `[REDACTED]` on
  `Display`/`Debug`/`toString`, MUST be unwrappable only by the host's I/O capability, and its
  taint MUST propagate to derived values.
- **Rationale:** This is detailed in `spec/04-secrets-security.md`. An output-scanning regex
  MAY exist as defense-in-depth, but MUST NOT be the primary mechanism.
- **Verification:** Test (attempt to log/serialize a `Secret<T>` and a value derived from it;
  assert `[REDACTED]` in both cases).

#### INV-008 — Guaranteed termination

- **Statement:** Every execution MUST reach a terminal state within a bounded time, enforced by
  three independent bounds: ack-wait/lease expiry (dead worker), per-node timeout (runaway
  node), and execution TTL (blocked workflow, generous default, overridable per pipeline).
  There MUST NOT be a code path that leads to permanent blocking.
- **Rationale:** Each bound catches a failure mode the other two cannot see; together they are
  the termination backstop of last resort.
- **Verification:** Analysis (liveness argument covering all three bounds) + Test (inject each
  failure mode independently and assert termination within its bound).

#### INV-009 — No bypass

- **Statement:** Every interface — GUI, CLI, MCP, third-party editors — MUST be a consumer of
  the control API. No interface MUST hold a capability the API does not expose, or a code path
  that escapes RBAC, tenant scope, RLS, or audit.
- **Rationale:** Hardening the API hardens every client for free; any interface-specific
  shortcut becomes a permanent, hard-to-audit exception.
- **Verification:** Inspection (for each new interface capability, confirm it is implemented
  by calling the control API, not by a private code path).

#### INV-010 — Host-mediated egress

- **Statement:** A node MUST NOT perform direct I/O. It MUST only describe the intended call;
  the host MUST inject the credential outside the sandbox; egress MUST be allowlisted
  per-credential (`connections.egress_scope`). The WASM module MUST NOT ever observe the
  secret.
- **Rationale:** This closes both exfiltration (a malicious node cannot redirect the credential
  to an attacker-controlled destination) and SSRF (the host refuses to attach the credential
  outside its declared scope) by construction.
- **Verification:** Test (node declares a call to a destination outside `egress_scope`; assert
  the host refuses to attach the credential).

---

## Corollary: cancellation is never confused with compensation

"Cancel" stops; it does not undo what has already happened. Refunding or reversing an
already-emitted side effect is business logic (a saga, a `Catch` handler) composed by the
pipeline author — never engine magic. The engine stops cleanly and exposes what it knows about
the resulting state (`interrupted` when the side effect's outcome is uncertain).

#### INV-011 — No automatic compensation

- **Statement:** The engine MUST NOT automatically compensate, refund, or roll back a side
  effect that a node has already emitted. Compensation logic, when needed, MUST be authored
  explicitly by the pipeline (e.g. a saga pattern or a `Catch` handler).
- **Rationale:** The engine cannot know the business meaning of "undo" for an arbitrary
  external effect; pretending otherwise would produce silently wrong compensations.
- **Verification:** Inspection (design review: no engine-level code path issues a compensating
  call on cancellation/kill without pipeline-authored logic).
