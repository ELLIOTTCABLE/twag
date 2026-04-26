-- Grants the runtime app role (`twag_app`) the privileges it needs against
-- objects owned by the schema owner (`twag_owner`). Idempotent.
--
-- Run ONCE per environment, AS `twag_owner`, BEFORE applying any migration.
-- `ALTER DEFAULT PRIVILEGES` only affects objects created *after* it runs,
-- so applying it ahead of migrations lets every table/sequence/type the
-- migrations subsequently create auto-grant to `twag_app` without further
-- per-object GRANTs.

GRANT USAGE ON SCHEMA "public" TO "twag_app";

ALTER DEFAULT PRIVILEGES FOR ROLE "twag_owner" IN SCHEMA "public"
GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO "twag_app";
ALTER DEFAULT PRIVILEGES FOR ROLE "twag_owner" IN SCHEMA "public"
GRANT USAGE, SELECT ON SEQUENCES TO "twag_app";
ALTER DEFAULT PRIVILEGES FOR ROLE "twag_owner" IN SCHEMA "public"
GRANT USAGE ON TYPES TO "twag_app";
