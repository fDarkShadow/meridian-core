# 07 — Infrastructure

See `spec/CONVENTIONS.md` for the requirement format and the `INFRA-NNN` ID scheme.

## Context

Starting provider: OVHcloud (French, outside the US CLOUD Act, free egress, the most EU
regions). Nothing proprietary is used, so the architecture stays portable and reproducible via
GitOps.

---

#### INFRA-001 — Portability by reproducibility, not a stretched cluster

- **Statement:** The architecture MUST remain portable across providers by being reproducible
  via GitOps (spin up a new cluster elsewhere, replicate data via CNPG logical replication and
  RustFS sync, switch DNS). The control plane and etcd MUST NOT be stretched across providers.
  Only stateless workers MAY run in a remote region/provider. Multi-cluster deployments (e.g.
  per-region/per-jurisdiction dedicated tenants) MUST use Cilium Cluster Mesh, never a single
  cluster spanning multiple providers.
- **Rationale:** etcd's synchronous Raft consensus is intolerant of WAN latency; data and
  compute must stay co-located, and portability comes from reproducibility, not from
  stretching consensus across the internet.
- **Verification:** Inspection (deployment topology: no etcd or control-plane member resides in
  a different provider/region from its quorum peers).

#### INFRA-002 — Self-managed control plane with 3-node etcd HA

- **Statement:** The control plane MUST be self-managed (not a managed Kubernetes control
  plane), with HA achieved via 3 etcd nodes (odd quorum). Etcd nodes MUST NOT carry heavy
  application workload in production.
- **Rationale:** Self-managing the control plane is required for the portability strategy in
  `INFRA-001`; loading etcd nodes with application work risks starving consensus.
- **Verification:** Inspection (etcd node count and quorum configuration; scheduling
  constraints keep application pods off etcd nodes).
- **Depends on:** INFRA-001

#### INFRA-003 — Postgres via CloudNativePG, 3 synchronous replicas, hard anti-affinity

- **Statement:** PostgreSQL MUST run via CloudNativePG with 3 synchronous replicas under hard
  pod anti-affinity (never two replicas on the same physical hypervisor).
- **Rationale:** Hard anti-affinity is what bounds the blast radius of a single node loss to at
  most one lost replica.
- **Verification:** Inspection (CNPG cluster manifest declares `podAntiAffinity` as a hard
  requirement, not a preference) + Demonstration (terminate the node hosting one replica;
  assert only that replica is affected).

#### INFRA-004 — WAL archived off-cluster with PITR

- **Statement:** Postgres WAL MUST be archived to S3 storage external to the cluster and
  external to the provider, with Point-In-Time Recovery enabled.
- **Rationale:** Off-cluster, off-provider WAL storage is what survives a full-cluster or
  full-provider incident (the SBG2 lesson: resilience belongs to the customer, not the
  provider).
- **Verification:** Demonstration (restore to a point in time from off-cluster WAL archive
  after simulating full cluster loss).
- **Depends on:** INFRA-003

#### INFRA-005 — WASM workloads isolated via taint and nodeAffinity

- **Statement:** Nodes running WASM workloads MUST be isolated using both a taint
  (`workload=wasm:NoSchedule`) and a matching `nodeAffinity` (`workload=wasm`). Using only one
  of the two MUST NOT be considered sufficient isolation.
- **Rationale:** The taint keeps other workloads off these nodes; the affinity pins WASM pods
  onto them — omitting either lets isolation leak.
- **Verification:** Inspection (node taints and pod affinity rules are both present in the
  manifests for WASM worker nodes).

#### INFRA-006 — QoS classes protect the source of truth on shared nodes

- **Statement:** On mutualized nodes, Postgres and etcd pods MUST run at QoS class
  `Guaranteed` (requests = limits); worker/WASM pods MUST run at QoS class `Burstable`. The CNI
  MUST run at `system-node-critical` priority. `--system-reserved`/`--kube-reserved` MUST be
  configured so kubelet and the CNI always have resources to run.
- **Rationale:** `Guaranteed` QoS gives the most protective `oom_score_adj`, ensuring Postgres
  and etcd survive an OOM event that kills Burstable workers first — this is non-negotiable
  even under a tight budget.
- **Verification:** Inspection (pod specs: PG/etcd requests equal limits; worker pods do not) +
  Demonstration (induce node memory pressure; assert workers are OOM-killed before PG/etcd/CNI).

#### INFRA-007 — Cilium with WireGuard enabled cluster-wide

- **Statement:** Networking MUST use Cilium (CNI + eBPF-based mesh) with WireGuard encryption
  enabled for all node-to-node traffic. Istio MUST NOT be deployed alongside Cilium's mTLS
  unless advanced L7 traffic management (canary, fine-grained circuit-breaking) is required, in
  which case Istio ambient mode MAY be layered on top of Cilium — never two concurrent mTLS
  implementations in the same datapath.
- **Rationale:** Two concurrent mTLS implementations would conflict at the datapath level;
  Cilium's eBPF datapath already covers basic L7 needs without Envoy's overhead.
- **Verification:** Inspection (Cilium WireGuard is enabled cluster-wide; no second mTLS
  implementation is active concurrently).

#### INFRA-008 — Gateway API only, no legacy Ingress

