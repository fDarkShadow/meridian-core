# 05 — Observability & audit

See `spec/CONVENTIONS.md` for the requirement format and the `OBS-NNN` ID scheme.

## Context

Two systems with opposite requirements must never be mixed: the audit log is *data* (an
append-only, hash-chained, queryable event store in Postgres, the source of truth), while
application logs are ephemeral diagnostics (structured, OpenTelemetry-shaped, high-volume,
sampleable, best-effort). Losing an application log line is not an incident; losing an audit
entry is.

---

#### OBS-001 — Audit log is append-only, hash-chained, attributed

- **Statement:** The audit event store MUST be append-only (enforced at the database role
  level, not by convention), timestamped, attributed (`actor` + `tenant` + trace-id), and
  hash-chained so each entry includes the hash of the previous entry for the same tenant.
- **Rationale:** These properties together make the audit log tamper-evident and admissible as
  a real record of what happened, not just a best-effort log.
- **Verification:** Inspection (DB role grants) + Test (verify a chain of entries: each
  `prev_hash` matches the previous entry's `hash`).
- **Depends on:** DAT-008

#### OBS-002 — Hash chain is computed at DB write time

- **Statement:** The hash-chain computation MUST occur at the moment of the database write,
  never inside a NATS subscriber. Insertion MUST be serialized per tenant, since each entry
  depends on the hash of the previous one for that tenant.
- **Rationale:** Canonical ordering belongs to the table; NATS delivery order would corrupt the
  chain if hashing happened downstream of the bus.
- **Verification:** Inspection (hash computation code path executes inside the same DB
  transaction/trigger as the insert, not in a subscriber process).
- **Depends on:** OBS-001, INV-002

#### OBS-003 — Audit entry is written in the same transaction as the audited action

- **Statement:** Every audit entry MUST be written in the same database transaction as the
  state change it audits.
- **Rationale:** This is what prevents a crash between "action committed" and "audit written"
  from producing a real action with zero audit trace.
- **Verification:** Test (inject a crash immediately after the state-change write but before a
  hypothetical separate audit write would occur; assert this scenario cannot exist because
  they share one transaction).
- **Depends on:** DAT-008

#### OBS-004 — Outbox pattern; no dual-write via a NATS subscriber

- **Statement:** The engine MUST NOT let a NATS subscriber be the origin of an audit write
  (e.g. "worker publishes an audit event, a subscriber writes it to the DB"). Instead, in one
  transaction, the worker MUST write the state change, the `audit_log` entry, and an `outbox`
  row; a separate relay (CDC, logical replication, or an `outbox` poller) MUST then publish to
  NATS; NATS subscribers MUST treat this as a best-effort fan-out (websocket, email, syslog,
  otlp, SIEM).
- **Rationale:** This is the pattern that prevents the dual-write hole: if NATS is down, no
  audit entry is lost, because entries live in the database and the relay catches up when NATS
  recovers.
- **Verification:** Test (stop the relay/NATS after a commit; assert the outbox row persists
  and is published once the relay resumes, with no duplicate or lost audit entries).
- **Depends on:** DAT-009, OBS-003, INV-001, INV-002

#### OBS-005 — Application logs are best-effort, OpenTelemetry-shaped

- **Statement:** Application/diagnostic logs MUST be structured (JSON), leveled, correlated by
  trace-id, and modeled on OpenTelemetry. They MUST be treated as best-effort — losing a log
  line MUST NOT be treated as an incident.
- **Rationale:** Keeps diagnostic logging cheap and high-volume without inheriting the audit
  log's durability obligations.
- **Verification:** Inspection (application logging pipeline is separate from the audit-log
  write path; no application log write participates in the audit transaction).
- **Depends on:** INV-001

#### OBS-006 — Pluggable export sinks behind a common interface

- **Statement:** Each audit/observability event MUST be produced exactly once, in a canonical
  schema (OTel), and MUST be exportable through interchangeable sinks (file, S3, syslog, otlp,
  websocket) behind a common interface, following the same pattern as `SecretProvider`
  (`SEC-002`).
- **Rationale:** A single canonical event plus pluggable sinks lets a new export channel be
  added as a new subscriber without touching the critical write path.
- **Verification:** Inspection (sink implementations conform to one interface; adding a sink
  requires no change to event-production code).
- **Depends on:** OBS-004

#### OBS-007 — Websocket is live-tail only, not a retention sink

- **Statement:** The websocket channel MUST serve only as a live-tail feed to the editor via
  the NATS pub/sub bridge. It MUST NOT be used as a retention sink, and no second real-time
  delivery path MUST be built alongside it.
- **Rationale:** Reuses the existing NATS-based nervous system instead of introducing a
  redundant real-time mechanism with its own failure modes.
- **Verification:** Inspection (websocket handler reads from the NATS bridge; no independent
  storage/query path backs the websocket feed).
- **Depends on:** INV-002, OBS-004

#### OBS-008 — DAG/execution visualization is derivable via SQL in v1

- **Statement:** DAG-status coloring, per-node error display, drill-down (input/output,
  attempts, timing), and timeline views MUST be derivable via SQL queries on `node_runs`,
  `executions`, and `audit_log`, with live updates from the NATS pub/sub bridge. A dedicated
  denormalized read-model MUST NOT be built until direct queries are demonstrated to be a
  bottleneck.
- **Rationale:** Building a projection layer ahead of demonstrated need is premature
  optimization; the reserved schema (`DAT-002`, `DAT-005`) already supports these views
  directly.
- **Verification:** Demonstration (each visualization listed above is implemented as a SQL
  query plus live NATS subscription, with no separate projection store).
- **Depends on:** DAT-002, DAT-005, DAT-008

#### OBS-009 — `retention_class` governs retention per flow

- **Statement:** Each data flow MUST carry a `retention_class` (audit: years, immutable;
  application: days; payloads: short-lived, purgeable for erasure requests), as declared in
  `DAT-011`.
- **Rationale:** Explicit retention classification is what makes "where is this tenant's data"
  answerable, and keeps payload data from being scattered across untracked files.
- **Verification:** Inspection (every retainable flow maps to a `retention_class`; a purge job
  for the "payloads" class exists and is exercised in tests).
- **Depends on:** DAT-011
