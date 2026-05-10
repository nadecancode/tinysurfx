# tinysurfx API-only edition — design

**Status:** approved
**Date:** 2026-05-10
**Owner:** nade
**Upstream:** `neon-mmd/websurfx` (`rolling` branch)

## Goal

Ship a slimmed-down, single-executable, JSON-only build of websurfx intended for embedding behind another service. Maintain it as a **patch-style edition** so upstream optimizations and bug fixes can flow in via routine merge.

Constraints, in priority order:

1. **Smallest possible binary.** No HTML/UI, no cache, no UI-only deps.
2. **Single executable per platform.** Static where the toolchain allows (Linux musl).
3. **API only** — JSON `/search` + `/healthz`. Intended for embedding, not direct end-user use.
4. **Cross-platform CI builds on every commit.**
5. **Patch-friendly fork.** Upstream `engines`, `aggregator`, `models` should flow in as fast-forward merges. Our divergence is small, additive, and cataloged.

## Architecture

### Crate layout — one crate, two binaries

The existing crate stays one crate. We do **not** split `aggregator` / `engines` into a separate library — that would force a permanent fork of upstream's module structure.

`Cargo.toml`:

```toml
[[bin]]
name = "websurfx"
path = "src/main.rs"
# Existing — untouched. Builds when api-only is OFF.

[[bin]]
name = "tinysurfx"
path = "src/bin/tinysurfx.rs"
required-features = ["api-only"]
# New. Only built when --features api-only is passed.
```

- `src/main.rs` is **byte-identical to upstream**.
- `src/bin/tinysurfx.rs` is **entirely new**. It wires its own Actix `App` with only `/search` and `/healthz`. It imports `aggregator::aggregate` directly. It has no reference to `templates`, `routes::*`, `cache`, or `parser`.

### The `api-only` Cargo feature

Acts as a build-time switch that controls which dependencies Cargo resolves and which lib modules compile.

When `api-only` is enabled (`--no-default-features --features api-only`):

- These deps become `optional = true` and are excluded from the dep graph: `maud`, `actix-files`, `actix-multipart`, `mlua`, `keyword_extraction`, `stop-words`, `thesaurus`, `moka`, `redis`, `chacha20poly1305`, `chacha20`, `base64`, `cfg-if`, `async-compression`.
- In `src/lib.rs`, the following declarations and the existing `pub async fn run()` get a `#[cfg(not(feature = "api-only"))]` gate so the lib still compiles without the optional deps:
  - `pub mod templates;`
  - `pub mod parser;`
  - `mod routes;`
  - `mod cache;` (already gated; feature simply tightens the guard)
  - `pub async fn run(...)`
- New modules exposed only under the feature:
  ```rust
  #[cfg(feature = "api-only")]
  pub mod api_config;

  // Shim so `aggregator.rs` keeps `use crate::parser::Config;` byte-identical
  // (see "Configuration" section for why this matters).
  #[cfg(feature = "api-only")]
  pub mod parser {
      pub use crate::api_config::Config;
  }
  ```
- Two surgical edits to keep the relevance-scoring path optional (see "Relevance scoring" below):
  - `src/aggregator.rs:175` — wrap the `value.calculate_relevance(query.as_str())` call in `#[cfg(not(feature = "api-only"))]`.
  - `src/models/aggregation.rs` — wrap the `calculate_relevance` impl, the `calculate_tf_idf` free fn, and their `use stop_words::*` / `use keyword_extraction::*` imports in `#[cfg(not(feature = "api-only"))]`.

### Relevance scoring (stop-words + keyword_extraction)

Upstream's `aggregator::aggregate` unconditionally calls `SearchResult::calculate_relevance(query)` to TF-IDF-rank results, which pulls in `stop-words` and `keyword_extraction`. We **gate this path off in `api-only` builds**: results are returned in upstream-engine order without re-ranking. Rationale:

- Consistent with the "smallest binary" priority — `stop-words` ships static word-lists for ~40 languages and `keyword_extraction` pulls extra parsing code; combined they are non-trivial bytes.
- An embedding consumer can apply its own ranking on the returned results.
- The patch is two small `#[cfg]` annotations across two files (cataloged as P6 in PATCHES.md).

### Patch surface against upstream

The entire structural divergence:

