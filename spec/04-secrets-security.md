# 04 — Secrets & taint

See `spec/CONVENTIONS.md` for the requirement format and the `SEC-NNN` ID scheme.

---

#### SEC-001 — The engine manipulates references only

- **Statement:** The engine MUST manipulate `secretRef` values only. Compiled IR, logs, and
  execution state persisted in the database MUST NOT contain secret material. Secret material
  MUST exist only in host memory, at the moment of an I/O call, and MUST be discarded
  afterwards.
- **Rationale:** This is the foundation the rest of this file builds on: if a reference can
  never resolve to material outside host memory, most leak paths are closed structurally.
- **Verification:** Inspection (grep-style audit: no code path stores a resolved secret value
  in the IR, logs, or a database column) + Test (attempt to persist a resolved secret; assert
  the type system rejects it — see `SEC-006`).
- **Depends on:** DAT-012, INV-007

#### SEC-002 — `SecretProvider` interface

- **Statement:** The engine MUST resolve secrets through a `SecretProvider` interface exposing
  `resolve(ref) -> material`, where `material` exists only ephemerally in host memory.
- **Rationale:** Making storage a replaceable detail behind this interface lets the backend
  evolve (Postgres, Vault/OpenBao) without touching engine code.
- **Verification:** Inspection (all secret resolution goes through this interface; no direct
  database/API call to a secret store bypasses it).

#### SEC-003 — Default backend uses envelope encryption

- **Statement:** The default `SecretProvider` backend (Postgres) MUST use envelope encryption:
  a key-encryption-key (KEK) held in a KMS/HSM; a per-tenant data-encryption-key (DEK)
  encrypted by the KEK; AEAD (AES-GCM or XChaCha20-Poly1305) with tenant ID and key version
  carried as AAD. A single AES column with a key stored in an environment variable MUST NOT be
  used.
- **Rationale:** This design allows KEK rotation without re-encrypting all secrets, and gives
  cryptographic isolation per tenant.
- **Verification:** Test (rotate the KEK; assert existing DEKs remain decryptable without
  re-encrypting secret rows) + Inspection (AAD includes tenant ID and key version).
- **Depends on:** SEC-002

#### SEC-004 — Vault/OpenBao as an optional backend

- **Statement:** Vault or OpenBao MAY be used as an alternate `SecretProvider` backend for
  dynamic, short-TTL, leased, and audited secrets, implemented behind the same interface.
- **Rationale:** OpenBao is an OSI-licensed fork aligned with the project's licensing posture;
  this is a heavy operational dependency, so it is optional, not a v1 prerequisite.
- **Verification:** Inspection (if implemented, this backend conforms to the `SecretProvider`
  interface from `SEC-002` with no engine-side special-casing).
- **Depends on:** SEC-002

#### SEC-005 — ESO/CSI is for platform secrets only, never tenant secrets

- **Statement:** External Secrets Operator (ESO) / CSI-driven Kubernetes secrets MUST be used
  only for platform secrets (database credentials, KMS keys), never for tenant credentials.
  Platform secrets (ESO/CSI) and tenant secrets (`SecretProvider`) MUST NOT be handled by the
  same mechanism.
- **Rationale:** Confusing the platform's own secret plane with the tenant-facing
  `SecretProvider` would blur a security boundary that must stay distinct.
- **Verification:** Inspection (platform secrets are provisioned via ESO/CSI manifests; tenant
  secret resolution never reads from a Kubernetes `Secret` object).

#### SEC-006 — `Secret<T>` renders `[REDACTED]`

- **Statement:** `Secret<T>`'s `Display`, `Debug`, and `toString` implementations MUST render
  `[REDACTED]` unconditionally.
- **Rationale:** This makes a secret structurally unloggable — a guarantee, not a best-effort
  practice.
- **Verification:** Test (serialize/format a `Secret<T>` value through every standard
  formatting path; assert `[REDACTED]` in every case).
- **Depends on:** DAT-012, INV-007

#### SEC-007 — `Secret<T>` unwraps only via the host I/O capability

- **Statement:** `Secret<T>` MUST be unwrappable only by the host's I/O capability, at the last
  possible moment before use.
- **Rationale:** Restricting the unwrap path to one auditable choke point is what makes
  `SEC-001` enforceable rather than aspirational.
- **Verification:** Inspection (the unwrap method/function is private to the host I/O
  capability module; no other code path can extract the inner value).
- **Depends on:** DAT-012, INV-007

#### SEC-008 — Taint is contagious

- **Statement:** A value derived from a `Secret<T>` MUST itself be tainted; taint MUST
  propagate through transformations rather than being lost at the first derivation.
- **Rationale:** Without contagious taint, a trivial transformation (e.g. string
  concatenation) would silently launder a secret into a loggable value.
- **Verification:** Test (derive a value from a `Secret<T>` via a transformation the engine
  supports; assert the result is still tainted and renders `[REDACTED]`).
- **Depends on:** DAT-012, INV-007, SEC-006

#### SEC-009 — Schema-directed redaction for sensitive business fields

- **Statement:** A node's output schema MAY declare `sensitive: true` per field; when declared,
  the engine MUST redact that field independently of the `Secret<T>` mechanism.
- **Rationale:** Not every PII field can be auto-tainted; schema-directed redaction is the
  deliberate, declared alternative to guessing.
- **Verification:** Test (a field marked `sensitive: true` is redacted in logs/exports even
  though its runtime value is a plain, non-`Secret<T>` type).
- **Depends on:** EXE-006

#### SEC-010 — Full payload capture is opt-in, per-tenant

- **Statement:** Capturing the full input/output payload of a node MUST be an explicit,
  per-tenant opt-in setting. It MUST NOT be enabled by default.
- **Rationale:** Logging every payload by default is a data-protection liability; opt-in keeps
  the default posture safe.
- **Verification:** Inspection (default tenant configuration has payload capture disabled;
  enabling it requires an explicit per-tenant setting change).

#### SEC-011 — `egress_scope` is per-credential, present from v1

- **Statement:** `connections.egress_scope` MUST declare the credential's authorized
  destination domain(s) and MUST be enforced per-credential, never globally. This field MUST
  exist from v1.
- **Rationale:** A global egress allowlist would not close exfiltration for a specific
  compromised credential; retrofitting a per-credential field later is expensive.
- **Verification:** Inspection (schema: `egress_scope` is a column on `connections`, not a
  global configuration value).
- **Depends on:** DAT-007, PROTO-004, INV-010

#### SEC-012 — Output-scanning regex as defense-in-depth

- **Statement:** The engine SHOULD run a prefilter that scans outgoing log/output values against
  a regex of known secret patterns, in addition to structural taint tracking.
- **Rationale:** This is a backstop for cases the type system cannot cover (e.g. a secret value
  copy-pasted into a plain string by a misbehaving node); it is not the primary mechanism.
- **Verification:** Test (a known secret pattern embedded in a non-tainted string is flagged by
  the prefilter).
- **Depends on:** SEC-006, SEC-007, SEC-008

#### SEC-013 — Host owns the OAuth refresh flow

- **Statement:** The host MUST own the OAuth refresh flow and MUST update the stored token. A
  node MUST NOT participate in the credential lifecycle at any point.
- **Rationale:** Keeps refresh-token handling inside the trusted host boundary, consistent with
  `PROTO-010`.
- **Verification:** Inspection (no node-facing API exposes a refresh-token operation).
- **Depends on:** PROTO-010
