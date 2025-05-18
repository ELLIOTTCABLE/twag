CREATE TABLE IF NOT EXISTS "twag_tags" (
   "id" char(14) UNIQUE NOT NULL PRIMARY KEY,
   "target_url" text NOT NULL,
   "created_at" timestamp with time zone DEFAULT current_timestamp,
   "updated_at" timestamp with time zone DEFAULT current_timestamp,
   "last_accessed" timestamp with time zone,
   "access_count" integer DEFAULT 0,
   "last_seen_tap_count" integer
);
