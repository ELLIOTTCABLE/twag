If user is repeatedly offering guidance on how to behave that isn't covered in this file, ask them if they'd like that guidance here added so it remains consistent; but deeply research *current* prompting guidelines before modifying this file.

### 1) Application description and user flows

Audience: non‑technical family members. The goal is a near‑invisible physical→digital bridge that sends people to the right Notion page fast and reliably.

- Redirect: an NFC/QR tap on a sticker must yield an immediate HTTP redirect to the corresponding Notion page. Lack of a local cache entry must not block.
- Creation: today’s creation form is temporary while capabilities mature; long‑term, adding tags should happen via Notion edits, not bespoke UI.
- Move (planned): tap a belonging, then within ~2 minutes tap a container; the belonging→container relation is updated in Notion; show a brief confirmation interstitial; then redirect to the container’s page.
- Undo (planned): belonging tap → container tap (accidental) → second container tap within the window reverts the prior containment.
- Failure/UI: HTML should appear only when necessary (mutation acknowledgment, clear failure with one actionable hint). Prefer fast redirect over waiting for upstream verification.

Implementation priorities: Correctness > Maintainability > Simplicity > Performance (unless latency threatens user experience). If a small increase in structure (e.g., a newtype) materially strengthens correctness or preserves invariants, it’s encouraged.

### 2) Architectural and engineering directives

System shape: single Axum binary (`src/main.rs`) with domain/parsing types in `src/models.rs`. Postgres stores augmentation/cache (`twag_tags`) only; Notion remains authoritative for objects and containment.

- Startup order is intentional and fail‑fast: read env → init tracing → connect Postgres → init Notion client → validate required relations → build router → serve. Preserve this order to surface issues early.
- Identifier types: `Hex14` (uppercase 14‑char hex) and `NotionPageId` (accepts bare UUID, hyphenated UUID, and Notion URLs; normalizes to lowercase hyphenated UUID). Normalize at construction; don’t pass raw strings past boundaries. New constrained IDs must get strict constructors and, if persisted, a DB domain mirroring rules.
- Notion vs Postgres: Notion defines items and containment. Postgres can cache and store auxiliary timestamps/counters or ephemeral windows. Do not let Postgres become an independent truth for containment. Reconcile after redirect or out‑of‑band.
- Multi‑tap mutation (planned): treat as a short-lived state machine (~2 minutes) with minimal state (signed cookie or ephemeral memory). Second tap mutates relation in Notion; interstitial acknowledges; third tap within window undoes. Expiration should fail silent and harmlessly.
- Routing & extraction: maintain a single authoritative regex for `TAGID` with optional `xTAPCOUNT`. Parse/validate at the boundary into strong types; handlers should perform minimal DB access (single query) and decide redirect vs mutation vs creation scaffolding.
- Postgres & migrations: plain, timestamped SQL. Use Postgres domains to encode formats and cast to them on insert (e.g., `$1::hex_14`). Avoid ORMs; explicit SQL keeps constraints visible.
- Logging & tracing: `tracing` with format-driven verbosity. Prefer stable structured fields (e.g., `tag_id`, `container_id`, `phase`) to ease future OpenTelemetry export. Avoid leaking sensitive Notion data at higher log levels.
- Error behavior: the redirect path should still redirect unless an invariant is definitively broken. Auxiliary failures (logging/enqueue) are best-effort. Mutation flows may return minimal, phone-friendly acknowledgments.
- Feature evolution: classify changes by impact (redirect latency vs mutation vs background). Avoid adding synchronous I/O to the hot path; reconcile after redirect. Introduce newtypes/domains early for new IDs. Add explicit SQL migrations for schema changes. Add tight unit tests for new parsing/normalization or failure branches. Refactor only after repetition is proven.
- Performance: throughput is irrelevant; optimize only user‑visible tap→redirect latency. Avoid speculative caches beyond Postgres until tracing demonstrates a bottleneck.
- Security & auth: open by default. Keep mutation logic factored so auth can be layered later without disturbing ID parsing or redirect.

### 3) Code style, taste, and preferences

- An absent comment is better than an unnecessary comment. Comments (when present at all) should explain “why,” not “what” or “how.” Prefer clear variable/function names and self-documenting code practices over inline commentary.
- Normalize and validate at boundaries; inside the code, assume invariants. Prefer small, explicit functions over generic abstractions until repetition is real.
- Tests should be explicit and W.E.T., located with the types they exercise (e.g., `models.rs`). Add fuzz/property tests where they materially increase confidence in parsers/normalizers. Defer E2E until the multi‑tap state or background reconciliation exists.
- Ask before adding dependencies, background job systems, caching layers, or changing Notion schema assumptions. Justify via user‑friction reduction, stronger invariants, or cross‑project maintainability.
- Documentation should live here. Keep inline comments to non‑obvious invariants (ordering, expiry handling). Keep the UI minimal; only surface HTML when necessary.
- Indentation: three spaces per level. If an existing file uses a different style, match local style to avoid unrelated diffs; prefer three spaces for new files.

Open areas (don’t implement without discussion): multi‑tap mutation + undo mechanics; post‑redirect reconciliation strategy (poll/webhook/hybrid); structured tracing span taxonomy and OpenTelemetry export; minimal mobile interstitial styling.

### 4) Contribution / commit hygiene

- Prefer granular, single‑concern commits. Each commit should introduce exactly one logical change (e.g., cache env vars in `AppState` separate from adding Notion lookup enums).
- Do not mix refactors with behavior changes unless inseparable; if inseparable, document why in the commit body.
- Preserve narrative history; avoid squashing unless the intermediate steps add zero archaeological value.
- Stage intentionally: review `git diff --staged` before committing to ensure scope purity.

Commit messages should be short, with an imperative subject line and optional body paragraphs. The subject should be very concise, ideally under 50 characters including the labels, and the body should explain the "why" behind the change (if necessary; this is rarely used in practice, as code should be self-documenting).

This repository uses the `.gitlabels` system (see root `.gitlabels`) rather than Conventional Commits. Commit subjects begin with a parenthetical label block containing one or more space‑separated labels (and optional payloads) describing orthogonal facets of the change, e.g.:

```
(AI new SQL) Add initial migration for tag storage
```

Key points:

- Multiple labels are normal; choose all that materially describe the change (feature vs refactor vs infra vs sql, etc.). Order from broader → more specific.
- Low‑impact change can include `(-)` in addition to other labels to allow consumers to filter it out. Since we like small, granular commits, about 60-80% of commits should start with `(- ...`.
- ALWAYS include the label `(AI)` with your changes. This helps distinguish AI contributions from human ones.
- Do NOT use Conventional Commit forms like `feat:` / `fix:` – they are incompatible noise here.
- Split commits if you are tempted to use disjoint label clusters (e.g. `(new)` plus `(re)`); each commit should remain single‑concern.
- Read and respect the canonical aliases and hierarchy in `.gitlabels`; add new labels only via a dedicated, reviewed commit amending that file.

Minimal template for a new commit:

```
(AI label1 label2 ...) Imperative summary of what changed

<Optional longer explanatory body paragraphs as needed>
```

When in doubt: re‑read `.gitlabels` and choose the smallest sufficient label set. If uncertain between `(re)` vs `(fix)`: ask or split.