1. `Cargo.toml` — add `[[bin]] tinysurfx`, mark deps `optional = true`, add `[features] api-only = []` (purely a build switch — no `dep:` entries because the deps it disables are already optional).
2. `src/lib.rs` — ~5 `#[cfg(not(feature = "api-only"))]` annotations on existing `mod` lines and the `run()` fn, plus the new `api_config` module declaration and the `parser` re-export shim. **No deletions.**
3. `src/aggregator.rs` — one `#[cfg(not(feature = "api-only"))]` on the line that calls `value.calculate_relevance(...)`.
4. `src/models/aggregation.rs` — `#[cfg(not(feature = "api-only"))]` on the relevance-scoring impl, the `calculate_tf_idf` fn, and their imports.
5. `src/bin/tinysurfx.rs` — new file.
6. `src/api_config.rs` — new file.
7. `.github/workflows/build-binaries.yml` — new file.
8. `.github/workflows/upstream-sync.yml` — new file.
9. `PATCHES.md` — new file.
10. `CONTRIBUTING.md` — append a section documenting the upstream remote setup.

Upstream's `routes/`, `templates/`, `parser.rs`, `engines/`, `handler.rs`, `user_agent.rs`, and the `models/*` types themselves stay byte-identical. Items 3–4 are the only edits to actively-evolving upstream files; PATCHES.md (P6) flags them as the highest upstream-conflict-risk surface.

## Configuration: env vars + CLI flags

A new file `src/api_config.rs`, gated `#[cfg(feature = "api-only")]`, defines a `Config` struct shaped to be a **strict subset** of the fields `aggregator::aggregate` and the API server actually read. UI-only fields (`style`, `colorscheme`, `animation`, `http_cache_expiry_time`) are not present.

The new struct is named `Config`. The `lib.rs` shim `pub mod parser { pub use crate::api_config::Config; }` (under `cfg(api-only)`) makes `crate::parser::Config` resolve to it, so `src/aggregator.rs:9` (`use crate::parser::Config;`) and the function signature on `src/aggregator.rs:71` (`config: &Config`) stay byte-identical to upstream — no edits required to those lines.

**Required field set** — derived from the actual reads in `aggregator.rs:78-93`, the API server, and the rate-limit middleware:

| Field | Type | Source |
|---|---|---|
| `binding_ip` + `port` | from `TINYSURFX_BIND` | API server bind |
| `threads` | `u8` | actix workers |
| `request_timeout` | `u8` | aggregator (per-engine timeout) |
| `tcp_connection_keep_alive` | `u8` | aggregator (reqwest builder) |
| `pool_idle_connection_timeout` | `u8` | aggregator (reqwest builder) |
| `number_of_https_connections` | `u8` | aggregator (reqwest builder) |
| `operating_system_tls_certificates` | `bool` | aggregator (reqwest builder) |
| `adaptive_window` | `bool` | aggregator (reqwest builder) |
| `proxy` | `Option<reqwest::Proxy>` | aggregator (reqwest builder) |
| `safe_search` | `u8` (0..=2 in v1) | aggregator + API param default |
| `upstream_search_engines` | `HashMap<String, bool>` | engine-selection contract upstream uses |
| `client_connection_keep_alive` | `u8` | API server |
| `rate_limiter` | `RateLimiter { number_of_requests, time_limit }` | actix-governor middleware |

**Maintenance note:** if upstream adds a field that `aggregator` starts reading, mirror it in `api_config::Config` with a sensible default. PATCHES.md (P2) flags this as the single point to audit on every upstream merge.

### Env var → CLI flag mapping

| Env var | Default | Notes |
|---|---|---|
| `TINYSURFX_BIND` | `127.0.0.1:8080` | Combined host:port |
| `TINYSURFX_THREADS` | `available_parallelism() / 2` | Actix workers |
| `TINYSURFX_REQUEST_TIMEOUT_SECS` | `30` | Per-engine request timeout |
| `TINYSURFX_RATE_LIMIT_RPS` | `20` | actix-governor burst |
| `TINYSURFX_RATE_LIMIT_WINDOW_SECS` | `3` | actix-governor window |
| `TINYSURFX_SAFE_SEARCH` | `2` | **0..=2 only in v1** (see "Safesearch levels" below) |
| `TINYSURFX_ENGINES` | `duckduckgo` | Comma-separated |
| `TINYSURFX_PROXY` | unset | Optional `reqwest::Proxy` URL |
| `TINYSURFX_LOG` | `info` | `env_logger` filter spec |
| `TINYSURFX_CLIENT_KEEPALIVE_SECS` | `120` | HTTP keep-alive |
| `TINYSURFX_TCP_KEEPALIVE_SECS` | `30` | Upstream TCP keep-alive |
| `TINYSURFX_POOL_IDLE_TIMEOUT_SECS` | `30` | reqwest pool idle |
| `TINYSURFX_HTTPS_CONNECTIONS` | `10` | reqwest pool size |
| `TINYSURFX_OS_TLS_CERTS` | `true` | Use OS root certificates |
| `TINYSURFX_ADAPTIVE_WINDOW` | `true` | HTTP/2 adaptive window |

