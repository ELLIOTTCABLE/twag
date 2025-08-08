## twag – AI Contributor Instructions

This file guides contributors and AI agents on how to think, design, and implement changes in this repository.

### 1) Application description and user flows

Audience: non‑technical family members. The goal is a near‑invisible physical→digital bridge that sends people to the right Notion page fast and reliably.

-  Redirect: an NFC/QR tap on a sticker must yield an immediate HTTP redirect to the corresponding Notion page. Lack of a local cache entry must not block.
-  Creation: today’s creation form is temporary while capabilities mature; long‑term, adding tags should happen via Notion edits, not bespoke UI.
-  Move (planned): tap a belonging, then within ~2 minutes tap a container; the belonging→container relation is updated in Notion; show a brief confirmation interstitial; then redirect to the container’s page.
-  Undo (planned): belonging tap → container tap (accidental) → second container tap within the window reverts the prior containment.
-  Failure/UI: HTML should appear only when necessary (mutation acknowledgment, clear failure with one actionable hint). Prefer fast redirect over waiting for upstream verification.

Implementation priorities: Correctness > Maintainability > Simplicity > Performance (unless latency threatens user experience). If a small increase in structure (e.g., a newtype) materially strengthens correctness or preserves invariants, it’s encouraged.

### 2) Architectural and engineering directives

System shape: single Axum binary (`src/main.rs`) with domain/parsing types in `src/models.rs`. Postgres stores augmentation/cache (`twag_tags`) only; Notion remains authoritative for objects and containment.

-  Startup order is intentional and fail‑fast: read env → init tracing → connect Postgres → init Notion client → validate required relations → build router → serve. Preserve this order to surface issues early.
-  Identifier types: `Hex14` (uppercase 14‑char hex) and `NotionPageId` (accepts bare UUID, hyphenated UUID, and Notion URLs; normalizes to lowercase hyphenated UUID). Normalize at construction; don’t pass raw strings past boundaries. New constrained IDs must get strict constructors and, if persisted, a DB domain mirroring rules.
-  Notion vs Postgres: Notion defines items and containment. Postgres can cache and store auxiliary timestamps/counters or ephemeral windows. Do not let Postgres become an independent truth for containment. Reconcile after redirect or out‑of‑band.
-  Multi‑tap mutation (planned): treat as a short-lived state machine (~2 minutes) with minimal state (signed cookie or ephemeral memory). Second tap mutates relation in Notion; interstitial acknowledges; third tap within window undoes. Expiration should fail silent and harmlessly.
-  Routing & extraction: maintain a single authoritative regex for `TAGID` with optional `xTAPCOUNT`. Parse/validate at the boundary into strong types; handlers should perform minimal DB access (single query) and decide redirect vs mutation vs creation scaffolding.
-  Database & migrations: plain, timestamped SQL. Use Postgres domains to encode formats and cast to them on insert (e.g., `$1::hex_14`). Avoid ORMs; explicit SQL keeps constraints visible.
-  Logging & tracing: `tracing` with format-driven verbosity. Prefer stable structured fields (e.g., `tag_id`, `container_id`, `phase`) to ease future OpenTelemetry export. Avoid leaking sensitive Notion data at higher log levels.
-  Error behavior: the redirect path should still redirect unless an invariant is definitively broken. Auxiliary failures (logging/enqueue) are best-effort. Mutation flows may return minimal, phone-friendly acknowledgments.
-  Feature evolution: classify changes by impact (redirect latency vs mutation vs background). Avoid adding synchronous I/O to the hot path; reconcile after redirect. Introduce newtypes/domains early for new IDs. Add explicit SQL migrations for schema changes. Add tight unit tests for new parsing/normalization or failure branches. Refactor only after repetition is proven.
-  Performance: throughput is irrelevant; optimize only user‑visible tap→redirect latency. Avoid speculative caches beyond Postgres until tracing demonstrates a bottleneck.
-  Security & auth: open by default. Keep mutation logic factored so auth can be layered later without disturbing ID parsing or redirect.

### 3) Code style, taste, and preferences

-  An absent comment is better than an unnecessary comment. Comments (when present at all) should explain “why,” not “what” or “how.” Prefer clear variable/function names and self-documenting code practices over inline commentary.
-  Normalize and validate at boundaries; inside the code, assume invariants. Prefer small, explicit functions over generic abstractions until repetition is real.
-  Tests should be explicit and W.E.T., located with the types they exercise (e.g., `models.rs`). Add fuzz/property tests where they materially increase confidence in parsers/normalizers. Defer E2E until the multi‑tap state or background reconciliation exists.
-  Ask before adding dependencies, background job systems, caching layers, or changing Notion schema assumptions. Justify via user‑friction reduction, stronger invariants, or cross‑project maintainability.
-  Documentation should live here. Keep inline comments to non‑obvious invariants (ordering, expiry handling). Keep the UI minimal; only surface HTML when necessary.
-  Indentation: three spaces per level. If an existing file uses a different style, match local style to avoid unrelated diffs; prefer three spaces for new files.

Open areas (don’t implement without discussion): multi‑tap mutation + undo mechanics; post‑redirect reconciliation strategy (poll/webhook/hybrid); structured tracing span taxonomy and OpenTelemetry export; minimal mobile interstitial styling.
