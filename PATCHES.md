# Patches against `neon-mmd/websurfx`

This fork (`nadecancode/tinysurfx`) is maintained as a small set of patches on top of upstream `neon-mmd/websurfx` `rolling`. New patches must be added here when introduced; obsolete patches must be removed.

When merging upstream (`.github/workflows/upstream-sync.yml` opens a PR weekly), the reviewer must verify each row below still applies cleanly. Update commit shas after squashes/rebases.

## Active patches

### P1 — api-only Cargo feature + tinysurfx bin
- **Files:** `Cargo.toml`, `src/lib.rs` (cfg gates only), `src/bin/tinysurfx.rs`
- **Purpose:** Adds api-only build target with no UI deps. Existing websurfx bin unchanged. Both bins now have `required-features` for symmetry.
- **Upstream conflict risk:** low — additive cfg gates and a new bin entry.
- **Verify after rebase:** `cargo build --bin tinysurfx --no-default-features --features api-only` succeeds AND `cargo build` (default) still succeeds.

### P2 — env/CLI config (api_config.rs)
- **Files:** `src/api_config.rs`, `src/api_config_help.txt`, `src/lib.rs` (the `pub mod parser { pub use crate::api_config::Config; }` shim under `cfg(api-only)`)
- **Purpose:** Replaces mlua-based `parser::Config` with env vars + CLI flags for api-only builds.
- **Upstream conflict risk:** medium — if upstream adds a `parser::Config` field that `aggregator` reads, mirror it in `api_config::Config`.
- **Verify after rebase:** binary boots with no `config.lua` present; `cargo test --test api_only --no-default-features --features api-only -- --test-threads=1` passes.

### P3 — JSON-only API server (api_server.rs)
- **Files:** `src/api_server.rs`, `src/lib.rs` (one new `pub mod api_server;` under cfg), `tests/api_only.rs`, `tests/index.rs` (cfg-gate to keep upstream test from compiling under api-only).
- **Purpose:** Actix App + `/search` (JSON) and `/healthz` handlers. Integration tests in `tests/api_only.rs`.
- **Upstream conflict risk:** low — isolated new files. The `tests/index.rs` cfg-gate is one line.
- **Verify after rebase:** `cargo test --test api_only --no-default-features --features api-only -- --test-threads=1` passes; `cargo test` (default) also passes.

### P4 — `build-binaries.yml` workflow
- **Files:** `.github/workflows/build-binaries.yml`
- **Purpose:** 5-target build matrix on every push; release publish on tag pushes.
- **Upstream conflict risk:** low — new file.
- **Verify after rebase:** workflow passes for at least one target.

### P5 — `upstream-sync.yml` workflow
- **Files:** `.github/workflows/upstream-sync.yml`
- **Purpose:** Weekly cron-driven `git merge upstream/rolling` PR.
- **Upstream conflict risk:** low — new file.
- **Verify after rebase:** workflow_dispatch run completes.

### P6 — Gate relevance scoring (stop-words + keyword_extraction)
- **Files:** `src/aggregator.rs` (one cfg-gated line at the `value.calculate_relevance(...)` call), `src/models/aggregation.rs` (cfg-gated `calculate_relevance` impl + `calculate_tf_idf` free fn)
- **Purpose:** Removes `stop-words` + `keyword_extraction` deps from the api-only graph. Search results returned in upstream order, no TF-IDF re-ranking.
- **Upstream conflict risk:** **high** — touches actively-evolving upstream files.
- **Known cosmetic effect:** with the cfg gate active, the `query` parameter captured by the `move ||` closure surrounding the gated `value.calculate_relevance(...)` may emit an `unused_variables` warning under api-only. Acceptable; do not introduce a `#[allow(unused_variables)]` to suppress it (would mask real warnings on upstream merges).
- **Verify after rebase:**
  - `cargo build` (default features) still succeeds (HTML edition still has relevance scoring).
  - `cargo build --bin tinysurfx --no-default-features --features api-only` still succeeds.
  - If upstream changes the closure body around `value.calculate_relevance`, re-apply the cfg gate to the new shape.

### P7 — Lint policy: `unsafe_code` deny (was forbid)
- **Files:** `Cargo.toml` (`[lints.rust] unsafe_code = "deny"`)
- **Purpose:** Rust 2024 made `std::env::set_var`/`remove_var` unsafe. Tests in `tests/api_only.rs` need them (env-driven config tests). `forbid` cannot be lifted by inner `#![allow(unsafe_code)]`; `deny` can. Production code in `src/` and `src/bin/` still cannot use `unsafe` without explicit annotation, and the test file's annotation is the only opt-in.
- **Upstream conflict risk:** low — single line in Cargo.toml.
- **Verify after rebase:** `cargo build` and `cargo build --bin tinysurfx --no-default-features --features api-only` both succeed; no new `unsafe` blocks in `src/` (`grep -rn 'unsafe' src/` returns nothing beyond `[lints]` itself).

### P8 — `regex` `std` feature explicit
- **Files:** `Cargo.toml` (regex features list now includes `"std"`)
- **Purpose:** Under html-edition, `keyword_extraction` transitively activates `regex/std`, which `aggregator.rs:214` depends on (`?` conversion of `regex::Error` requires `std::error::Error`). Under api-only that transitive activation disappears, so we activate `std` explicitly to keep `aggregator.rs` compiling.
- **Upstream conflict risk:** low — single feature list addition.
- **Verify after rebase:** both build modes succeed.

### P9 — Force-vendor OpenSSL (transitive via fake-useragent)
- **Files:** `Cargo.toml` (direct `openssl-sys` dep with `vendored` feature)
- **Purpose:** `fake-useragent v0.1.3` transitively pulls `reqwest v0.9.24` (2019) which uses `native-tls` → `openssl-sys`. Static musl Linux builds in CI cannot find system OpenSSL; vendoring lets the openssl-sys build script compile its own copy. Affects all builds; modest binary size cost.
- **Upstream conflict risk:** low — single Cargo.toml addition.
- **Verify after rebase:** `cargo build --bin tinysurfx --no-default-features --features api-only --target x86_64-unknown-linux-musl` succeeds in CI; `cargo build` (html edition) still succeeds.