- **Statement:** Ingress traffic MUST be configured via Gateway API. Legacy Ingress resources
  and proprietary mesh CRDs MUST NOT be used.
- **Rationale:** Gateway API is the forward-looking config surface; the Cilium/eBPF dataplane
  still handles basic L7 without needing Envoy, which only reappears (embedded) for advanced
  L7 features.
- **Verification:** Inspection (no `Ingress` resources exist in the cluster; routing is
  expressed via Gateway API resources).

#### INFRA-009 — Deny-all NetworkPolicies by default

- **Statement:** NetworkPolicies MUST default to deny-all; flows MUST be opened individually.
  DNS (CoreDNS) and controlled egress MUST be included among the explicitly opened flows.
- **Rationale:** Zero-trust-by-default networking; forgetting the DNS exception is a
  well-known way to silently break an entire namespace, so it must be handled explicitly, not
  implicitly.
- **Verification:** Test (a pod with no explicit policy cannot reach another namespace) +
  Inspection (DNS egress is explicitly allowed for every namespace).

#### INFRA-010 — Tenant hard isolation provisions a complete namespaced stack via GitOps

- **Statement:** Tenant hard isolation MUST be provisioned by a GitOps manifest that creates,
  per tenant: a namespace, deny-all cross-namespace NetworkPolicies, a dedicated CNPG cluster,
  a dedicated NATS stream, isolated RustFS storage/secrets/DEKs, and resource quotas (plus an
  optional tagged nodepool for premium tenants). Onboarding a tenant MUST reduce to a single
  `git commit`.
- **Rationale:** Namespace-per-tenant is the SaaS-scale silo model (a cluster per customer does
  not scale); making onboarding a single commit keeps the process auditable and repeatable.
- **Verification:** Demonstration (onboard a new tenant via one commit; assert the full stack —
  namespace, policies, CNPG, NATS stream, RustFS isolation, quotas — is provisioned).
- **Depends on:** SEC-003, INFRA-009

#### INFRA-011 — Continuous WAL streaming for minutes-scale RPO, backups off-cluster

- **Statement:** Backups MUST use continuous WAL streaming (not nightly dumps), yielding an RPO
  of minutes. Restore MUST use the latest snapshot plus WAL replay. RPO and RTO MUST be
  documented separately. Restore MUST maintain consistency between Postgres and the Object
  Store (no orphaned `input_ref`/`output_ref`). WAL and backups MUST be stored off-cluster and
  off-provider.
- **Rationale:** A nightly dump gives a far worse RPO than continuous WAL streaming; consistency
  between Postgres and object storage at restore time avoids orphaned references that would
  otherwise silently corrupt execution history.
- **Verification:** Demonstration (full restore drill: measure actual RPO/RTO, verify no
  orphaned `input_ref`/`output_ref` after restore).
- **Depends on:** INFRA-004, DAT-004

#### INFRA-012 — Transactional email via a third-party SMTP API, not an on-cluster mail server

- **Statement:** Transactional email (HITL presigned links, alerts) MUST be sent via a
  third-party SMTP API service (e.g. Postmark, Resend, TEM). The engine MUST NOT run a mail
  server on-cluster.
- **Rationale:** Port 25 is commonly blocked by cloud providers, and running a mail server adds
  deliverability and abuse-management burden better handled by a specialized provider.
- **Verification:** Inspection (email delivery code path calls a third-party API, no SMTP
  daemon runs in-cluster).
- **Depends on:** LIFE-019

#### INFRA-013 — Minimum viable topology and additive growth

- **Statement:** At launch, the cluster MUST run at least 3 mutualized nodes (K8s control
  plane + etcd + application control plane + NATS + CNPG + RustFS) to satisfy etcd quorum and
  CNPG anti-affinity simultaneously. Growth MUST be additive (join dedicated nodes, add taints,
  drain shared nodes) and MUST NOT require a topology redesign.
- **Rationale:** Three nodes is the minimum that satisfies both quorum and anti-affinity at
  once; designing growth as additive keeps early-stage budget constraints from becoming
  technical debt later.
- **Verification:** Inspection (initial deployment manifest provisions at least 3 nodes meeting
  both constraints; a documented growth runbook adds capacity without redeploying the base
  topology).
- **Depends on:** INFRA-002, INFRA-003

#### INFRA-014 — Tenant quiesce/migration job reuses hierarchical kill

- **Statement:** A tenant quiesce-and-migration job MUST reuse the hierarchical kill mechanism
  (`LIFE-017`) to freeze a tenant cleanly. Cold migration (quiesce-based) MUST be the default
  approach; hot migration (logical replication) MUST be implemented only when a customer
  requires it.
- **Rationale:** The same job serves soft-to-dedicated upsell, regional portability, and
  erasure-request purges, so it is worth building once, correctly, on top of the existing kill
  primitive rather than as a bespoke mechanism.
- **Verification:** Demonstration (quiesce a tenant, confirm no in-flight executions continue,
  migrate its stack, resume).
- **Depends on:** LIFE-017, INFRA-010

---

## Non-normative note: storage device choice

Data-plane instances are ideally backed by local NVMe (POP2-like) or network block storage
(survives node replacement, simpler operationally, ~5k IOPS). Starting with block storage is
the pragmatic default — one less operational concern while the platform is small.

