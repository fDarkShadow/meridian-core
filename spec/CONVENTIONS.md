# Requirements conventions

This file defines how every document under `spec/` states requirements. It exists so that
each requirement can become exactly one GitHub issue, and each issue can become exactly one
pull request, reviewed and merged by hand. Read this once before reading any `spec/NN-*.md`
file.

## Requirement ID scheme

Every normative statement has a stable ID: `<PREFIX>-<NNN>`, zero-padded to three digits,
unique within its prefix for the lifetime of the project. IDs are never reused and never
renumbered — a superseded requirement is marked `Status: Superseded by <ID>`, not deleted and
not renumbered, so that issue/PR history stays traceable.

| Prefix | Domain | File |
|---|---|---|
| `INV` | Non-negotiable invariants | `spec/00-invariants.md` |
| `EXE` | Execution model (ASL semantics, expressions, IR) | `spec/01-execution-model.md` |
| `DAT` | Data model (schema v1) | `spec/02-data-model.md` |
| `PROTO` | Node protocol (WASM in-process) | `spec/03-node-protocol.md` |
| `SEC` | Secrets, taint, egress-scope | `spec/04-secrets-security.md` |
| `OBS` | Observability & audit | `spec/05-observability.md` |
| `LIFE` | Durability, lifecycle, Map, cancellation, HITL | `spec/06-durability-lifecycle.md` |
| `INFRA` | Infrastructure | `spec/07-infra.md` |
| `DET` | Detection & cybersecurity | `spec/08-security-detection.md` |
| `MCP` | MCP & authorization | `spec/09-mcp-auth.md` |
| `LIC` | Licensing & rights governance | `spec/10-licensing.md` |

`spec/11-build-order.md` does not mint new IDs. It sequences the IDs above into phases; it is
a roadmap and a traceability index, not a source of requirements.

## Requirement format

Each requirement is a level-4 heading followed by three fields:

```
#### INV-001 — Short title

- **Statement:** <EARS-pattern sentence using MUST / MUST NOT / SHOULD / SHOULD NOT / MAY>
- **Rationale:** <why this exists — one or two sentences, not a design essay>
- **Verification:** <Test | Inspection | Analysis | Demonstration>
```

Optional fourth field, used only where it adds real traceability:

- **Depends on:** `<ID, ID, …>` — other requirements this one presupposes.

Keep each requirement atomic (ISO/IEC/IEEE 29148: one statement, one testable claim). If a
paragraph in the old spec bundled several obligations, split it into several IDs rather than
one compound requirement — that split is what makes "one requirement = one issue" work.

## EARS patterns used

Every **Statement** follows one of these five patterns (Easy Approach to Requirements
Syntax). Pick the narrowest one that fits:

- **Ubiquitous:** `The <system/component> MUST <response>.`
- **Event-driven:** `When <trigger>, the <system/component> MUST <response>.`
- **State-driven:** `While <state>, the <system/component> MUST <response>.`
- **Unwanted behavior:** `If <undesired condition>, then the <system/component> MUST <response>.`
- **Optional feature:** `Where <feature is present/enabled>, the <system/component> MUST <response>.`

Complex requirements may combine a state or trigger clause with an unwanted-behavior clause,
but never combine two unrelated obligations in one statement.

## RFC 2119 keywords

Keywords are used exactly as defined in RFC 2119 / BCP 14:

- **MUST / MUST NOT / SHALL / SHALL NOT** — absolute requirement or prohibition. Violating one
  of these is a defect, not a trade-off.
- **SHOULD / SHOULD NOT** — strong recommendation; deviating requires a documented reason in
  the PR that deviates.
- **MAY / OPTIONAL** — genuinely optional; the system is conformant either way.

`spec/00-invariants.md` is entirely MUST/MUST NOT by construction — see its own header.

## Verification methods

- **Test** — an automated test (unit/integration/property) exercises the behavior directly.
- **Inspection** — verified by reading code/config/schema against the statement (e.g. "column
  exists with this type").
- **Analysis** — verified by reasoning/proof/model-checking where a live test is impractical
  (e.g. a liveness argument about the reconciler).
- **Demonstration** — verified by running the system end-to-end and observing the behavior
  (used sparingly, mostly for infra/ops requirements).

A PR closing a requirement's issue MUST state which verification method it satisfies and how.

## From requirement to issue to PR

1. **One requirement = one GitHub issue.** The issue title is `<ID>: <short title>` (e.g.
   `DAT-003: node_runs.status enum is frozen to the v1 set`). The issue body quotes the
   Statement, Rationale and Verification fields verbatim and links back to the spec file.
2. **One issue = one PR.** A PR implements exactly one requirement unless requirements are
   explicitly grouped (see below). The PR description names the requirement ID and its
   verification method, and states how it was satisfied.
3. **Review and merge are manual.** No auto-merge, no bot approval substitutes for a human
   reviewer. This is also a licensing/provenance control — see `LIC-007`.
4. **Grouping requirements into one issue/PR is allowed only when they are inseparable at the
   implementation level** (e.g. a schema migration that must create several reserved columns
   in the same DDL statement). When grouped, the issue title lists every ID it covers and the
   PR must satisfy all of them before merge — splitting is always preferred when it is
   possible without breaking atomicity of the change.
5. **Traceability:** `spec/11-build-order.md` maps every requirement ID to a phase and a
   GitHub milestone. Issues are labeled with their domain prefix (e.g. `spec:DAT`) and their
   phase (e.g. `phase:0`).
