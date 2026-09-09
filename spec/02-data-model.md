# 02 — Data model (schema v1)

See `spec/CONVENTIONS.md` for the requirement format and the `DAT-NNN` ID scheme.

> **Columns marked 🔒 are RESERVED in v1.** They live in the primary key, the column type, or an
> enum. Adding them later means a painful migration or reinterpreting history. Never omit them
> to move faster.

Multi-tenancy is enforced by two complementary mechanisms: `tenant_id` in every composite
foreign key (integrity), and Postgres RLS with session `SET LOCAL app.tenant_id` and
deny-by-default policies (access — an unfiltered `SELECT` must not leak cross-tenant rows).

---

#### DAT-001 — Composite tenant FK and RLS on every tenant-scoped table

- **Statement:** Every table holding tenant-scoped data MUST include `tenant_id` in its
  composite foreign keys, and Postgres Row-Level Security MUST enforce deny-by-default access
  scoped to `SET LOCAL app.tenant_id`.
- **Rationale:** The FK protects referential integrity across tenants; RLS protects access —
  together they ensure an unfiltered query cannot leak another tenant's rows.
- **Verification:** Test (RLS policy test: query without setting `app.tenant_id`, or with a
  different tenant's ID, returns zero rows for another tenant's data).

#### DAT-002 — `node_runs` reserved columns

- **Statement:** The `node_runs` table MUST have primary key `(execution_id, node_id,
  iteration_index, attempt)` and MUST include the reserved columns 🔒 `tenant_id`, `status`,
  `leased_until`, `owned_by_worker`, plus `input_ref`, `output_ref`, `created_at`, `updated_at`.
  `iteration_index` and `attempt`🔒 are part of the primary key.
- **Rationale:** `iteration_index` is what makes durable Map possible (one durable node-run per
  item); `leased_until` is the lease that distinguishes "running" from "orphaned"; `running` is
  not a status, it is a lease state.
- **Verification:** Inspection (schema migration defines exactly this primary key and these
  columns).
- **Depends on:** INV-003, INV-004

#### DAT-003 — `node_runs.status` enum is frozen for v1

- **Statement:** The `status` enum MUST be limited, for v1, to exactly: `pending`, `running`,
  `done` (terminal), `failed` (terminal, retryable), `cancelled` (terminal), `interrupted`
  (terminal, side effect uncertain), `killed` (terminal, non-retryable). `killed` MUST NOT be
  conflated with `failed`: a killed node-run MUST NOT be retried automatically; resuming from
  `killed` MUST require an explicit manual act.
- **Rationale:** These states encode the outcomes the reconciler, retry logic, and audit trail
  all depend on; a mid-flight change to this enum is either a painful migration or a
  reinterpretation of history.
- **Verification:** Inspection (enum type in the migration matches this set exactly; retry
  logic path explicitly excludes `killed`).
- **Depends on:** INV-008, LIFE-016

#### DAT-004 — `input_ref`/`output_ref` are references, never inline blobs

- **Statement:** `node_runs.input_ref` and `output_ref` MUST reference the Object Store
  (RustFS) and MUST NOT store payload content inline in Postgres.
- **Rationale:** Keeps Postgres focused on control-plane state and bounded row sizes; large
  payloads belong in object storage, addressed by reference.
- **Verification:** Inspection (column type is a reference/URI type or equivalent, not
  `bytea`/`jsonb` holding arbitrary payload content).

#### DAT-005 — `executions` reserved columns

- **Statement:** The `executions` table MUST have primary key `(id)` and MUST include the
  reserved columns 🔒 `tenant_id`, `deadline`/`ttl`, `version_pin`, `replay_of`, plus `status`,
  `created_at`, `updated_at`.
- **Rationale:** `deadline`/`ttl` is the termination circuit-breaker (`INV-008`); `version_pin`
  pins the compiled IR hash so a 30-day-old in-flight execution resumes against the version it
  started with; `replay_of` records replay lineage as a new linked run, never a mutation.
- **Verification:** Inspection (schema migration defines exactly these columns).
- **Depends on:** INV-008, EXE-008, LIFE-028

#### DAT-006 — `map_state` reserved columns

- **Statement:** The `map_state` table MUST have primary key `(execution_id, node_id)` and MUST
  include the reserved column 🔒 `tenant_id` and 🔒 `fan_in_status` (a CAS field transitioning
  `pending -> aggregating`), plus `total_items` and `remaining_terminal` for completeness
  detection.
- **Rationale:** Fan-in uniqueness for Map and Parallel lives entirely in the CAS on
  `fan_in_status`; this is the one irreducible synchronization point in the Map/Parallel design.
- **Verification:** Inspection (schema migration defines exactly these columns; a CAS UPDATE
  test on `fan_in_status` shows exactly one winner under concurrent attempts).
- **Depends on:** LIFE-012