CLI flags mirror these one-to-one (`--bind`, `--threads`, `--engines`, etc.) and **override env**. CLI parsing is hand-rolled — no `clap` dependency. `--help` prints the env-var table above. `--version` prints the crate version.

### Safesearch levels

In v1, `TINYSURFX_SAFE_SEARCH` and the request-time `safesearch` query param are validated to `0..=2` and **levels 3 and 4 are rejected**.

Reason: levels 3 and 4 cause `aggregator::aggregate` to call `crate::handler::file_path(FileType::AllowList | BlockList)`, which searches `~/.config/websurfx/`, `/etc/xdg/websurfx/`, and `./websurfx/` for `allowlist.txt` and `blocklist.txt`. The "single executable, env-vars-only" framing of this edition breaks if these files are required at runtime. Validation is enforced in two places:

- `api_config::Config::from_env_and_args` — rejects `TINYSURFX_SAFE_SEARCH=3|4` at startup with a clear error.
- The `/search` request handler — rejects `safesearch=3|4` with `400 {"error":"safesearch level not supported in api-only edition","code":"bad_request"}`.

Adding allow/block-list support later (via `TINYSURFX_ALLOWLIST_PATH` / `TINYSURFX_BLOCKLIST_PATH` env vars feeding a small wrapper around `handler::file_path`) is non-breaking; deferred to v2.

## API contract

### `GET /healthz`

Always 200, no auth, no rate limit.

```json
{"status":"ok","version":"<crate-version>"}
```

### `GET /search`

Query parameters:

| Param | Required | Default | Notes |
|---|---|---|---|
| `q` | yes | — | Non-empty search query |
| `page` | no | `0` | Zero-indexed |
| `safesearch` | no | config default | `0..=2` (3 and 4 rejected with 400 — see "Safesearch levels") |

Success (`200`, `application/json`): the existing `models::aggregation::SearchResults` serialized via `serde_json::to_string`. Reusing the upstream-defined struct verbatim means our wire format tracks upstream automatically.

Errors (always JSON, never HTML):

| Condition | Status | Body |
|---|---|---|
| Missing or empty `q` | `400` | `{"error":"missing query","code":"empty_query"}` |
| Param parse failure | `400` | `{"error":"<msg>","code":"bad_request"}` |
| All upstream engines failed | `502` | `{"error":"all engines failed","code":"upstream_failed"}` |
| Rate limited | `429` | JSON-wrapped via custom error response handler over actix-governor |

### Middleware

- `actix_web::middleware::Compress` (brotli — already a default actix-web feature we keep)
- `actix_web::middleware::Logger`
- `actix_cors::Cors::permissive()` restricted to `GET`
- `actix_governor::Governor` from `Config::rate_limit_*`

**Removed:** `DefaultHeaders` Cache-Control middleware (was for browser HTML caching), static file serving, all HTML routes.

### Engines

The enabled-engine list is global, taken from `TINYSURFX_ENGINES`. There is **no per-request engine override** in v1 (keeps the surface minimal). Adding `&engines=` later is non-breaking.

## Build

Build command for the slim binary:

```bash
cargo build --bin tinysurfx \
    --no-default-features --features api-only \
    --profile bsr2 \
    --target <triple>
```

Profile `bsr2` already exists in `Cargo.toml` (`opt-level = "z"`, inherits `release` LTO + strip). No new profile needed.

## CI: build matrix on every commit

New workflow `.github/workflows/build-binaries.yml`. The existing `release.yml` (which currently handles version bumps and Linux-only release artifacts) stays as-is for tagged releases.

**Triggers:**

```yaml
on:
  push:
    branches: ['**']
    tags: ['v*']
  workflow_dispatch:

concurrency:
  group: build-binaries-${{ github.ref }}
  cancel-in-progress: true
```

**Matrix:**

| Target triple | Runner | Notes |
|---|---|---|
| `x86_64-unknown-linux-musl` | `ubuntu-latest` | Static. `apt install musl-tools`, set `CC_x86_64_unknown_linux_musl=musl-gcc`. |
| `aarch64-unknown-linux-musl` | `ubuntu-24.04-arm` | Native ARM runner, static. (Existing `release.yml` also uses `ubuntu-24.04-arm` — no cross-compile, intentional.) |
| `aarch64-apple-darwin` | `macos-latest` | Native arm64. |
| `x86_64-apple-darwin` | `macos-13` | Last Intel-mac runner image. |
| `x86_64-pc-windows-msvc` | `windows-latest` | `.exe` output. |

**Per-job steps:**

