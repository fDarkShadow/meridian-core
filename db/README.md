# Database

Migrations are managed by [dbmate](https://github.com/amacneil/dbmate) against Postgres 18.
Schema documentation is generated from the live schema by [tbls](https://github.com/k1LoW/tbls)
into `docs/db/`.

Every table and column here must satisfy `spec/02-data-model.md` — in particular, the reserved
columns marked 🔒 are non-negotiable from the first migration that creates their table
(`DAT-001`…`DAT-013`). One requirement issue = one migration = one PR; do not fold multiple
`DAT-*` requirements into a single migration unless they are inseparable at the schema level
(see `spec/CONVENTIONS.md`).

## Commands (via Taskfile)

| Command | Effect |
|---|---|
| `task db:new -- <name>` | Create a new timestamped migration file in `db/migrations/` |
| `task db:up` | Apply pending migrations |
| `task db:down` | Roll back the last migration |
| `task db:status` | Show migration status |
| `task db:dump` | Dump the current schema to `db/schema.sql` |
| `task db:docs` | Regenerate `docs/db/` from the live schema via tbls |
| `task db:docs:check` | Fail if committed docs have drifted from the live schema |

`DATABASE_URL` is read from `.env` (copy `.env.example`). `task setup` starts Postgres via
`docker-compose.yml` and applies migrations.
