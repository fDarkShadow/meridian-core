# 10 — Licensing & rights governance

See `spec/CONVENTIONS.md` for the requirement format and the `LIC-NNN` ID scheme.

> **Not legal advice.** The exact drafting (Additional Use Grant wording, CLA text, the
> derivative-work vs. SDK-usage boundary) is the responsibility of an OSS-specialized lawyer.
> This file fixes the *intent* and the parts that affect code (headers, provenance).

---

#### LIC-001 — BSL from day zero, single edition, no feature gating

- **Statement:** The project MUST be licensed under the Business Source License (BSL) from its
  first release. There MUST be a single edition with no closed-source Enterprise/Community
  split — audit, SSO via Keycloak, and multi-tenancy MUST all be included.
- **Rationale:** Monetization targets the managed platform and commercial resale-as-a-service,
  never feature unlocking; adopting BSL from day zero avoids the reputational cost of later
  relicensing an already-open project.
- **Verification:** Inspection (LICENSE file is BSL from the first tagged release; no
  feature-flagged "enterprise" build exists).

#### LIC-002 — Additional Use Grant permits free internal/dev/embedding use

- **Statement:** The BSL Additional Use Grant MUST permit free internal use, development use,
  and embedding. It MUST require a commercial license only for usage that offers the product
  as a service to third parties.
- **Rationale:** This is the precise scalpel that keeps the project maximally open for
  everything except reselling it as a competing hosted service.
- **Verification:** Inspection (Additional Use Grant text matches this scope exactly).
- **Depends on:** LIC-001

#### LIC-003 — Change Date is 4 years after each release, rolling per version

- **Statement:** The BSL Change Date for each release MUST be set to 4 years after that
  release's date, rolling independently per version (recent releases stay under BSL while
  older ones become freely licensed on schedule).
- **Rationale:** A rolling per-version Change Date is what makes "open source at a fixed date"
  true for every release, not just the first one.
- **Verification:** Inspection (each release's LICENSE metadata states a Change Date exactly 4
  years after that release).
- **Depends on:** LIC-001

#### LIC-004 — Change License is Apache 2.0

- **Statement:** The BSL Change License MUST be Apache 2.0.
- **Rationale:** Apache 2.0 gives CNCF compatibility in advance for code that has rolled over
  to the open license.
- **Verification:** Inspection (LICENSE metadata names Apache 2.0 as the Change License).
- **Depends on:** LIC-001

#### LIC-005 — CLA required from the first external contributor

- **Statement:** The project MUST require a Contributor License Agreement (CLA), not a DCO,
  starting from the first external contribution.
- **Rationale:** Retrofitting a CLA after external contributions have accumulated is
  impractical; a CLA (unlike a DCO) grants the right to relicense, which the BSL strategy
  depends on.
- **Verification:** Inspection (CLA-signing is enforced by CI/bot before a first-time external
  contributor's PR can merge).

#### LIC-006 — CLA includes an AI-provenance clause

- **Statement:** The CLA MUST include a clause under which the contributor (a) attests they
  have the right to contribute the content, (b) discloses whether the contribution is
  significantly AI-generated, and (c) confirms the absence of a problematic license dependency.
- **Rationale:** An AI model can reproduce copyleft material from its training data; explicit
  disclosure and attestation is a control against contaminating the codebase's licensing
  posture.
- **Verification:** Inspection (CLA text includes all three attestations).
- **Depends on:** LIC-005

#### LIC-007 — Every PR receives substantive human review before merge

- **Statement:** Every pull request MUST receive substantive, traceable human review before
  merge. This rule MUST NOT be bypassed for any contributor, including maintainers.
- **Rationale:** Purely machine-generated output may be protected by no one's copyright; human
  review is what turns "machine output" into "human-directed work" for copyright purposes,
  which the entire BSL/CLA strategy depends on.
- **Verification:** Inspection (branch protection requires at least one human review approval
  before merge, with no bot-approval exception).

#### LIC-008 — CI runs a license/SCA scan; incompatible copyleft is never introduced

- **Statement:** CI MUST run a software composition analysis (license) scan on every PR.
  Dependencies under a copyleft license incompatible with the project's licensing MUST NOT be
  introduced.
- **Rationale:** An AI coding agent can regurgitate copyleft-licensed training data; scanning in
  CI is the provenance control that catches this before merge.
- **Verification:** Test (CI fails a PR that introduces a dependency flagged as incompatible
  copyleft).
- **Depends on:** LIC-007

#### LIC-009 — Every source file carries the standard BSL header

- **Statement:** Every source file MUST carry the standard BSL header (license, Change Date,
  Change License), generated from a repo scaffolding template.
- **Rationale:** A per-file header makes each file's license terms self-evident without relying
  on a single root LICENSE file being consulted correctly.
- **Verification:** Test (CI lints for the presence of the standard header in every source
  file, failing the build if one is missing).
- **Depends on:** LIC-001, LIC-003, LIC-004
