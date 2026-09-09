# 03 — Node protocol (in-process WASM)

See `spec/CONVENTIONS.md` for the requirement format and the `PROTO-NNN` ID scheme.

## Context

A node is an in-process WASM module, not a container. A "worker" is a host process that loads
and runs several WASM modules in-process — this avoids the pod-per-step model (e.g. Argo).
WASM is chosen because it is simultaneously in-process (no container-ops overhead), sandboxed
(hard memory isolation), and polyglot (Rust, Go/TinyGo, JS/QuickJS, Python/componentize-py).
Expected overhead is ~1.5–2x on heavy compute, but 95% of nodes are I/O-bound (remote API
calls), making this a non-issue; instances should be sized RAM-first (linear memory per
instance).

---

#### PROTO-001 — A node is an in-process WASM module

- **Statement:** A node MUST execute as an in-process WASM module on a Wasmtime (or Extism)
  runtime. A node MUST NOT be implemented as a separate container or process-per-step.
- **Rationale:** In-process WASM avoids the operational cost of a pod-per-step model while
  still providing sandboxing.
- **Verification:** Inspection (worker architecture: node execution occurs within the worker
  process's WASM runtime, no `exec`/container-spawn per node invocation).

#### PROTO-002 — Standard WIT / Component Model interface

- **Statement:** The core-to-node protocol MUST use the WIT / Component Model interface
  standard.
- **Rationale:** A standard interface enables polyglot node implementations without a
  bespoke per-language ABI.
- **Verification:** Inspection (node interface definitions are expressed in WIT).

#### PROTO-003 — Host-mediated I/O (no direct network access)

- **Statement:** A node MUST NOT perform a direct network call. It MUST describe the intended
  call (target capability, e.g. `http`; provider; body) to the host; the host MUST inject the
  credential at call time, outside the sandbox. The WASM module MUST NOT ever observe the
  secret.
- **Rationale:** This is the mechanism behind `INV-010`; it is what makes exfiltration and SSRF
  structurally impossible rather than merely discouraged.
- **Verification:** Test (a node that attempts a raw socket/network syscall from inside the
  sandbox MUST fail — no such capability is granted).
- **Depends on:** INV-010

#### PROTO-004 — Egress refusal outside credential scope

- **Statement:** When a node's described call targets a destination outside the credential's
  declared `egress_scope`, the host MUST refuse to attach the credential to that call.
- **Rationale:** This closes exfiltration (a malicious node cannot redirect a credential to an
  attacker-controlled destination) and SSRF, at the point where the credential would otherwise
  leave host memory.
- **Verification:** Test (node declares a call to a destination outside scope; assert the host
  refuses to attach the credential and the call fails or proceeds unauthenticated per policy).
- **Depends on:** INV-010, DAT-007, SEC-011

#### PROTO-005 — Zero ambient authority via granular WASI capabilities

- **Statement:** A node's runtime instance MUST be granted only the specific host functions it
  needs (granular WASI capabilities). A node MUST NOT hold ambient authority to call arbitrary
  host functions.
- **Rationale:** Reduces a node's blast radius to exactly the capabilities its manifest
  declares.
- **Verification:** Inspection (capability grant is derived from the node's declared manifest;
  no default "allow all host functions" path exists).

#### PROTO-006 — Epoch-based interruption for runaway modules

- **Statement:** The runtime MUST enforce a hard timeout on a module via Wasmtime
  epoch-interruption. The same mechanism MUST be reusable to implement an operator/TTL kill
  (see `LIFE-015`), differing only in trigger (time vs. external signal).
- **Rationale:** A single, well-tested interruption mechanism serving both "node timeout" and
  "kill" reduces the number of distinct failure paths to reason about.
- **Verification:** Test (a module in an infinite loop is interrupted at its configured
  timeout).
- **Depends on:** INV-008, LIFE-015

#### PROTO-007 — Per-instance memory limits and single-invocation isolation

- **Statement:** The runtime MUST enforce a memory limit per instance and MUST use one instance
  per invocation (or a pool with no shared state across tenants) for multi-tenant isolation. A
  module panic MUST NOT crash the worker process.
- **Rationale:** Per-invocation isolation prevents cross-tenant or cross-invocation state
  leakage within a shared worker process; a panicking module must be an isolated failure.
- **Verification:** Test (a module that panics or exceeds its memory limit does not affect a
  concurrently running module in the same worker).

#### PROTO-008 — Ordered resource cleanup on every exit path

- **Statement:** On every exit path (normal completion, epoch-interruption trap, panic), the
  host MUST close host-mediated resources (sockets, files, capability handles) tied to the
  instance's lifecycle before persisting the state snapshot.
- **Rationale:** Closing external resources before writing the durable snapshot ("close the
  outside world, then record the truth") avoids persisting a state that implies a resource is
  still open when it has already been abandoned.
- **Verification:** Test (for each exit path — normal, timeout trap, panic — assert host
  resources are closed and closure happens before the state-snapshot write).
- **Depends on:** INV-003

#### PROTO-009 — No OS-level zombies; orphans are resolved logically

- **Statement:** The engine MUST NOT rely on OS process semantics (PID, `wait()`) to detect a
  crashed node execution. A `running` node-run left behind by a crashed worker MUST be resolved
  by lease expiry and the reconciler, not by OS process management.
- **Rationale:** A WASM module is not an OS process; dropping the instance reclaims its linear
  memory immediately, but a logically orphaned node-run still needs the lease/reconciler
  mechanism to be reclaimed.
- **Verification:** Inspection (no code path polls OS process state to detect a dead node
  execution).
- **Depends on:** LIFE-004, LIFE-005

#### PROTO-010 — Node holds only an opaque credential handle

- **Statement:** The host MUST own the full credential lifecycle, including OAuth refresh. A
  node MUST receive only an opaque handle and MUST NOT have access to the token value or the
  refresh flow.
- **Rationale:** Keeps the node's trust boundary minimal — it cannot leak or misuse what it
  never holds.
- **Verification:** Inspection (node-facing API surface exposes only an opaque handle type for
  credentials, no token accessor).
- **Depends on:** SEC-013

---

## Non-normative note: ecosystem maturity caveats

Component Model / WASI Preview 2 is still stabilizing (2025–2026); `wasi-http` is the least
mature piece. JS/Python-in-WASM are heavy (embedded engine cost). TinyGo does not support 100%
of the Go language. None of this blocks the design above, but it should be validated early via
a spike before committing to a specific language SDK for node authors.