1. `actions/checkout@v6`.
2. `dtolnay/rust-toolchain@stable` with `targets: <triple>`.
3. `Swatinem/rust-cache@v2` keyed on the target triple.
4. Install platform prereqs (Linux: `musl-tools`).
5. `cargo build --bin tinysurfx --no-default-features --features api-only --profile bsr2 --target <triple>`.
6. Strip the binary (`strip` on linux/mac; skip on windows — already covered by `[profile.release] strip = "debuginfo"` inherited by bsr2).
7. Rename to `tinysurfx-<asset_suffix>[.exe]` and emit a `.sha256` sidecar.
8. `actions/upload-artifact@v7` with 14-day retention.

**Tag releases:** when the trigger ref is a `v*` tag, an additional `publish` job runs after the matrix completes. It downloads all artifacts and attaches them to the GitHub Release via `softprops/action-gh-release@v3`. Mirrors the existing `release.yml` style.

**Expected wall-clock per target:** ~5–8 min once cache is warm. Removing `mlua`/luajit from the dep graph is the largest CI speedup.

## Upstream sync mechanism

### Git remote

Documented one-time setup added to `CONTRIBUTING.md`:

```bash
git remote add upstream https://github.com/neon-mmd/websurfx.git
git fetch upstream
```

### Periodic sync workflow

New workflow `.github/workflows/upstream-sync.yml`:

```yaml
on:
  schedule:
    - cron: '0 8 * * 1'   # Mondays 08:00 UTC
  workflow_dispatch:
```

Steps:

1. Checkout `rolling` with `fetch-depth: 0`.
2. `git remote add upstream …` and `git fetch upstream rolling`.
3. Compute `branch=upstream-sync/$(date +%Y-%m-%d)`. Skip the run if the branch already exists.
4. `git checkout -b $branch`.
5. `git merge upstream/rolling --no-ff --no-commit`. Capture exit code.
6. If clean: `git commit -m "chore: merge upstream/rolling"`. If conflicts: `git add -A && git commit --no-verify -m "chore: merge upstream/rolling (CONFLICTS)"` — surfaces conflicts inside the PR rather than failing the action.
7. Push the branch and open a PR via `gh pr create`. PR body lists conflicting files (if any) and embeds a checklist of patches from `PATCHES.md` for the reviewer to verify.
8. Auto-label the PR `upstream-sync`.

Reviewer always lands the PR manually.

### `PATCHES.md`

Top-level catalog of every divergence-from-upstream commit. Format per entry:

```markdown
### P<n> — <short title>
- **Commits:** <sha-list>
- **Files:** <paths>
- **Purpose:** <one line>
- **Upstream conflict risk:** low | medium | high
- **How to verify after rebase:** <command or check>
```

Initial entries:

- **P1** — api-only Cargo feature + tinysurfx bin. Files: `Cargo.toml`, `src/lib.rs` (cfg gates only), `src/bin/tinysurfx.rs`. Risk: low (additive cfg gates).
- **P2** — env/CLI config (`api_config.rs`). Files: `src/api_config.rs`, `src/lib.rs` (the `pub mod parser { pub use crate::api_config::Config; }` shim under `cfg(api-only)`). Risk: medium — if upstream adds a `Config` field that `aggregator` reads, mirror it in `api_config::Config`.
- **P3** — JSON-only API server. Files: `src/bin/tinysurfx.rs`. Risk: low (isolated file).
- **P4** — `build-binaries.yml` workflow. Files: `.github/workflows/build-binaries.yml`. Risk: low (isolated file).
- **P5** — `upstream-sync.yml` workflow. Files: `.github/workflows/upstream-sync.yml`. Risk: low (isolated file).
- **P6** — gate relevance scoring (stop-words + keyword_extraction) behind `api-only`. Files: `src/aggregator.rs` (one cfg-gated line at ~line 175), `src/models/aggregation.rs` (cfg-gated impl + free fn + their imports). **Risk: high** — touches actively-evolving upstream files. Verify after every upstream merge: `cargo build --bin tinysurfx --no-default-features --features api-only` succeeds AND `cargo build` (default-features, the upstream binary) still succeeds.

A PR template for upstream-sync PRs reminds reviewers: *"verify each row in PATCHES.md still applies; update PATCHES.md if any commits were squashed during merge."*

## Out of scope (v1)

- Per-request engine override (`?engines=…`)
- Bearer-token auth
- `/engines` discovery endpoint
- Caching of any kind (memory or redis)
- HTML or static-file serving
- Settings export/import endpoints
- OpenSearch description, robots.txt, favicon, anything UI-adjacent
- Safesearch levels 3 and 4 (require allowlist/blocklist files on disk)
- TF-IDF relevance re-ranking (consumer can rank the returned results themselves)

These can be added later; the design does not preclude them.

## Open questions

None — all architectural decisions captured above. Implementation plan to follow via the writing-plans skill.
