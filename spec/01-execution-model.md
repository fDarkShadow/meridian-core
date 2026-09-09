# 01 — Execution model

See `spec/CONVENTIONS.md` for the requirement format and the `EXE-NNN` ID scheme.

## Context

The core is a state machine in the ASL (Amazon States Language / Step Functions) sense. The
project borrows the *semantics*, not necessarily the exact JSON syntax: loops, retry,
parallelism, and error handling become first-class, orthogonal citizens instead of ad-hoc
node types. The state machine lives in the core; the flow/dataflow ergonomics some editors
want are a GUI projection, never a core semantic.

---

#### EXE-001 — Core implements ASL-equivalent state machine semantics

- **Statement:** The core MUST implement execution semantics equivalent to ASL / Step
  Functions (states, transitions, error handling, retry, parallelism, timeouts) as first-class,
  orthogonal constructs.
- **Rationale:** Borrowing a proven semantics avoids reinventing ad-hoc, coupled behavior for
  loops/retry/parallel/error-handling.
- **Verification:** Inspection (state machine spec/compiler matches ASL semantics for each
  primitive in `INV-006`).

#### EXE-002 — State-machine semantics MUST NOT leak into the GUI

- **Statement:** The GUI/editor MUST treat the state machine as a projection (a dataflow view)
  and MUST NOT implement or duplicate execution semantics of its own.
- **Rationale:** Per `INV-009`/core contract, the GUI is a consumer, not an alternate source of
  execution semantics; duplicated semantics would drift from the core and become a bypass.
- **Verification:** Inspection (GUI code review: no local evaluation of retry/timeout/branching
  logic; it renders and edits IR, it does not interpret it).

#### EXE-003 — Closed set of state types

- **Statement:** The core MUST support exactly these state types: `Task` (invoke a leaf node /
  WASM plugin), `Choice` (conditional branching), `Wait` (durable wait, including HITL signal
  wait — see `spec/06-durability-lifecycle.md`), `Parallel` (concurrent branches with fan-in),
  `Map` (iteration with fan-out/fan-in), `Pass` (side-effect-free transformation), `Succeed` /
  `Fail` (terminal states).
- **Rationale:** This is the closed primitive set required by `INV-006`.
- **Verification:** Inspection (IR schema enumerates exactly this set).
- **Depends on:** INV-006

#### EXE-004 — Retry/Catch/Timeout are modifiers, not states

- **Statement:** `Retry` (retry policy: backoff, max attempts), `Catch` (error routing to a
  branch), and `Timeout` (per-node duration bound) MUST be modeled as attributes of a state or
  as composite-state modifiers. The compiler MUST NOT accept a standalone "Retry node", "Loop
  node", or "Wait node" distinct from the primitives in `EXE-003`.
- **Rationale:** Treating retry/catch/timeout as separate node types would smuggle new control
  primitives past `INV-006` through the back door.
- **Verification:** Inspection (IR schema: `Retry`/`Catch`/`Timeout` exist only as fields on a
  state definition, never as a `type` value).
- **Depends on:** INV-006, EXE-003

#### EXE-005 — Field-shaping at the state boundary, not per-field edges

- **Statement:** Edges MUST represent node-to-node control flow only. Field-level data shaping
  MUST occur at the input/output boundary of each state, via ASL-equivalent operators
  (`InputPath`, `Parameters`, `ResultSelector`, `ResultPath`, `OutputPath`). The engine MUST
  NOT model per-field edges between nodes.
- **Rationale:** Per-field edges produce spaghetti graphs that break under dynamic data shapes;
  boundary-based shaping keeps control flow and data flow orthogonal.
- **Verification:** Inspection (IR schema: edges carry no field-level references; shaping
  operators exist only on state input/output).

#### EXE-006 — Explicit output schema per node

- **Statement:** Each node type MUST declare an explicit output schema (a contract), not an
  untyped blob.
- **Rationale:** A declared schema enables static autocompletion/validation in the editor
  (versus runtime failures from stringly-typed expressions) and gives schema-directed
  redaction (`sensitive: true`) a place to live — see `SEC-009`.
- **Verification:** Inspection (every registered node type has a schema; the compiler rejects
  a node type registered without one).

#### EXE-007 — Sandboxed, non-Turing-complete expression language

- **Statement:** The expression language MUST be a sandboxed, non-Turing-complete language with
  native field access (JSONata or CEL). The engine MUST NOT implement a custom templating
  engine.
- **Rationale:** A custom template language accidentally grows into an unaudited programming
  language coupled to the internal data model; JSONata/CEL are proven, bounded alternatives
  (AWS Step Functions adopted JSONata in late 2024).
- **Verification:** Inspection (expression evaluator is backed by the chosen sandboxed engine;
  no custom parser/interpreter for expressions exists in the codebase).

#### EXE-008 — Authoring format compiles to a normalized, content-addressed IR

- **Statement:** The engine MUST accept YAML or JSON as the authoring format, parse it, validate
  it against the schema, compile it into a normalized IR with macros expanded, and store that
  IR content-addressed in Postgres. The engine MUST execute only the compiled IR, never the
  source text.
- **Rationale:** This reconciles "Postgres is the source of truth" with "authoring is YAML",
  and makes versioning of in-flight executions tractable (`DAT-006` `version_pin`).
- **Verification:** Test (round-trip: author YAML → compile → assert the engine executes the
  stored IR even if the source YAML file is later changed or deleted).
- **Depends on:** DAT-013

#### EXE-009 — YAML footguns MUST be rejected at parse time

- **Statement:** The parser MUST reject or normalize known YAML footguns (the Norway problem
  `no`/`off` implicitly parsed as boolean, implicit type coercion, unexpected anchors) before
  producing IR. The canonical form is the parsed AST, never raw bytes.
- **Rationale:** Silent YAML type coercion is a well-known source of production incidents in
  declarative pipeline formats.
- **Verification:** Test (parser test suite includes the Norway problem and anchor-based
  footguns; each must either be rejected with a clear error or normalized correctly, never
  silently misinterpreted).
- **Depends on:** EXE-008

#### EXE-010 — Item/data model owned by the core

- **Statement:** The data model for items flowing through the graph, including paired-item
  lineage across merges and branches, MUST be owned by the core and coupled to the expression
  semantics. It MUST NOT be defined or reinterpreted by the GUI.
- **Rationale:** This is part of the core contract (see `CLAUDE.md`): the GUI is a client of
  the item model, not its author.
- **Verification:** Inspection (item/lineage types are defined in the core crate/module; the
  GUI only consumes them via the core API).
