# Meridian

A durable, source-available workflow orchestration engine. A workflow is data — a declarative
graph, interpreted — never code.

Read `CLAUDE.md` first, then the relevant `spec/*.md` domain file before writing code. Every
requirement in `spec/` is EARS/RFC 2119-formatted with a stable ID and maps 1:1 to a GitHub
issue and then one PR — see `spec/CONVENTIONS.md`.

## Toolchain

| Concern | Tool |
|---|---|
| Language | Rust (pinned via `rust-toolchain.toml`) |
| Monorepo task graph | [moon](https://moonrepo.dev) (`.moon/`, per-crate `moon.yml`) |
| Developer-facing commands | [Task](https://taskfile.dev) (`Taskfile.yml`) |
| Database | Postgres 18 (`docker-compose.yml`) |
| Migrations | [dbmate](https://github.com/amacneil/dbmate) (`db/migrations/`) |
| Schema docs | [tbls](https://github.com/k1LoW/tbls), generated into `docs/db/` |
| License / SCA scan | [cargo-deny](https://github.com/EmbarkStudios/cargo-deny) (`deny.toml`) |
| Container images | Docker Bake (`docker-bake.hcl`) → rootless, distroless (`docker/`) |
| CI | GitHub Actions (`.github/workflows/ci.yml`) |

## Getting started

```sh
cp .env.example .env
task setup     # starts Postgres 18, applies migrations, builds the workspace
task ci        # lint + test + license scan + schema-docs-drift check, same as CI
```

Common commands (`task --list` for the full set): `task build`, `task test`, `task lint`,
`task fmt`, `task db:new -- <name>`, `task db:up`, `task db:docs`, `task docker:build`.

## Repository layout

```
crates/            Rust workspace members (Cargo workspace, orchestrated by moon)
db/migrations/      dbmate migrations — one DAT-* requirement's schema change per migration
docs/db/            tbls-generated schema documentation (regenerate with `task db:docs`)
docker/             Dockerfiles (multi-stage: build on Debian, run distroless + nonroot)
spec/               Requirements (EARS/RFC 2119), one stable ID per normative statement
```

## Contributing

Every PR implements exactly one `spec/` requirement ID (or an inseparable group — see
`spec/CONVENTIONS.md`), states which verification method it satisfies, and is reviewed and
merged by hand. No auto-merge; see `spec/10-licensing.md` (`LIC-007`) for why.
