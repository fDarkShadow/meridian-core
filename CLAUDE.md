# Meridian — project context (read this first)

> **Code name: Meridian.** Replace it everywhere once the final name is chosen.

Meridian is a **durable, source-available workflow orchestration engine**.
Founding principle: **a workflow is data (a declarative graph, interpreted), not code.**
The engine *interprets* a graph; it never replays imperative code.

This file is the working contract. Details live in `spec/`. **Before writing code in a given
domain, read the matching `spec/` file.** Do not reopen a decision that is already settled: if
a constraint gets in your way, say so — do not silently work around it.

Every requirement in `spec/` is written in EARS syntax with RFC 2119 keywords, carries a
stable ID, and maps 1:1 to a GitHub issue and then to one PR. See `spec/CONVENTIONS.md` before
touching any spec file or opening an issue against one.

---

## Non-negotiable invariants (see `spec/00-invariants.md`)

These rules are never violated. If an implementation seems to require violating one, the
implementation is wrong.

1. **Durable / best-effort boundary.** Every mechanism is either durable (transactional,
   source of truth) or best-effort (transient, recoverable). Never in between.
2. **Postgres is the ONLY source of truth.** NATS carries only the *notification* of an
   already-durable fact — never the fact itself. Nothing reads NATS to learn the truth.
3. **Durability at node boundaries.** State is consistent *between* nodes, never in the middle
   of one. The node boundary is the only safe interruption/resume point.
4. **Mandatory internal idempotence** keyed by `(execution_id, node_id, iteration_index,
   attempt)`. Resume is an upsert conditioned on that key. Not delegable to the pipeline
   author.
5. **At-least-once** execution (side effects may be replayed). A **reception-time idempotence
   guard** discards already-terminal work arriving from the bus.
6. **Control primitives = a CLOSED set owned by the core.** Tasks (leaf nodes) are open
   (WASM plugins). Macros *compile* into primitives. A plugin NEVER adds a control primitive.
7. **Secret = a taint-tracked type.** Secret material and `sensitive` fields never circulate
   in a loggable, plain form. The output regex is only a safety net.
8. **Guaranteed termination.** Every execution reaches a terminal state in bounded time (3
   bounds: ack-wait, node timeout, execution TTL). No path leads to permanent blocking.
9. **No bypass.** Every interface (GUI, CLI, MCP) is an *API consumer*. None holds a capability
   the API does not expose, nor a path that escapes RBAC / tenant scope / RLS / audit.
10. **Host-mediated egress.** A node never performs direct I/O: it *describes* the call, the
    host injects the credential outside the sandbox, egress is allowlisted *per credential*.

Full EARS/RFC 2119 statements for each of these: `spec/00-invariants.md` (`INV-001`…`INV-011`).

---

## Stack (decided)

| Layer | Choice | Notes |
|---|---|---|
| Source of truth | **PostgreSQL** (self-managed, CloudNativePG) | 3 synchronous replicas, anti-affinity |
| Transport / nervous system | **NATS JetStream** | dispatch, triggers, progress, Object Store |
| Object storage | **RustFS** (S3-compatible, self-hosted) | WASM blobs, binary payloads, WAL archives |
| Node execution | **In-process WASM** (Wasmtime / Extism) | Component Model / WIT for the interface |
| Core↔node protocol | **gRPC / WIT** | no "human-readable" REST API required |
| Definition format | **JSON with ASL semantics** (Amazon States Language) | YAML accepted for authoring → compiled to IR |
| Expressions | **JSONata** (or CEL) — sandboxed, non-Turing-complete | field access, no custom template engine |
| Provider (start) | **OVHcloud** | portable; see `spec/07-infra.md` |
| Network | **Cilium** (CNI + mesh, eBPF) + WireGuard everywhere | Gateway API; no Istio unless advanced L7 is needed |
| IdP | **Keycloak** on our side; core is **IdP-agnostic** (OIDC/OAuth 2.1) | role→permission mapper lives in the core |
| Detection | Tetragon (runtime) + CrowdSec (north-south) + Wazuh (SIEM) + Vector | |
| License | **BSL** from day zero → Apache 2.0 (4-year rolling change date) | CLA mandatory; see `spec/10-licensing.md` |

