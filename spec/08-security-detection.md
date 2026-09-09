# 08 — Detection & cybersecurity

See `spec/CONVENTIONS.md` for the requirement format and the `DET-NNN` ID scheme.

## Context

Prevention (host-mediated I/O, egress-scope, WASM sandbox, taint, deny-all networking)
eliminates the most common exfiltration path by construction. Detection is the safety net for
"if we get breached anyway".

---

#### DET-001 — Egress mediation choke point is instrumented for per-tenant anomaly detection

- **Statement:** All egress passes through the host's I/O mediation, so the engine MUST
  instrument this single choke point with per-tenant behavioral anomaly detection (abnormal
  volume, abnormal rate, unusual destination), rather than diffuse packet sniffing elsewhere.
- **Rationale:** Every egress call already passes through one code path (`PROTO-003`); this is
  the cheapest, most complete place to observe tenant behavior.
- **Verification:** Test (simulate an abnormal-volume egress burst for one tenant; assert an
  alert fires scoped to that tenant).
- **Depends on:** PROTO-003, PROTO-004

#### DET-002 — Kernel-level runtime detection via Tetragon

- **Statement:** Runtime kernel detection (Tetragon, eBPF, built on the existing Cilium
  deployment) MUST detect and block anomalous syscalls, connections, and file access at the
  node/host level.
- **Rationale:** Provides a last-resort control if an RCE occurs despite the WASM sandbox and
  host mediation — e.g. killing an unexpected outbound socket.
- **Verification:** Demonstration (trigger a policy-violating syscall/connection in a test
  environment; assert Tetragon blocks it).

#### DET-003 — CrowdSec at the edge with real source IPs

- **Statement:** CrowdSec MUST be deployed north-south to defend exposed surfaces (webhooks,
  presigned URLs, auth, free tier) and MUST see real client source IPs. Behind a load balancer,
  `X-Forwarded-For` or proxy-protocol MUST be propagated through to the ingress, with the
  bouncer deployed at the ingress layer.
- **Rationale:** Without real source IPs, CrowdSec bans the load balancer's IP (or nothing) —
  this is a well-known install pitfall that must be closed explicitly.
- **Verification:** Inspection (LB forwards real client IP via `X-Forwarded-For` or
  proxy-protocol to ingress; bouncer is deployed at ingress, not further upstream).

#### DET-004 — Security telemetry flows to a central SIEM

- **Statement:** In-cluster security telemetry MUST be collected by Vector and forwarded to a
  central SIEM (Wazuh) for correlation.
- **Rationale:** Exfiltration is rarely a single isolated signal — correlation across sources
  is what surfaces it.
- **Verification:** Inspection (Vector pipelines route the relevant telemetry sources to
  Wazuh).
- **Depends on:** DET-002

#### DET-005 — Legitimate-credential abuse is caught by volume/rate anomaly, not destination

- **Statement:** Detection MUST cover a compromised-but-legitimate credential slowly exfiltrating
  data via its own authorized destination, using volume/rate anomaly detection on the egress
  mediation choke point rather than destination-based rules.
- **Rationale:** `egress_scope` sees nothing wrong in this scenario because the destination is
  authorized; only behavioral anomaly detection on the mediation point can catch it.
- **Verification:** Test (simulate a slow, steady data push to an authorized destination
  outside normal baseline; assert the anomaly detector flags it).
- **Depends on:** DET-001

#### DET-006 — At-rest exfiltration detection covers direct data-store access

- **Statement:** Detection MUST cover direct dumps of Postgres or RustFS via database access
  auditing, object-storage access logs, and detection of anomalous queries (e.g. an
  out-of-pattern mass `SELECT *`). Detection MUST NOT be concentrated solely on application-
  layer egress.
- **Rationale:** An attacker who reaches the data store directly bypasses the application
  egress path entirely; at-rest access must be monitored independently.
- **Verification:** Test (simulate an anomalous mass-select query pattern; assert it is flagged
  by the audit/access-log detection path).
- **Depends on:** DAT-008

#### DET-007 — Detection baselines are computed per-tenant

- **Statement:** Anomaly-detection baselines MUST be computed per-tenant, never globally.
- **Rationale:** A global baseline would mask a small tenant's anomalous behavior inside the
  aggregate noise of larger tenants.
- **Verification:** Inspection (baseline computation is scoped by `tenant_id` in its
  aggregation query/model).
- **Depends on:** DET-001

#### DET-008 — Free-tier abuse controls

- **Statement:** The free tier MUST enforce, per tenant and at dispatch time: concurrency
  limits, volume limits, bounded Map cardinality, and a short execution TTL. Sign-up MUST
  require email/card verification.
- **Rationale:** On providers like Hetzner/OVH, an abuser (cryptomining, outbound scraping) can
  get the platform's entire account suspended — these controls protect account survival, not
  just user experience.
- **Verification:** Test (free-tier tenant exceeding concurrency/volume/Map-cardinality limits
  is rejected at dispatch).
- **Depends on:** LIFE-010, LIFE-011, LIFE-007

---

## Non-normative note: identified blind spots and priorities

Beyond `DET-005` and `DET-006`, a **compromised host** (the attacker is inside the mediation
boundary itself, not just running a malicious tenant workflow) requires node-level Tetragon/
Falco watching the host as a last line of defense — this is covered by `DET-002` but worth
calling out as a distinct threat model from a malicious tenant pipeline.

Suggested implementation priority: Tetragon → egress-mediation anomaly detection → Wazuh
correlation via Vector → CrowdSec (already required) → per-tenant baselines.

Identity-based zero-trust (Cilium mTLS on top of L3/4 NetworkPolicies) is not required for v1;
it should be added when a compliance-driven customer requires it.
