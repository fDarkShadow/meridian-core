# 09 — MCP & authorization

See `spec/CONVENTIONS.md` for the requirement format and the `MCP-NNN` ID scheme.

---

#### MCP-001 — MCP server is a pure API consumer

- **Statement:** The MCP server MUST drive Meridian exactly as the GUI/CLI do: as a thin
  protocol adapter over the existing control API. It MUST NOT bypass RBAC, tenant scope, RLS,
  or audit through any private code path.
- **Rationale:** This is `INV-009` applied to MCP specifically; if the MCP server ever needs a
  shortcut, that is a sign the API itself has a gap, not a reason to add one.
- **Verification:** Inspection (every MCP tool handler calls the control API; no MCP-specific
  database or internal-service access exists).
- **Depends on:** INV-009

#### MCP-002 — MCP server runs inside the tenant's hard-isolated namespace

- **Statement:** The per-tenant MCP server MUST be deployed inside that tenant's hard-isolation
  namespace, inheriting its topological isolation (deny-all NetworkPolicies, RLS, visibility
  limited to that tenant's resources).
- **Rationale:** Placing the MCP server inside the existing tenant boundary gets strict
  isolation for free instead of requiring a bespoke MCP-specific isolation mechanism.
- **Verification:** Inspection (MCP server deployment manifest targets the tenant's namespace).
- **Depends on:** INFRA-010

#### MCP-003 — Mutating MCP tools require human confirmation

- **Statement:** Mutating MCP tools (launch, edit, kill, delete) MUST require explicit human
  confirmation before execution — the tool MUST return a confirmation request ("about to kill
  X, confirm") rather than executing directly. Read-only tools (list, inspect, diagnose) MAY
  have broad access without this friction.
- **Rationale:** A model can decide, on a hallucination, to call a destructive tool on its own;
  requiring human confirmation is deliberate friction against that failure mode.
- **Verification:** Test (invoke a mutating MCP tool; assert execution does not occur until a
  separate confirmation step succeeds).

#### MCP-004 — `execution:kill` via MCP requires the same permission as elsewhere

- **Statement:** Invoking `execution:kill` through MCP MUST require exactly the same RBAC
  permission required by any other interface.
- **Rationale:** Restates `INV-009`/`MCP-001` for the single highest-consequence action in the
  system, since it is the most tempting place to special-case.
- **Verification:** Test (attempt kill via MCP without `execution:kill`; assert denial
  identical to the GUI/CLI path).
- **Depends on:** MCP-001, LIFE-016

#### MCP-005 — Core stays IdP-agnostic via standard OIDC/OAuth 2.1

- **Statement:** The core MUST consume standard OIDC/OAuth 2.1 tokens and MUST NOT assume a
  specific identity provider. Keycloak MAY be used as the operated Authorization Server, but
  any IdP exposing standard discovery endpoints MUST be usable.
- **Rationale:** Coupling the core to one IdP's specifics would make the mapper described in
  `MCP-010` pointless and lock every future deployment to that IdP.
- **Verification:** Inspection (token validation uses standard OIDC discovery/JWKS, not
  IdP-specific APIs).

#### MCP-006 — Per-tenant MCP server acts as an OAuth 2.1 Resource Server

- **Statement:** The per-tenant MCP server MUST validate the presented token, extract
  `tenant_id` and scopes from it, and apply RBAC and RLS accordingly, per OAuth 2.1 Resource
  Server semantics.
- **Rationale:** Standard Resource Server behavior means the MCP server needs no bespoke
  authorization logic beyond what the core API already enforces.
- **Verification:** Test (present a token for tenant A to the MCP server scoped to tenant B;
  assert rejection).
- **Depends on:** MCP-005

#### MCP-007 — Agent session token carries a subset of the human's permissions

- **Statement:** The MCP session token MUST carry a subset of the authenticated human user's
  permissions, delegated explicitly via dedicated scopes (`mcp:read`, `mcp:execute`,
  `mcp:kill`).
- **Rationale:** No-bypass (`INV-009`) governs *authority*; least privilege governs the
  *granularity of delegation* — a separate, product-level decision that can be tightened later
  without an architecture change.
- **Verification:** Test (an MCP session scoped to `mcp:read` cannot invoke a mutating tool
  even if the underlying human user has the broader permission).
- **Depends on:** MCP-001

#### MCP-008 — Agent-initiated actions are audited with dual identity

- **Statement:** Audit entries for agent-initiated actions MUST record the dual identity
  `agent:<id>@user:<id>`.
- **Rationale:** Distinguishes a deliberate human action from an agent action taken on a
  hallucination, which matters both operationally and for incident review.
- **Verification:** Test (an action taken via MCP produces an audit entry with `actor` in the
  `agent:<id>@user:<id>` form).
- **Depends on:** DAT-008

#### MCP-009 — MCP server holds no ambient authority

- **Statement:** The MCP server MUST hold no ambient authority; every call MUST act strictly
  within the scoped session token's permissions. No all-powerful MCP service account MUST
  exist.
- **Rationale:** Prevents a confused-deputy scenario where the MCP server's own privilege,
  rather than the caller's, determines what an action can do.
- **Verification:** Inspection (MCP server has no service-account credential capable of acting
  outside a request's scoped token).
- **Depends on:** MCP-007

#### MCP-010 — Role-to-permission mapping happens once, in the core

- **Statement:** The core MUST reason exclusively in internal permissions (e.g.
  `execution:kill`, `workflow:read`). A mapper MUST translate incoming IdP roles into these
  internal permissions exactly once, at the core boundary. After that boundary, no component
  (RBAC, RLS, audit, MCP, GUI) MUST reason about IdP roles directly.
- **Rationale:** This anti-corruption layer keeps IdP-specific concepts from leaking into every
  downstream consumer of permissions.
- **Verification:** Inspection (only the mapper component reads IdP role claims; every other
  component consumes internal permission types).

#### MCP-011 — Unmapped IdP role grants zero permissions

- **Statement:** An IdP role with no explicit mapping entry MUST grant no permissions
  (deny-by-default allowlist of translations, not a passthrough).
- **Rationale:** A permissive fallback would silently grant unintended access whenever a new
  or misspelled IdP role appears.
- **Verification:** Test (present a token with an unmapped role; assert zero permissions are
  granted).
- **Depends on:** MCP-010

#### MCP-012 — Role-to-permission mapping is scoped per-tenant

- **Statement:** The role-to-permission mapping table MUST be scoped per-tenant; a role named
  `admin` in tenant A MUST NOT be conflated with a role named `admin` in tenant B.
- **Rationale:** Tenants may define their own IdP role names independently; only per-tenant
  scoping prevents cross-tenant privilege confusion.
- **Verification:** Test (tenant A's `admin` role mapping does not apply when evaluating a
  token for tenant B).
- **Depends on:** MCP-010, DAT-001

#### MCP-013 — RBAC is enforced server-side regardless of model requests (BYO model)

- **Statement:** When a tenant's own model drives MCP (BYO model), RBAC MUST be enforced
  server-side independent of what the model requests. The model MUST only be able to propose
  actions; the MCP server MUST validate every one. The model MUST NOT be treated as an
  authority.
- **Rationale:** The model is external and untrusted; prompt injection becoming action
  injection is the specific failure mode this closes.
- **Verification:** Test (a model-driven request for an action outside its granted scope is
  rejected server-side regardless of how the model justifies the request).
- **Depends on:** MCP-001, MCP-007

#### MCP-014 — Data returned to a BYO model never includes secrets; PII is opt-in

- **Statement:** Data returned to a tenant's BYO model MUST NOT include secret material
  (enforced by taint tracking) and MUST include PII only under explicit tenant opt-in.
- **Rationale:** BYO model implies the tenant's data leaves the platform's trust boundary
  toward their chosen model; this must not include secrets under any configuration, and PII
  inclusion must be a deliberate, documented tenant choice.
- **Verification:** Test (attempt to surface a `Secret<T>` value to a BYO model context; assert
  it renders `[REDACTED]`).
- **Depends on:** SEC-006, SEC-007, SEC-008, SEC-010

#### MCP-015 — Self-hosted model endpoint is an egress destination

- **Statement:** A self-hosted BYO model endpoint MUST be treated as an egress destination like
  any other, subject to `egress_scope` enforcement.
- **Rationale:** There is no reason for a model endpoint to be exempt from the same
  host-mediated egress control every other destination goes through.
- **Verification:** Inspection (calls to a self-hosted model endpoint route through the same
  host-mediated I/O path as any other node call).
- **Depends on:** PROTO-003, PROTO-004, SEC-011
