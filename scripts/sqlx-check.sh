#!/bin/sh
# Pre-commit guard for .sqlx cache freshness. Only a *reachable* DB plus a
# failing cargo check is a real failure; missing env or unreachable host pass.

if [ -z "${DATABASE_URL}" ]; then
   printf %s\\n "sqlx-check: skipped — DATABASE_URL unset. Run \`mise run neon:auth\` (or set it manually) to enable cache-verification." >&2
   exit 0
fi

if ! PGCONNECT_TIMEOUT=3 psql "$DATABASE_URL" -tAc 'select 1' >/dev/null 2>&1; then
   printf %s\\n "sqlx-check: skipped — DB unreachable. Run \`mise run db:cache-typechecking\` once online if queries changed." >&2
   exit 0
fi

exec cargo sqlx prepare --check