**Language convention:** everything is in English — prose, comments, identifiers, table/column
names, states, and scopes alike (`node_runs`, `leased_until`, `execution:kill`, …).

---

## The core contract (the line that holds everything together)

The core owns four things; the GUI/editors own NONE of them:
(a) the graph schema, (b) the data/item model, (c) expression/referencing semantics,
(d) the node execution protocol.
Every other surface (GUI, CLI, MCP, third-party editors) is a *client* of this contract.
The core expresses authorization as **internal permissions**; the IdP is translated exactly
once at the boundary by the mapper (`spec/09-mcp-auth.md`).

**State machine in the core, dataflow projection in the editor.** The core is rigorous (ASL);
"flow" ergonomics are a GUI projection, never a core semantic.

---

## How to work on this repo

- **Read the matching `spec/` domain file before coding.** Several may apply.
- **Read `spec/CONVENTIONS.md` before touching a requirement.** Every requirement is one
  GitHub issue; every issue is closed by one PR; **review and merge are manual** — no
  auto-merge, no bot approval substitutes for a human reviewer (see `LIC-007`).
- **Respect the v1 reserved columns** (`spec/02-data-model.md`): they live in the key, the
  type, or an enum. Adding them later means a painful migration or reinterpreting history.
  Never omit them "to move faster".
- **Idempotence and durability first.** Any code touching execution state must be correct
  under crash/redelivery before it is optimized.
- **Never put a secret in a log, an output, a model context, or the IR.** Use the
  taint-tracked `Secret<T>` type.
- **When torn between "simple but violates an invariant" and "more work but correct", choose
  correct** and explain the extra cost in the PR description.
- Every PR is reviewed by a human (protectability + provenance, see `spec/10-licensing.md`).
  A license scan (SCA) runs in CI: never introduce an incompatible copyleft dependency.

---

## Requirement → issue → PR workflow

1. Every normative statement in `spec/00`–`spec/10` has a stable ID (`INV-001`, `DAT-003`, …).
2. Each ID becomes exactly one GitHub issue, titled `<ID>: <short title>`, labeled with its
   domain (`spec:DAT`) and its phase (`phase:1`) per `spec/11-build-order.md`.
3. Each issue is closed by exactly one PR (grouping is allowed only when requirements are
   inseparable at the implementation level — see `spec/CONVENTIONS.md`).
4. The PR states which requirement ID(s) it satisfies and which verification method (Test /
   Inspection / Analysis / Demonstration) it used.
5. A human reviews and merges by hand. No auto-merge.

---

## Spec index

| File | Domain |
|---|---|
| `spec/CONVENTIONS.md` | Requirement ID scheme, EARS patterns, RFC 2119 usage, issue/PR workflow |
| `spec/00-invariants.md` | The non-negotiable rules (detailed, `INV-*`) |
| `spec/01-execution-model.md` | ASL semantics, closed primitives, macros, expressions (`EXE-*`) |
| `spec/02-data-model.md` | v1 schema, reserved columns, states, content-addressing (`DAT-*`) |
| `spec/03-node-protocol.md` | In-process WASM, WIT, host-mediated I/O, lifecycle (`PROTO-*`) |
| `spec/04-secrets-security.md` | SecretProvider, envelope encryption, taint, egress-scope (`SEC-*`) |
| `spec/05-observability.md` | Hash-chained audit, outbox, OTel logs, retention (`OBS-*`) |
| `spec/06-durability-lifecycle.md` | Leases, reconciler, Map, cancellation, HITL, webhooks (`LIFE-*`) |
| `spec/07-infra.md` | Cluster, CNPG, Cilium networking, tenant hard-isolation GitOps, backups (`INFRA-*`) |
| `spec/08-security-detection.md` | Tetragon, CrowdSec, Wazuh, Vector, blind spots (`DET-*`) |
| `spec/09-mcp-auth.md` | MCP as API consumer, OIDC, role→permission mapper (`MCP-*`) |
| `spec/10-licensing.md` | BSL, CLA, file headers, AI provenance (`LIC-*`) |
| `spec/11-build-order.md` | Suggested implementation order, phase→requirement traceability, open topics |