#### DAT-007 — `connections` reserved columns

- **Statement:** The `connections` table (per-tenant application credentials) MUST include the
  reserved columns 🔒 `tenant_id` and 🔒 `egress_scope` (bound to the credential), plus
  `secret_ref`, which MUST hold a reference, never the raw secret value.
- **Rationale:** `egress_scope` bound per-credential (not global) closes host-mediated
  exfiltration and SSRF (`INV-010`); this field is expensive to retrofit, hence reserved from
  v1.
- **Verification:** Inspection (schema migration defines these columns; `secret_ref` column
  type cannot hold raw secret material — see `SEC-001`).
- **Depends on:** INV-010, SEC-011

#### DAT-008 — `audit_log` is append-only and hash-chained

- **Statement:** The `audit_log` table MUST include `tenant_id`, `prev_hash`, `hash` (a
  tamper-evident chain), `actor` (e.g. `user:<id>`, `agent:<id>@user:<id>`, or
  `ttl_exceeded`), `motif` (mandatory for high-consequence actions such as a kill), `resource`,
  `action`, `result`, `ts`. Append-only MUST be enforced at the database role level, not by
  application convention, and each entry MUST be written in the same transaction as the
  action it audits.
- **Rationale:** Role-level enforcement is the only guarantee that survives an application bug;
  same-transaction writes are what the outbox pattern in `spec/05-observability.md` depends on.
- **Verification:** Inspection (DB role grants exclude `UPDATE`/`DELETE` on `audit_log`) + Test
  (attempt an `UPDATE`/`DELETE` as the application role and assert it is rejected).
- **Depends on:** OBS-001, OBS-002, OBS-003

#### DAT-009 — `outbox` bridges Postgres to NATS

- **Statement:** The `outbox` table MUST include `payload`, `created_at`, `published_at`, and
  MUST act as a durable buffer between a database transaction and NATS publication.
- **Rationale:** Implements the outbox pattern (`OBS-004`) that prevents the dual-write hole
  where an action commits but its notification is lost.
- **Verification:** Inspection (schema migration defines these columns) + Test (kill the relay
  process after a commit but before publish; assert the row remains and is eventually
  published on relay restart).
- **Depends on:** OBS-004

#### DAT-010 — `signals` never stores the HITL token

- **Statement:** The `signals` table (`spec/06-durability-lifecycle.md` HITL Wait-on-signal)
  MUST include `execution_id`, `node_id`, `iteration_index`, `tenant_id`, the reserved column
  🔒 `wait_deadline`, `key_id`, and `expected_payload_shape`. It MUST NOT store the signing
  token; the signature MUST be recomputable from persisted state and the signing key.
- **Rationale:** A stored token would be a stored secret with all the taint/leak obligations of
  `spec/04-secrets-security.md`; recomputing it server-side (like an S3 presigned URL) avoids
  that entirely.
- **Verification:** Inspection (schema migration defines exactly these columns, no token
  column exists).
- **Depends on:** LIFE-019, LIFE-021

#### DAT-011 — `retention_class` is present on every flow from v1

- **Statement:** Every data flow (audit, application logs, payloads) MUST carry a
  `retention_class` field from v1: audit (years, immutable), application (days), payloads
  (short-lived, purgeable for erasure requests).
- **Rationale:** Retrofitting retention classification after data has accumulated is far more
  costly than declaring it from the start, and is required to answer "where is this tenant's
  data" for erasure requests.
- **Verification:** Inspection (every table holding retainable data has a `retention_class`
  column or is explicitly mapped to one in the retention policy).

#### DAT-012 — `Secret<T>` is a base type, not an add-on

- **Statement:** `Secret<T>` MUST be a distinct, taint-tracked type that is part of the base
  type system through which all engine data circulates, rendering `[REDACTED]` by default.
- **Rationale:** A type this fundamental cannot be added after the fact without walking every
  existing code path that touches data; it must exist in the base type system from the start.
- **Verification:** Inspection (type is defined in the core's foundational type module, used by
  the compiler/IR/runtime, not layered on top as a wrapper introduced late).
- **Depends on:** INV-007, SEC-006, SEC-007, SEC-008

#### DAT-013 — Content-addressed IR and WASM modules

- **Statement:** The compiled IR and WASM modules MUST be addressed by content hash. A workflow
  definition MUST reference nodes by `(logical identity + version) -> hash`. WASM blobs MUST
  live in the Object Store (RustFS) with metadata in Postgres; Postgres MUST NOT store blob
  content as `bytea`.
- **Rationale:** Content-addressing makes rollback a matter of repointing to a prior hash, and
  keeps Postgres free of large binary payloads.
- **Verification:** Test (compile the same source twice, assert identical hash; modify source,
  assert different hash) + Inspection (no `bytea` blob columns hold WASM module content).
- **Depends on:** EXE-008
