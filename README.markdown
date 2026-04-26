twag
====

A tiny URL-redirector for physical belongings.

Deployment
----------

### Database

Prereq: a PostgreSQL database named `twag` with two users;

1. `twag_owner`: a DDL role with database ownership for setup and migrations, and
2. `twag_app`: a RW role for all runtime access.

First-time provisioning requires running [grants.sql][provisioning/grants.sql];
thereafter a `sqlx migrate run` that will deploy the database schema.

For local development, convenience [mise][] tasks are provided that run
these against a [Neon][] 'serverless Postgres' project, which is a free
and quick way to deploy a small database. Create a Neon project, a
`twag` database, owned by `twag_owner`; then run:

```console
mise run neon:create  # or `neonctl set-context --project-id [e.g. your-project-50125040]`
mise run neon:provision ::: neon:migrate ::: neon:auth
mise run watch
```

   [mise]: <https://mise.jdx.dev/> "Language runtime/tooling and automation management tool"
   [Neon]: <https://neon.com/> "Fast, developer-friendly Postgres host with versioning/branching/rollback"
