# 11 — Implementation roadmap & traceability

This file does not mint requirement IDs (see `spec/CONVENTIONS.md`). It sequences the
requirements defined in `spec/00`–`spec/10` into phases, and maps each phase to a GitHub
milestone. Governing principle: **engine correctness before anything else.** Nothing that
touches execution state gets optimized before it is correct under crash/redelivery.

Each phase below is a GitHub milestone (`Phase 0` … `Phase 4`). Every requirement ID listed
becomes one issue labeled with its domain prefix (e.g. `spec:DAT`) and its phase label (e.g.
`phase:0`), per the one-requirement-one-issue-one-PR workflow in `spec/CONVENTIONS.md`.

---

## Phase 0 — Foundations

| Deliverable | Requirement IDs |
|---|---|
| Complete Postgres v1 schema, RLS, frozen terminal-state enum, `Secret<T>` type | DAT-001…DAT-013, INV-004, INV-007 |
| IR + content-addressing (parse YAML/JSON → validate → normalize → hash-addressed store) | EXE-008, EXE-009, DAT-013 |
| gRPC/WIT node-protocol skeleton | PROTO-001, PROTO-002 |

## Phase 1 — The correct engine (the non-negotiable core, first)

| Deliverable | Requirement IDs |
|---|---|
| ASL state machine: closed primitives + Retry/Catch/Timeout modifiers | EXE-001…EXE-007, EXE-010, INV-006 |
| Execution loop: node-boundary durability, internal idempotence (upsert by node-run-id), reception guard, resume without replay | INV-003, INV-004, INV-005, LIFE-001 |
| Leases + reconciler (reads Postgres) + ack-wait; optional heartbeat | LIFE-002…LIFE-006 |
| Durable Map (iteration_index, semaphore, push+pull fan-in CAS, collect policy) | LIFE-009…LIFE-013 |
| Hierarchical cancellation/kill + `interrupted`/`killed` states + execution TTL | LIFE-007, LIFE-008, LIFE-014…LIFE-018 |
| Hash-chained audit + outbox + relay (build early: it is transactional with state) | OBS-001…OBS-004, DAT-008, DAT-009 |

## Phase 2 — Node execution

| Deliverable | Requirement IDs |
|---|---|
| WASM runtime (Wasmtime/Extism), WIT interface, one instance per invocation, epoch-interruption | PROTO-006, PROTO-007, PROTO-009 |
| Host-mediated I/O + capabilities + resource cleanup on every exit path | PROTO-003…PROTO-005, PROTO-008, PROTO-010 |
| `SecretProvider` (Postgres + envelope encryption) + egress-scope + end-to-end taint | SEC-001…SEC-013 |

## Phase 3 — Surfaces & ingestion

| Deliverable | Requirement IDs |
|---|---|
| Control API (the contract) + IdP-role→permission mapper (deny-by-default, per-tenant) | MCP-005, MCP-006, MCP-010…MCP-012, INV-009 |
| Triggers: decoupled scheduler + timers; webhook harness (durable-before-ack, dedup) | LIFE-026, LIFE-027, LIFE-029 |
| HITL: `Wait-on-signal` + presigned HMAC (recomputable, CAS, wait_deadline, key rotation) | LIFE-019…LIFE-025 |
| Observability: pluggable OTel exporters + NATS→websocket live-tail | OBS-005…OBS-009 |
| Replay (`replay_of` lineage, 3 granularities) | LIFE-028 |
| MCP server (API consumer, agent scopes, mediated BYO model) | MCP-001…MCP-004, MCP-007…MCP-009, MCP-013…MCP-015 |

## Phase 4 — Platform

| Deliverable | Requirement IDs |
|---|---|
| GitOps infra: cluster (mutualized pools + tainted WASM nodes), CNPG (3 anti-affine replicas, WAL→external S3, PITR), RustFS, NATS, Cilium+WireGuard+Gateway API, deny-all | INFRA-001…INFRA-009, INFRA-013 |
| Tenant hard isolation: full-stack GitOps manifest + quiesce/migration job | INFRA-010, INFRA-014 |
| Backups & continuity | INFRA-011 |
| Transactional email | INFRA-012 |
| Detection: Tetragon, CrowdSec (Hubble parser), Vector→Wazuh, per-tenant egress anomaly | DET-001…DET-008 |
| Licensing/governance controls (apply from the first commit, tracked here for completeness) | LIC-001…LIC-009 |

---

## Open topics (tooling/infra — do not touch the model; do not block on these)

- **Deterministic engine test/replay**: make the engine replayable in tests from an arbitrary
  persisted state. Design this into the architecture during Phase 1, not retrofitted later.
- **Dedicated GUI read-model**: denormalized projections, only once direct SQL queries are a
  demonstrated bottleneck (see `OBS-008`).
- **Map beyond N items**: v1 has an explicit bound (`LIFE-011`) plus application-level
  pagination; distributed batch Map is reserved, not built.
- **WASM module registry**: packaging/distribution (OCI?).
- **Per-tenant backpressure/fairness**: partitioned NATS queues / quota-aware scheduling — the
  free tier makes this necessary early, so anticipate it at dispatch time (`DET-008`).
- **mTLS / full zero-trust identity**: at compliance scale (see the non-normative note in
  `spec/08-security-detection.md`).
- **Hot migration** (logical replication): once a large customer requires it (`INFRA-014`).

## Cross-cutting reminders (re-check on every PR)

- No reserved 🔒 column omitted (`spec/02-data-model.md`).
- No secret in a log, output, model context, or the IR (`INV-007`, `spec/04-secrets-security.md`).
- The reconciler reads Postgres, never NATS (`INV-002`, `LIFE-003`).
- Audit is written in the same transaction as the action, never via a subscriber (`OBS-003`,
  `OBS-004`).
- No new control primitive (plugins are Tasks + macros only) (`INV-006`).
- MCP / GUI / CLI are API consumers, zero bypass (`INV-009`, `MCP-001`).
- CI license scan is green — no incompatible copyleft dependency (`LIC-008`).
