# tinysurfx API-only edition — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a slim, single-binary, JSON-only build of the tinysurfx fork (`/home/nade/workspaces/external/tinysurfx`) suitable for embedding behind another service. Cross-platform CI builds on every commit. Maintainable as a patch-style edition against `neon-mmd/websurfx` upstream.

**Architecture:** Single Rust crate, two binaries. Existing `[[bin]] websurfx` (HTML/Lua-config) stays byte-identical to upstream. New `[[bin]] tinysurfx` (gated by `required-features = ["api-only"]`) is built from a tiny new `src/bin/tinysurfx.rs` plus two new modules: `src/api_config.rs` (env-vars + CLI flags replacing the Lua config) and `src/api_server.rs` (Actix `App` builder with `/search` JSON + `/healthz`). The `api-only` Cargo feature switches off UI deps (`maud`, `actix-files`, `mlua`, `keyword_extraction`, `stop-words`, etc.) and gates UI-related modules in `lib.rs` behind `#[cfg(not(feature = "api-only"))]`. A re-export shim `pub mod parser { pub use crate::api_config::Config; }` lets `aggregator.rs` keep `use crate::parser::Config;` byte-identical.

**Tech Stack:** Rust 2024 edition, Actix-web 4.11, reqwest with rustls, the existing `aggregator`/`engines`/`models` modules. No new runtime dependencies. CI: GitHub Actions with `dtolnay/rust-toolchain` and `Swatinem/rust-cache`.

**Spec:** [`docs/superpowers/specs/2026-05-10-tinysurfx-api-only-edition-design.md`](../specs/2026-05-10-tinysurfx-api-only-edition-design.md)

**Patch surface (target file list):**

| Action | Path | Purpose |
|---|---|---|
| Modify | `Cargo.toml` | Mark UI deps `optional = true`; add `[features] api-only`; add `[[bin]] tinysurfx` |
| Modify | `src/lib.rs` | Cfg-gate `templates`/`parser`/`routes`/`run`; add `api_config` + parser shim |
| Modify | `src/aggregator.rs` | Cfg-gate `value.calculate_relevance(query.as_str())` call |
| Modify | `src/models/aggregation.rs` | Cfg-gate `calculate_relevance` impl + `calculate_tf_idf` fn + their imports |
| Create | `src/api_config.rs` | Env + CLI config struct (gated `cfg(feature = "api-only")`) |
| Create | `src/api_server.rs` | Actix App builder + `/search` + `/healthz` handlers |
| Create | `src/bin/tinysurfx.rs` | Binary entry point: load config, run server |
| Create | `tests/api_only.rs` | Integration tests for the api-only bin |
| Create | `PATCHES.md` | Catalog of divergence-from-upstream commits |
| Modify | `CONTRIBUTING.md` | Append upstream-remote setup section |
| Create | `.github/workflows/build-binaries.yml` | 5-target build matrix on every push + tag |
| Create | `.github/workflows/upstream-sync.yml` | Weekly `git merge upstream/rolling` PR |

---

## Task 1: Bootstrap the api-only build (foundation, no tests)

**Why no tests:** This task establishes a buildable `tinysurfx` binary that just exits. Subsequent tasks TDD real behavior on top.

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/lib.rs`
- Modify: `src/aggregator.rs:175`
- Modify: `src/models/aggregation.rs` (function-level cfg-gates)
- Create: `src/api_config.rs`
- Create: `src/bin/tinysurfx.rs`

- [ ] **Step 1.1 — Edit `Cargo.toml`: mark UI deps as optional**

For each of the following deps, add `optional = true` to its inline table (most already have other attributes — don't remove them):

```toml
maud = { version = "0.27.0", default-features = false, features = ["actix-web"], optional = true }
actix-files = { version = "0.6.10", default-features = false, optional = true }
actix-multipart = { version = "0.7.2", default-features = false, features = ["derive", "tempfile"], optional = true }
mlua = { version = "0.11.6", features = ["async", "luajit", "vendored"], default-features = false, optional = true }
keyword_extraction = { version = "1.5.0", default-features = false, features = ["tf_idf", "rayon"], optional = true }
stop-words = { version = "0.10.0", default-features = false, features = ["iso"], optional = true }
```

(`moka`, `redis`, `chacha20*`, `base64`, `cfg-if`, `async-compression`, `dhat`, `thesaurus` are already `optional = true` — leave alone.)

- [ ] **Step 1.2 — Edit `Cargo.toml`: update `[features]` block**

Existing `[features]` block becomes:

```toml
[features]
default = ["html-edition"]

# The api-only edition: smallest possible binary, JSON API only.
api-only = []

# The original websurfx HTML edition. Default. Pulls every UI dep.
html-edition = [
    "memory-cache",
    "dep:maud",
    "dep:actix-files",
    "dep:actix-multipart",
    "dep:mlua",
    "dep:keyword_extraction",
    "dep:stop-words",
]

use-synonyms-search = ["thesaurus/static"]
dhat-heap = ["dep:dhat"]
memory-cache = ["dep:moka"]
redis-cache = ["dep:redis", "dep:base64"]
compress-cache-results = ["dep:async-compression", "dep:cfg-if"]
encrypt-cache-results = ["dep:chacha20poly1305", "dep:chacha20"]
cec-cache-results = ["compress-cache-results", "encrypt-cache-results"]
experimental-io-uring = ["actix-web/experimental-io-uring"]
use-non-static-synonyms-search = ["thesaurus"]
```

The `default = ["html-edition"]` keeps the existing `cargo build` behavior for upstream parity. The `cargo build --bin websurfx` path is unchanged.

- [ ] **Step 1.3 — Edit `Cargo.toml`: add the second binary**

Add after the existing `[[bin]] name = "websurfx"` block:

```toml
[[bin]]
name = "tinysurfx"
path = "src/bin/tinysurfx.rs"
required-features = ["api-only"]
```

- [ ] **Step 1.4 — Create `src/bin/tinysurfx.rs` (stub)**

```rust
//! tinysurfx — JSON-only api-only edition entry point. See `docs/superpowers/specs/2026-05-10-tinysurfx-api-only-edition-design.md`.

fn main() {
    eprintln!("tinysurfx {} (api-only stub)", env!("CARGO_PKG_VERSION"));
}
```

- [ ] **Step 1.5 — Create `src/api_config.rs` (skeleton with hardcoded defaults)**

```rust
//! Env-var + CLI-flag configuration for the api-only edition.
//!
//! Replaces `crate::parser::Config` (Lua-driven) when the `api-only` feature
//! is enabled. The fields here are a strict subset of upstream's `Config`,
//! covering only what `aggregator::aggregate` and the API server actually
//! read at runtime.

use std::collections::HashMap;

use crate::models::parser::RateLimiter;

/// API-only edition runtime configuration.
pub struct Config {
    pub binding_ip: String,
    pub port: u16,
    pub threads: u8,
    pub request_timeout: u8,
    pub tcp_connection_keep_alive: u8,
    pub pool_idle_connection_timeout: u8,
    pub number_of_https_connections: u8,
    pub operating_system_tls_certificates: bool,
    pub adaptive_window: bool,
    pub proxy: Option<reqwest::Proxy>,
    pub safe_search: u8,
    pub upstream_search_engines: HashMap<String, bool>,
    pub client_connection_keep_alive: u8,
    pub rate_limiter: RateLimiter,
}

impl Config {
    /// Build a `Config` with all hardcoded defaults — no env, no args.
    /// Real `from_env_and_args` arrives in Task 2/3.
    pub fn defaults() -> Self {
        let mut engines = HashMap::new();
        engines.insert("duckduckgo".to_string(), true);
        Self {
            binding_ip: "127.0.0.1".to_string(),
            port: 8080,
            threads: 4,
            request_timeout: 30,
            tcp_connection_keep_alive: 30,
            pool_idle_connection_timeout: 30,
            number_of_https_connections: 10,
            operating_system_tls_certificates: true,
            adaptive_window: true,
            proxy: None,
            safe_search: 2,
            upstream_search_engines: engines,
            client_connection_keep_alive: 120,
            rate_limiter: RateLimiter { number_of_requests: 20, time_limit: 3 },
        }
    }
}
```

- [ ] **Step 1.6 — Edit `src/lib.rs`: add cfg gates and the parser shim**

The current top of `src/lib.rs`:

```rust
mod aggregator;
#[cfg(any(feature = "redis-cache", feature = "memory-cache"))]
mod cache;
mod engines;
mod handler;
mod models;
pub mod parser;
mod routes;
pub mod templates;
mod user_agent;
```

Replace the four UI-related lines so the file becomes:

```rust
mod aggregator;
#[cfg(any(feature = "redis-cache", feature = "memory-cache"))]
mod cache;
mod engines;
mod handler;
mod models;

#[cfg(not(feature = "api-only"))]
pub mod parser;
#[cfg(not(feature = "api-only"))]
mod routes;
#[cfg(not(feature = "api-only"))]
pub mod templates;

mod user_agent;

// api-only edition modules + the parser shim that lets aggregator.rs keep
// `use crate::parser::Config;` byte-identical.
#[cfg(feature = "api-only")]
pub mod api_config;
#[cfg(feature = "api-only")]
pub mod parser {
    pub use crate::api_config::Config;
}
```

Then gate the existing `pub async fn run(...)` and **every** top-level `use` statement that supports it. Concretely, prepend `#[cfg(not(feature = "api-only"))]` to each of these existing items in `src/lib.rs`:

```rust
#[cfg(not(feature = "api-only"))]
use actix_cors::Cors;
#[cfg(not(feature = "api-only"))]
use actix_files as fs;
#[cfg(not(feature = "api-only"))]
use actix_governor::{Governor, GovernorConfigBuilder};
#[cfg(not(feature = "api-only"))]
use actix_web::{
    App, HttpServer,
    dev::Server,
    http::header::{self, CacheControl, CacheDirective},
    middleware::{Compress, DefaultHeaders, Logger},
    web,
};
#[cfg(not(feature = "api-only"))]
use handler::{FileType, file_path};
#[cfg(not(feature = "api-only"))]
use parser::Config;
#[cfg(not(feature = "api-only"))]
use tokio::{net::TcpListener, time::Duration};

#[cfg(not(feature = "api-only"))]
pub async fn run(listener: TcpListener, config: &'static Config) -> tokio::io::Result<Server> {
    // ... existing body unchanged ...
}
```

Note specifically that `use parser::Config;` (line ~22 in upstream) **must** be gated, because under api-only the api-only `parser` shim re-exports `Config` from `api_config`, and the `run` function (which would import the legacy struct) is gone. Leaving this `use` ungated will compile in both modes but is dead in api-only mode — gate it for clarity.

- [ ] **Step 1.7 — Edit `src/aggregator.rs:175`: cfg-gate the relevance call**

The current closure body (the `.map(|(_, mut value)|` block):

```rust
.map(|(_, mut value)| {
    if !value.url.contains("temu.com") {
        value.calculate_relevance(query.as_str())
    }
    value
})
```

Becomes:

```rust
.map(|(_, mut value)| {
    #[cfg(not(feature = "api-only"))]
    if !value.url.contains("temu.com") {
        value.calculate_relevance(query.as_str())
    }
    value
})
```

- [ ] **Step 1.8 — Edit `src/models/aggregation.rs`: cfg-gate relevance scoring**

Find the `pub fn calculate_relevance(&mut self, query: &str)` impl (around line 57). Add `#[cfg(not(feature = "api-only"))]` directly above the `pub fn` line.

Find the free fn `fn calculate_tf_idf(...)` (around line 245). Add `#[cfg(not(feature = "api-only"))]` directly above its signature.

The `use stop_words::*` import lives **inside** `calculate_relevance` (line 58, inside the function body). The `use keyword_extraction::*` import lives **inside** `calculate_tf_idf`. Because they are local-to-function, gating the function gates the imports. No additional edits to the top-of-file imports are required.

- [ ] **Step 1.9 — Verify both build modes**

```bash
cargo build --bin websurfx 2>&1 | tail -20
```

Expected: succeeds (HTML edition still builds, no warnings beyond pre-existing ones).

```bash
cargo build --bin tinysurfx --no-default-features --features api-only 2>&1 | tail -20
```

Expected: succeeds. Output binary at `target/debug/tinysurfx`.

```bash
./target/debug/tinysurfx
```

Expected: prints `tinysurfx 1.29.0 (api-only stub)` and exits 0.

If either build fails: read the error. Most likely cause is a `templates::` / `actix_files::` / `Compress::` / etc. reference that lives in `src/lib.rs` outside the cfg-gated block. Add the gate.

- [ ] **Step 1.10 — Commit**

```bash
git add Cargo.toml Cargo.lock src/lib.rs src/aggregator.rs src/models/aggregation.rs src/api_config.rs src/bin/tinysurfx.rs
git commit -m "$(cat <<'EOF'
feat(api-only): bootstrap api-only Cargo feature + tinysurfx bin

- Mark UI deps (maud, actix-files, actix-multipart, mlua,
  keyword_extraction, stop-words) optional behind html-edition feature
- Add api-only Cargo feature; default stays html-edition for upstream parity
- Cfg-gate templates/routes/parser/run() in lib.rs so the lib compiles
  without the UI deps
- Add api_config module + parser re-export shim so aggregator.rs keeps
  `use crate::parser::Config;` byte-identical
- Gate the calculate_relevance call in aggregator.rs and the impl + free fn
  in models/aggregation.rs so stop-words and keyword_extraction can be
  excluded from the api-only dep graph
- Stub `[[bin]] tinysurfx` (required-features = ["api-only"]) — prints
  version and exits

EOF
)"
```

---

## Task 2: TDD env-var parsing for `api_config::Config`

**Files:**
- Modify: `src/api_config.rs`
- Create: `tests/api_only.rs` (new integration test file)

- [ ] **Step 2.1 — Create `tests/api_only.rs` with the first failing test**

```rust
//! Integration tests for the api-only edition.

#![cfg(feature = "api-only")]

use websurfx::api_config::Config;

#[test]
fn from_env_uses_defaults_when_unset() {
    // Snapshot env, clear all TINYSURFX_* vars, restore on drop via a guard.
    let _guard = EnvGuard::clear_tinysurfx();
    let cfg = Config::from_env().expect("defaults");
    assert_eq!(cfg.binding_ip, "127.0.0.1");
    assert_eq!(cfg.port, 8080);
    assert_eq!(cfg.safe_search, 2);
    assert_eq!(cfg.upstream_search_engines.get("duckduckgo"), Some(&true));
}

#[test]
fn from_env_reads_bind_split() {
    let _guard = EnvGuard::set("TINYSURFX_BIND", "0.0.0.0:9090");
    let cfg = Config::from_env().expect("ok");
    assert_eq!(cfg.binding_ip, "0.0.0.0");
    assert_eq!(cfg.port, 9090);
}

#[test]
fn from_env_reads_engines_csv() {
    let _guard = EnvGuard::set("TINYSURFX_ENGINES", "duckduckgo,brave,searx");
    let cfg = Config::from_env().expect("ok");
    assert_eq!(cfg.upstream_search_engines.len(), 3);
    assert_eq!(cfg.upstream_search_engines.get("brave"), Some(&true));
}

#[test]
fn from_env_reads_numeric_fields() {
    let _g1 = EnvGuard::set("TINYSURFX_THREADS", "8");
    let _g2 = EnvGuard::set("TINYSURFX_REQUEST_TIMEOUT_SECS", "45");
    let _g3 = EnvGuard::set("TINYSURFX_RATE_LIMIT_RPS", "100");
    let cfg = Config::from_env().expect("ok");
    assert_eq!(cfg.threads, 8);
    assert_eq!(cfg.request_timeout, 45);
    assert_eq!(cfg.rate_limiter.number_of_requests, 100);
}

// --- Helpers ---

/// Tiny RAII guard so tests don't pollute each other's env. Cargo runs
/// integration tests in a single process by default — without this guard the
/// tests would race.
struct EnvGuard {
    keys: Vec<String>,
    prior: Vec<(String, Option<String>)>,
}

impl EnvGuard {
    fn set(key: &str, val: &str) -> Self {
        let prior = vec![(key.to_string(), std::env::var(key).ok())];
        // SAFETY: tests are run with --test-threads=1 (see step 2.3)
        unsafe { std::env::set_var(key, val); }
        Self { keys: vec![key.to_string()], prior }
    }
    fn clear_tinysurfx() -> Self {
        let mut prior = Vec::new();
        let mut keys = Vec::new();
        for (k, v) in std::env::vars() {
            if k.starts_with("TINYSURFX_") {
                prior.push((k.clone(), Some(v)));
                unsafe { std::env::remove_var(&k); }
                keys.push(k);
            }
        }
        Self { keys, prior }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (k, v) in &self.prior {
            unsafe {
                match v {
                    Some(val) => std::env::set_var(k, val),
                    None => std::env::remove_var(k),
                }
            }
        }
        for k in &self.keys {
            // already restored via prior, but guard against keys added during the test
            if !self.prior.iter().any(|(pk, _)| pk == k) {
                unsafe { std::env::remove_var(k); }
            }
        }
    }
}
```

- [ ] **Step 2.2 — Run the test; expect failure**

```bash
cargo test --test api_only --no-default-features --features api-only -- --test-threads=1 2>&1 | tail -30
```

Expected: compile failure — `Config::from_env` does not exist.

- [ ] **Step 2.3 — Implement `Config::from_env`**

Replace the `Config::defaults` impl in `src/api_config.rs` (keep `defaults` available for `from_env`'s starting point):

```rust
use std::collections::HashMap;
use std::env;
use std::str::FromStr;

use crate::models::parser::RateLimiter;

#[derive(Debug)]
pub enum ConfigError {
    BadBind(String),
    BadInt(String, String),
    BadBool(String, String),
    BadProxy(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadBind(s) => write!(f, "TINYSURFX_BIND must be `host:port`, got {s:?}"),
            Self::BadInt(k, s) => write!(f, "{k} must be an integer, got {s:?}"),
            Self::BadBool(k, s) => write!(f, "{k} must be true|false, got {s:?}"),
            Self::BadProxy(s) => write!(f, "TINYSURFX_PROXY is not a valid URL: {s}"),
        }
    }
}

impl std::error::Error for ConfigError {}

pub struct Config { /* fields as in step 1.5 — unchanged */ }

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let mut cfg = Self::defaults();

        if let Ok(bind) = env::var("TINYSURFX_BIND") {
            let (host, port) = bind.rsplit_once(':').ok_or(ConfigError::BadBind(bind.clone()))?;
            cfg.binding_ip = host.to_string();
            cfg.port = port.parse().map_err(|_| ConfigError::BadBind(bind.clone()))?;
        }
        cfg.threads = parse_env_int("TINYSURFX_THREADS", cfg.threads)?;
        cfg.request_timeout = parse_env_int("TINYSURFX_REQUEST_TIMEOUT_SECS", cfg.request_timeout)?;
        cfg.tcp_connection_keep_alive = parse_env_int("TINYSURFX_TCP_KEEPALIVE_SECS", cfg.tcp_connection_keep_alive)?;
        cfg.pool_idle_connection_timeout = parse_env_int("TINYSURFX_POOL_IDLE_TIMEOUT_SECS", cfg.pool_idle_connection_timeout)?;
        cfg.number_of_https_connections = parse_env_int("TINYSURFX_HTTPS_CONNECTIONS", cfg.number_of_https_connections)?;
        cfg.operating_system_tls_certificates = parse_env_bool("TINYSURFX_OS_TLS_CERTS", cfg.operating_system_tls_certificates)?;
        cfg.adaptive_window = parse_env_bool("TINYSURFX_ADAPTIVE_WINDOW", cfg.adaptive_window)?;
        cfg.safe_search = parse_env_int("TINYSURFX_SAFE_SEARCH", cfg.safe_search)?;
        cfg.client_connection_keep_alive = parse_env_int("TINYSURFX_CLIENT_KEEPALIVE_SECS", cfg.client_connection_keep_alive)?;
        cfg.rate_limiter.number_of_requests = parse_env_int("TINYSURFX_RATE_LIMIT_RPS", cfg.rate_limiter.number_of_requests)?;
        cfg.rate_limiter.time_limit = parse_env_int("TINYSURFX_RATE_LIMIT_WINDOW_SECS", cfg.rate_limiter.time_limit)?;

        if let Ok(engines) = env::var("TINYSURFX_ENGINES") {
            cfg.upstream_search_engines.clear();
            for name in engines.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                cfg.upstream_search_engines.insert(name.to_lowercase(), true);
            }
        }

        if let Ok(proxy_url) = env::var("TINYSURFX_PROXY") {
            cfg.proxy = Some(reqwest::Proxy::all(&proxy_url).map_err(|_| ConfigError::BadProxy(proxy_url))?);
        }

        Ok(cfg)
    }
}

fn parse_env_int<T: FromStr>(key: &str, default: T) -> Result<T, ConfigError> {
    match env::var(key) {
        Ok(s) => s.parse().map_err(|_| ConfigError::BadInt(key.to_string(), s)),
        Err(_) => Ok(default),
    }
}

fn parse_env_bool(key: &str, default: bool) -> Result<bool, ConfigError> {
    match env::var(key) {
        Ok(s) => match s.to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => Ok(true),
            "false" | "0" | "no" | "off" => Ok(false),
            _ => Err(ConfigError::BadBool(key.to_string(), s)),
        },
        Err(_) => Ok(default),
    }
}
```

- [ ] **Step 2.4 — Run the tests; expect pass**

```bash
cargo test --test api_only --no-default-features --features api-only -- --test-threads=1 2>&1 | tail -20
```

Expected: all 4 tests pass. The `--test-threads=1` is required because the tests mutate process env.

- [ ] **Step 2.5 — Commit**

```bash
git add src/api_config.rs tests/api_only.rs
git commit -m "$(cat <<'EOF'
feat(api-only): env-var config parsing for tinysurfx bin

Implements Config::from_env reading TINYSURFX_BIND, _THREADS,
_REQUEST_TIMEOUT_SECS, _RATE_LIMIT_*, _SAFE_SEARCH, _ENGINES, _PROXY,
_OS_TLS_CERTS, _ADAPTIVE_WINDOW, _CLIENT_KEEPALIVE_SECS,
_TCP_KEEPALIVE_SECS, _POOL_IDLE_TIMEOUT_SECS, _HTTPS_CONNECTIONS.

Tests in tests/api_only.rs run with --test-threads=1 because they mutate
process env via an RAII guard.

EOF
)"
```

---

## Task 3: TDD CLI flag override on top of env

**Files:**
- Modify: `src/api_config.rs`
- Modify: `tests/api_only.rs`

- [ ] **Step 3.1 — Add failing tests**

Append to `tests/api_only.rs`:

```rust
#[test]
fn cli_flag_overrides_env() {
    let _g = EnvGuard::set("TINYSURFX_BIND", "127.0.0.1:7000");
    let args = vec!["--bind".to_string(), "0.0.0.0:9999".to_string()];
    let cfg = Config::from_env_and_args(&args).expect("ok");
    assert_eq!(cfg.binding_ip, "0.0.0.0");
    assert_eq!(cfg.port, 9999);
}

#[test]
fn cli_engines_replaces_env() {
    let _g = EnvGuard::set("TINYSURFX_ENGINES", "duckduckgo");
    let args = vec!["--engines".to_string(), "brave,searx".to_string()];
    let cfg = Config::from_env_and_args(&args).expect("ok");
    assert_eq!(cfg.upstream_search_engines.len(), 2);
    assert!(cfg.upstream_search_engines.contains_key("searx"));
}

#[test]
fn cli_help_returns_help_request() {
    let args = vec!["--help".to_string()];
    let err = Config::from_env_and_args(&args);
    assert!(matches!(err, Err(ConfigError::HelpRequested)));
}
```

Adjust the imports at the top of the file:

```rust
use websurfx::api_config::{Config, ConfigError};
```

- [ ] **Step 3.2 — Run; expect failure**

```bash
cargo test --test api_only --no-default-features --features api-only -- --test-threads=1 2>&1 | tail -20
```

Expected: compile failure — `from_env_and_args` and `HelpRequested` do not exist.

- [ ] **Step 3.3 — Add `HelpRequested` and `from_env_and_args`**

In `src/api_config.rs`, add a variant to `ConfigError`:

```rust
pub enum ConfigError {
    BadBind(String),
    BadInt(String, String),
    BadBool(String, String),
    BadProxy(String),
    BadFlag(String),
    HelpRequested,
}
```

Update the `Display` impl:

```rust
Self::BadFlag(s) => write!(f, "unknown flag {s:?}"),
Self::HelpRequested => write!(f, "help requested"),
```

Add the entry point:

```rust
const HELP_TEXT: &str = include_str!("api_config_help.txt");

impl Config {
    pub fn from_env_and_args(args: &[String]) -> Result<Self, ConfigError> {
        let mut cfg = Self::from_env()?;
        let mut iter = args.iter();
        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "--help" | "-h" => return Err(ConfigError::HelpRequested),
                "--version" | "-V" => {
                    println!("tinysurfx {}", env!("CARGO_PKG_VERSION"));
                    std::process::exit(0);
                }
                "--bind" => {
                    let v = iter.next().ok_or_else(|| ConfigError::BadFlag("--bind needs value".into()))?;
                    let (h, p) = v.rsplit_once(':').ok_or_else(|| ConfigError::BadBind(v.clone()))?;
                    cfg.binding_ip = h.to_string();
                    cfg.port = p.parse().map_err(|_| ConfigError::BadBind(v.clone()))?;
                }
                "--threads" => cfg.threads = take_int(&mut iter, "--threads")?,
                "--request-timeout" => cfg.request_timeout = take_int(&mut iter, "--request-timeout")?,
                "--rate-limit-rps" => cfg.rate_limiter.number_of_requests = take_int(&mut iter, "--rate-limit-rps")?,
                "--rate-limit-window" => cfg.rate_limiter.time_limit = take_int(&mut iter, "--rate-limit-window")?,
                "--safe-search" => cfg.safe_search = take_int(&mut iter, "--safe-search")?,
                "--engines" => {
                    let v = iter.next().ok_or_else(|| ConfigError::BadFlag("--engines needs value".into()))?;
                    cfg.upstream_search_engines.clear();
                    for name in v.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                        cfg.upstream_search_engines.insert(name.to_lowercase(), true);
                    }
                }
                "--proxy" => {
                    let v = iter.next().ok_or_else(|| ConfigError::BadFlag("--proxy needs value".into()))?;
                    cfg.proxy = Some(reqwest::Proxy::all(v).map_err(|_| ConfigError::BadProxy(v.clone()))?);
                }
                other => return Err(ConfigError::BadFlag(other.to_string())),
            }
        }
        Ok(cfg)
    }

    pub fn help_text() -> &'static str { HELP_TEXT }
}

fn take_int<'a, I, T>(iter: &mut I, name: &str) -> Result<T, ConfigError>
where I: Iterator<Item = &'a String>, T: FromStr {
    let v = iter.next().ok_or_else(|| ConfigError::BadFlag(format!("{name} needs value")))?;
    v.parse().map_err(|_| ConfigError::BadInt(name.to_string(), v.clone()))
}
```

Create `src/api_config_help.txt`:

```text
tinysurfx — JSON-only meta-search API server.

USAGE:
    tinysurfx [FLAGS]

FLAGS (override env vars; CLI > env > default):
    --bind <HOST:PORT>            (TINYSURFX_BIND)            default 127.0.0.1:8080
    --threads <N>                 (TINYSURFX_THREADS)         default 4
    --request-timeout <SECS>      (TINYSURFX_REQUEST_TIMEOUT_SECS)  default 30
    --rate-limit-rps <N>          (TINYSURFX_RATE_LIMIT_RPS)  default 20
    --rate-limit-window <SECS>    (TINYSURFX_RATE_LIMIT_WINDOW_SECS)  default 3
    --safe-search <0..2>          (TINYSURFX_SAFE_SEARCH)     default 2
    --engines <CSV>               (TINYSURFX_ENGINES)         default duckduckgo
    --proxy <URL>                 (TINYSURFX_PROXY)           default unset
    --help, -h                    Print this help and exit
    --version, -V                 Print version and exit

ENV-ONLY (no CLI counterpart):
    TINYSURFX_LOG                 env_logger spec, default "info"
    TINYSURFX_OS_TLS_CERTS        true|false, default true
    TINYSURFX_ADAPTIVE_WINDOW     true|false, default true
    TINYSURFX_CLIENT_KEEPALIVE_SECS  default 120
    TINYSURFX_TCP_KEEPALIVE_SECS  default 30
    TINYSURFX_POOL_IDLE_TIMEOUT_SECS  default 30
    TINYSURFX_HTTPS_CONNECTIONS   default 10

ENDPOINTS:
    GET /search?q=<QUERY>&page=<N>&safesearch=<0..2>
    GET /healthz
```

- [ ] **Step 3.4 — Run; expect pass**

```bash
cargo test --test api_only --no-default-features --features api-only -- --test-threads=1 2>&1 | tail -20
```

Expected: 7 tests pass.

- [ ] **Step 3.5 — Commit**

```bash
git add src/api_config.rs src/api_config_help.txt tests/api_only.rs
git commit -m "$(cat <<'EOF'
feat(api-only): CLI flag overrides for env config

Adds Config::from_env_and_args with hand-rolled flag parser (no clap dep)
that honors --bind, --threads, --request-timeout, --rate-limit-rps,
--rate-limit-window, --safe-search, --engines, --proxy, --help, --version.
CLI flags override env. Help text in src/api_config_help.txt is included
at compile time.

EOF
)"
```

---

## Task 4: TDD safesearch and threads validation

**Files:**
- Modify: `src/api_config.rs`
- Modify: `tests/api_only.rs`

- [ ] **Step 4.1 — Add failing tests**

Append to `tests/api_only.rs`:

```rust
#[test]
fn rejects_safesearch_3() {
    let _g = EnvGuard::set("TINYSURFX_SAFE_SEARCH", "3");
    let result = Config::from_env_and_args(&[]);
    assert!(matches!(result, Err(ConfigError::SafesearchUnsupported(3))),
        "expected SafesearchUnsupported(3), got {result:?}");
}

#[test]
fn rejects_safesearch_4() {
    let _g = EnvGuard::set("TINYSURFX_SAFE_SEARCH", "4");
    assert!(matches!(Config::from_env_and_args(&[]), Err(ConfigError::SafesearchUnsupported(4))));
}

#[test]
fn accepts_safesearch_0_through_2() {
    for level in 0u8..=2 {
        let _g = EnvGuard::set("TINYSURFX_SAFE_SEARCH", &level.to_string());
        let cfg = Config::from_env_and_args(&[]).expect("ok");
        assert_eq!(cfg.safe_search, level);
    }
}

#[test]
fn rejects_zero_threads() {
    let _g = EnvGuard::set("TINYSURFX_THREADS", "0");
    assert!(matches!(Config::from_env_and_args(&[]), Err(ConfigError::InvalidValue(_))));
}

#[test]
fn rejects_empty_engines() {
    let _g = EnvGuard::set("TINYSURFX_ENGINES", "");
    assert!(matches!(Config::from_env_and_args(&[]), Err(ConfigError::InvalidValue(_))));
}
```

`from_env_and_args` is the validation point because env-only `from_env` is also invoked through it.

- [ ] **Step 4.2 — Run; expect failure**

```bash
cargo test --test api_only --no-default-features --features api-only -- --test-threads=1 2>&1 | tail -20
```

Expected: compile failure on `SafesearchUnsupported`, `InvalidValue` variants.

- [ ] **Step 4.3 — Add validation**

In `src/api_config.rs`, extend `ConfigError`:

```rust
pub enum ConfigError {
    BadBind(String),
    BadInt(String, String),
    BadBool(String, String),
    BadProxy(String),
    BadFlag(String),
    HelpRequested,
    SafesearchUnsupported(u8),
    InvalidValue(String),
}
```

Update `Display`:

```rust
Self::SafesearchUnsupported(n) => write!(f, "safesearch level {n} is not supported in the api-only edition (allowed: 0..=2)"),
Self::InvalidValue(s) => write!(f, "{s}"),
```

In `Config::from_env_and_args`, **after** the `args` parsing loop and before `Ok(cfg)`, insert:

```rust
        if cfg.safe_search > 2 {
            return Err(ConfigError::SafesearchUnsupported(cfg.safe_search));
        }
        if cfg.threads == 0 {
            return Err(ConfigError::InvalidValue("threads must be > 0".into()));
        }
        if cfg.upstream_search_engines.is_empty() {
            return Err(ConfigError::InvalidValue("at least one engine must be enabled".into()));
        }
```

- [ ] **Step 4.4 — Run; expect pass**

```bash
cargo test --test api_only --no-default-features --features api-only -- --test-threads=1 2>&1 | tail -20
```

Expected: 12 tests pass.

- [ ] **Step 4.5 — Commit**

```bash
git add src/api_config.rs tests/api_only.rs
git commit -m "$(cat <<'EOF'
feat(api-only): validate safesearch (0..=2), threads > 0, non-empty engines

Levels 3 and 4 require allowlist/blocklist files on disk; rejected at
startup with a clear error so misconfigurations fail fast rather than
producing silent runtime failures inside aggregator.

EOF
)"
```

---

## Task 5: TDD `/healthz` route in a new `api_server` module

**Files:**
- Create: `src/api_server.rs`
- Modify: `src/lib.rs` (declare the module under `cfg(api-only)`)
- Modify: `tests/api_only.rs`

- [ ] **Step 5.1 — Add failing test**

Append to `tests/api_only.rs`:

```rust
use actix_web::{App, test, web, http::StatusCode};
use serde_json::Value;
use websurfx::api_server;

#[actix_web::test]
async fn healthz_returns_ok_json() {
    let cfg = leak_config();
    let app = test::init_service(
        App::new().app_data(web::Data::new(cfg)).configure(api_server::configure)
    ).await;
    let req = test::TestRequest::get().uri("/healthz").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["status"], "ok");
    assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));
}

fn leak_config() -> &'static Config {
    let _g = EnvGuard::clear_tinysurfx();
    let cfg = Config::from_env_and_args(&[]).expect("ok");
    Box::leak(Box::new(cfg))
}
```

- [ ] **Step 5.2 — Run; expect compile failure**

```bash
cargo test --test api_only --no-default-features --features api-only -- --test-threads=1 2>&1 | tail -30
```

Expected: `unresolved import websurfx::api_server`.

- [ ] **Step 5.3 — Create `src/api_server.rs` with `/healthz`**

The "build the whole `App`" pattern requires writing out an `impl ServiceFactory<...>` return type that is notoriously brittle. Avoid it: this module exposes a `pub fn configure(cfg: &mut ServiceConfig)` that registers routes only. Both the bin and the integration tests build their own `App` and call `.configure(api_server::configure)`. Middleware (Logger, Compress, Cors, Governor) is wired in the bin (not exported) — tests don't need any middleware to exercise route logic.

```rust
//! JSON-only route handlers for the api-only edition.

use actix_web::{HttpResponse, get, web::ServiceConfig};
use serde_json::json;

/// Register all routes on the provided `ServiceConfig`.
/// Both the bin entry point and integration tests call this so the same set
/// of handlers is exercised in both places.
pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(healthz);
    // /search added in Task 6.
}

#[get("/healthz")]
async fn healthz() -> HttpResponse {
    HttpResponse::Ok().json(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}
```

- [ ] **Step 5.4 — Declare the module in `src/lib.rs`**

Below the existing `pub mod api_config;` line, add:

```rust
#[cfg(feature = "api-only")]
pub mod api_server;
```

- [ ] **Step 5.5 — Run; expect pass**

```bash
cargo test --test api_only --no-default-features --features api-only -- --test-threads=1 2>&1 | tail -20
```

Expected: 13 tests pass.

(The contract: tests stand up the same handlers the bin does, by calling `App::new().app_data(...).configure(api_server::configure)`.)

- [ ] **Step 5.6 — Commit**

```bash
git add src/api_server.rs src/lib.rs tests/api_only.rs
git commit -m "$(cat <<'EOF'
feat(api-only): /healthz endpoint and api_server module skeleton

api_server::configure(&mut ServiceConfig) registers route handlers; both
the bin and integration tests build their own App and call .configure()
so the same handlers are exercised in both. Middleware (Logger, Compress,
Cors) is wired in the bin only — tests don't need it to exercise route
logic. No Cache-Control, no static-file serving.

EOF
)"
```

---

## Task 6: TDD `/search` validation paths (empty q, bad safesearch)

**Files:**
- Modify: `src/api_server.rs`
- Modify: `tests/api_only.rs`

- [ ] **Step 6.1 — Add failing tests**

Append to `tests/api_only.rs`:

```rust
#[actix_web::test]
async fn search_missing_q_returns_400() {
    let app = test::init_service(
        App::new().app_data(web::Data::new(leak_config())).configure(api_server::configure)
    ).await;
    let req = test::TestRequest::get().uri("/search").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["code"], "empty_query");
}

#[actix_web::test]
async fn search_empty_q_returns_400() {
    let app = test::init_service(
        App::new().app_data(web::Data::new(leak_config())).configure(api_server::configure)
    ).await;
    let req = test::TestRequest::get().uri("/search?q=").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["code"], "empty_query");
}

#[actix_web::test]
async fn search_safesearch_3_returns_400() {
    let app = test::init_service(
        App::new().app_data(web::Data::new(leak_config())).configure(api_server::configure)
    ).await;
    let req = test::TestRequest::get().uri("/search?q=hello&safesearch=3").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["code"], "bad_request");
}

#[actix_web::test]
async fn search_bad_page_returns_400() {
    let app = test::init_service(
        App::new().app_data(web::Data::new(leak_config())).configure(api_server::configure)
    ).await;
    let req = test::TestRequest::get().uri("/search?q=hi&page=notanumber").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["code"], "bad_request");
}
```

- [ ] **Step 6.2 — Run; expect failure**

```bash
cargo test --test api_only --no-default-features --features api-only -- --test-threads=1 2>&1 | tail -20
```

Expected: 4 new tests fail with 404 (no /search route yet).

- [ ] **Step 6.3 — Add `/search` handler with validation only**

In `src/api_server.rs`, register the handler in `configure`:

```rust
pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(healthz);
    cfg.service(search);
}
```

Add at the top of the file (alongside the existing `use` line):

```rust
use actix_web::{HttpRequest, web};
use serde::Deserialize;

use crate::api_config::Config;
```

Add at the bottom of the file:

```rust
#[derive(Deserialize)]
struct SearchQuery {
    q: Option<String>,
    page: Option<u32>,
    safesearch: Option<u8>,
}

#[get("/search")]
async fn search(req: HttpRequest, config: web::Data<&'static Config>) -> HttpResponse {
    // Manual decode so a parse failure becomes a JSON 400 instead of actix's
    // default plain-text 400.
    let parsed = web::Query::<SearchQuery>::from_query(req.query_string());
    let params = match parsed {
        Ok(p) => p.into_inner(),
        Err(e) => return bad_request(&e.to_string()),
    };

    let q = params.q.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let q = match q {
        Some(s) => s.to_string(),
        None => return HttpResponse::BadRequest().json(json!({
            "error": "missing query",
            "code": "empty_query",
        })),
    };

    let safe_search = params.safesearch.unwrap_or(config.safe_search);
    if safe_search > 2 {
        return bad_request("safesearch level not supported in api-only edition (allowed: 0..=2)");
    }

    let _page = params.page.unwrap_or(0);
    let _q = q;
    // Aggregator wiring lands in Task 7.
    HttpResponse::NotImplemented().json(json!({
        "error": "search not yet wired",
        "code": "not_implemented",
    }))
}

fn bad_request(msg: &str) -> HttpResponse {
    HttpResponse::BadRequest().json(json!({"error": msg, "code": "bad_request"}))
}
```

- [ ] **Step 6.4 — Run; expect pass**

```bash
cargo test --test api_only --no-default-features --features api-only -- --test-threads=1 2>&1 | tail -20
```

Expected: 17 tests pass (3 new validation tests; the 4th uses `bad_request` code as expected).

- [ ] **Step 6.5 — Commit**

```bash
git add src/api_server.rs tests/api_only.rs
git commit -m "$(cat <<'EOF'
feat(api-only): /search request validation (q required, safesearch 0..=2)

Returns 400 + structured JSON error for empty q (`empty_query`), unparseable
params, and safesearch>2 (`bad_request`). Aggregator wiring lands next.

EOF
)"
```

---

## Task 7: Wire `/search` to the aggregator (happy path)

**Why no failing test first:** `aggregator::aggregate` makes real HTTPS calls to upstream search engines (DuckDuckGo etc.). A unit test would need network access and would be flaky in CI. The failing test for this task is an `#[ignore]`'d integration test; CI ignores it; the developer runs it manually.

**Files:**
- Modify: `src/api_server.rs`
- Modify: `tests/api_only.rs`

- [ ] **Step 7.1 — Add the ignored happy-path test**

Append to `tests/api_only.rs`:

```rust
/// Hits real DuckDuckGo. Run with: `cargo test --test api_only --no-default-features --features api-only --ignored -- --test-threads=1`
#[ignore]
#[actix_web::test]
async fn search_returns_results_from_duckduckgo() {
    let app = test::init_service(
        App::new().app_data(web::Data::new(leak_config())).configure(api_server::configure)
    ).await;
    let req = test::TestRequest::get().uri("/search?q=rust+programming+language").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = test::read_body_json(resp).await;
    assert!(body["results"].as_array().unwrap().len() > 0,
        "expected non-empty results, got {body}");
}
```

- [ ] **Step 7.2 — Replace the placeholder in `search` handler**

Two changes inside the `search` handler in `src/api_server.rs`:

a) Drop the leading underscores in the existing handler — rename `_q` → `q` and `_page` → `page` (Task 6 left them prefixed because they were unused).

b) Replace the `HttpResponse::NotImplemented()...` block at the end with:

```rust
    use crate::aggregator::aggregate;
    use crate::models::engine::EngineHandler;
    use crate::user_agent::random_user_agent;

    let engines: Vec<EngineHandler> = config.upstream_search_engines.iter()
        .filter(|(_, enabled)| **enabled)
        .filter_map(|(name, _)| EngineHandler::new(name).ok())
        .collect();

    if engines.is_empty() {
        return HttpResponse::InternalServerError().json(json!({
            "error": "no engines available",
            "code": "no_engines",
        }));
    }

    // random_user_agent is `async fn (u8) -> Result<&'static str, _>` — see src/user_agent.rs.
    let user_agent = match random_user_agent(config.threads).await {
        Ok(ua) => ua,
        Err(e) => {
            log::warn!("user_agent init error: {e}");
            return HttpResponse::InternalServerError().json(json!({
                "error": "user agent init failed",
                "code": "internal",
            }));
        }
    };

    match aggregate(&q, page, *config.get_ref(), &engines, safe_search, user_agent).await {
        Ok(results) => HttpResponse::Ok().json(results),
        Err(e) => {
            log::warn!("aggregator error: {e}");
            HttpResponse::BadGateway().json(json!({
                "error": "all engines failed",
                "code": "upstream_failed",
            }))
        }
    }
```

- [ ] **Step 7.3 — Verify all non-ignored tests still pass**

```bash
cargo test --test api_only --no-default-features --features api-only -- --test-threads=1 2>&1 | tail -20
```

Expected: 17 tests pass, 1 ignored.

- [ ] **Step 7.4 — (Optional, requires network) verify happy path**

```bash
cargo test --test api_only --no-default-features --features api-only --ignored -- --test-threads=1 search_returns_results_from_duckduckgo 2>&1 | tail -20
```

Expected: 1 test passes. Skip if no network.

- [ ] **Step 7.5 — Commit**

```bash
git add src/api_server.rs tests/api_only.rs
git commit -m "$(cat <<'EOF'
feat(api-only): wire /search to aggregator + serialize SearchResults JSON

Builds EngineHandler list from config.upstream_search_engines, calls
aggregator::aggregate, serializes the existing SearchResults type
verbatim. Aggregator failures map to 502 upstream_failed.

Live happy-path test is #[ignore]'d to avoid flakiness in CI; run with
`cargo test --ignored` against real DuckDuckGo to verify.

EOF
)"
```

---

## Task 8: Wire `src/bin/tinysurfx.rs` to actually run the server

**Files:**
- Modify: `src/bin/tinysurfx.rs`

- [ ] **Step 8.1 — Replace the stub**

Overwrite `src/bin/tinysurfx.rs` with:

```rust
//! tinysurfx — JSON-only api-only edition entry point.
//! See docs/superpowers/specs/2026-05-10-tinysurfx-api-only-edition-design.md.

use actix_cors::Cors;
use actix_governor::{Governor, GovernorConfigBuilder};
use actix_web::middleware::{Compress, Logger};
use actix_web::{App, HttpServer, web::Data};
use std::process::exit;

use websurfx::api_config::{Config, ConfigError};
use websurfx::api_server;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // CLI args minus argv[0].
    let args: Vec<String> = std::env::args().skip(1).collect();

    let config = match Config::from_env_and_args(&args) {
        Ok(c) => c,
        Err(ConfigError::HelpRequested) => {
            print!("{}", Config::help_text());
            return Ok(());
        }
        Err(e) => {
            eprintln!("error: {e}");
            exit(2);
        }
    };

    // env_logger reads RUST_LOG; honor TINYSURFX_LOG as a more obvious alias.
    if std::env::var("RUST_LOG").is_err() {
        let level = std::env::var("TINYSURFX_LOG").unwrap_or_else(|_| "info".to_string());
        // SAFETY: single-threaded at this point — actix runtime not started yet.
        unsafe { std::env::set_var("RUST_LOG", &level); }
    }
    env_logger::init();

    let config: &'static Config = Box::leak(Box::new(config));

    let governor = GovernorConfigBuilder::default()
        .seconds_per_request(config.rate_limiter.time_limit as u64)
        .burst_size(config.rate_limiter.number_of_requests as u32)
        .finish()
        .unwrap_or_else(|| {
            eprintln!("error: invalid rate limiter config");
            exit(2);
        });

    log::info!("tinysurfx {} listening on {}:{}", env!("CARGO_PKG_VERSION"), config.binding_ip, config.port);
    log::info!("engines: {:?}", config.upstream_search_engines.keys().collect::<Vec<_>>());

    let bind_addr = (config.binding_ip.as_str(), config.port);
    HttpServer::new(move || {
        App::new()
            .app_data(Data::new(config))
            .wrap(Logger::default())
            .wrap(Compress::default())
            .wrap(Cors::permissive())
            .wrap(Governor::new(&governor))
            .configure(api_server::configure)
    })
    .workers(config.threads as usize)
    .keep_alive(std::time::Duration::from_secs(config.client_connection_keep_alive as u64))
    .bind(bind_addr)?
    .run()
    .await
}
```

Notes:
- `#[actix_web::main]` macro requires `async fn main() -> std::io::Result<()>` (or compatible). Returning `ExitCode` would not work.
- All middleware (Logger, Compress, Cors, Governor) is wired here in the bin, not in `api_server::configure`. Tests don't need middleware to exercise route logic.
- `exit(2)` is used for fatal config errors before the runtime starts; once the runtime is running we propagate via `?` and let actix surface the error.

- [ ] **Step 8.2 — Build the release binary**

```bash
cargo build --bin tinysurfx --no-default-features --features api-only --profile bsr2 2>&1 | tail -10
```

Expected: succeeds. Output at `target/bsr2/tinysurfx`.

- [ ] **Step 8.3 — Smoke test the binary**

```bash
./target/bsr2/tinysurfx --help
```

Expected: prints the help text from `api_config_help.txt`.

```bash
./target/bsr2/tinysurfx --version
```

Expected: `tinysurfx 1.29.0`.

```bash
TINYSURFX_BIND=127.0.0.1:18080 ./target/bsr2/tinysurfx &
SERVER_PID=$!
sleep 1
curl -fsS http://127.0.0.1:18080/healthz | head
kill $SERVER_PID
```

Expected: `{"status":"ok","version":"1.29.0"}` printed before the kill.

- [ ] **Step 8.4 — Note the binary size**

```bash
ls -lh target/bsr2/tinysurfx
strip target/bsr2/tinysurfx 2>/dev/null && ls -lh target/bsr2/tinysurfx
```

Record the size in the commit message — this is the headline metric.

- [ ] **Step 8.5 — Commit**

```bash
git add src/bin/tinysurfx.rs
git commit -m "$(cat <<'EOF'
feat(api-only): wire tinysurfx bin to api_server with rate limiter

Reads CLI args + env, builds Config, leaks to 'static, configures
actix-governor middleware from config, binds to TINYSURFX_BIND, runs
HttpServer. --help and --version short-circuit before binding.

Stripped bsr2 binary size on linux x86_64: <FILL IN FROM STEP 8.4>.

EOF
)"
```

---

## Task 9: PATCHES.md and CONTRIBUTING.md

**Files:**
- Create: `PATCHES.md`
- Modify: `CONTRIBUTING.md`

- [ ] **Step 9.1 — Create `PATCHES.md`**

```markdown
# Patches against `neon-mmd/websurfx`

This fork (`nadecancode/tinysurfx`) is maintained as a small set of patches on top of upstream `neon-mmd/websurfx` `rolling`. New patches must be added here when introduced; obsolete patches must be removed.

When merging upstream (`.github/workflows/upstream-sync.yml` opens a PR weekly), the reviewer must verify each row below still applies cleanly. Update commit shas after squashes/rebases.

## Active patches

### P1 — api-only Cargo feature + tinysurfx bin
- **Files:** `Cargo.toml`, `src/lib.rs` (cfg gates only), `src/bin/tinysurfx.rs`
- **Purpose:** Adds api-only build target with no UI deps. Existing websurfx bin unchanged.
- **Upstream conflict risk:** low — additive cfg gates and a new bin entry.
- **Verify after rebase:** `cargo build --bin tinysurfx --no-default-features --features api-only` succeeds AND `cargo build` (default) still succeeds.

### P2 — env/CLI config (api_config.rs)
- **Files:** `src/api_config.rs`, `src/api_config_help.txt`, `src/lib.rs` (the `pub mod parser { pub use crate::api_config::Config; }` shim under `cfg(api-only)`)
- **Purpose:** Replaces mlua-based `parser::Config` with env vars + CLI flags for api-only builds.
- **Upstream conflict risk:** medium — if upstream adds a `parser::Config` field that `aggregator` reads, mirror it in `api_config::Config`.
- **Verify after rebase:** binary boots with no `config.lua` present; `cargo test --test api_only --no-default-features --features api-only -- --test-threads=1` passes.

### P3 — JSON-only API server (api_server.rs)
- **Files:** `src/api_server.rs`, `src/lib.rs` (one new `pub mod api_server;` under cfg)
- **Purpose:** Actix App + `/search` (JSON) and `/healthz` handlers.
- **Upstream conflict risk:** low — isolated new file.
- **Verify after rebase:** integration tests in `tests/api_only.rs` pass.

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
```

- [ ] **Step 9.2 — Append to `CONTRIBUTING.md`**

Read the file first to find a sensible insertion point. At the end of the file, append:

```markdown

## Maintaining the api-only edition (tinysurfx fork)

This fork carries a small set of patches against `neon-mmd/websurfx` to ship an api-only edition. See [`PATCHES.md`](./PATCHES.md) for the catalog.

### One-time setup

```bash
git remote add upstream https://github.com/neon-mmd/websurfx.git
git fetch upstream
```

### Pulling upstream changes

A scheduled GitHub Action (`.github/workflows/upstream-sync.yml`) runs weekly and opens a PR titled `chore: merge upstream/rolling`. To run it manually:

1. GitHub → Actions → "Sync upstream" → "Run workflow".
2. Review the PR. If clean, merge. If conflicts, resolve them locally and push.
3. After merging, walk through `PATCHES.md` and verify each patch still applies (the verification command for each patch is in its `Verify after rebase` line).

### Local-only manual sync

```bash
git fetch upstream
git checkout -b upstream-sync/$(date +%Y-%m-%d)
git merge upstream/rolling
# resolve conflicts if any, then:
cargo build && cargo build --bin tinysurfx --no-default-features --features api-only
git push origin HEAD
gh pr create --base rolling --title "chore: merge upstream/rolling"
```
```

- [ ] **Step 9.3 — Commit**

```bash
git add PATCHES.md CONTRIBUTING.md
git commit -m "$(cat <<'EOF'
docs: PATCHES.md catalog + CONTRIBUTING upstream-sync section

PATCHES.md enumerates every commit that diverges from neon-mmd/websurfx,
with conflict-risk ratings and per-patch verify-after-rebase commands.
P6 (relevance scoring gate) flagged as high risk.

CONTRIBUTING.md documents the upstream remote setup and the manual /
scheduled merge workflow.

EOF
)"
```

---

## Task 10: `build-binaries.yml` workflow

**Files:**
- Create: `.github/workflows/build-binaries.yml`

- [ ] **Step 10.1 — Write the workflow**

Create `.github/workflows/build-binaries.yml`:

```yaml
name: Build api-only binaries

on:
  push:
    branches: ['**']
    tags: ['v*']
  workflow_dispatch:

concurrency:
  group: build-binaries-${{ github.ref }}
  cancel-in-progress: true

permissions:
  contents: write   # required for tag-triggered Release publish

jobs:
  build:
    name: ${{ matrix.asset }}
    runs-on: ${{ matrix.runner }}
    strategy:
      fail-fast: false
      matrix:
        include:
          - asset: linux-x86_64
            runner: ubuntu-latest
            target: x86_64-unknown-linux-musl
            ext: ""
            apt: musl-tools
          - asset: linux-aarch64
            runner: ubuntu-24.04-arm
            target: aarch64-unknown-linux-musl
            ext: ""
            apt: musl-tools
          - asset: macos-aarch64
            runner: macos-latest
            target: aarch64-apple-darwin
            ext: ""
            apt: ""
          - asset: macos-x86_64
            runner: macos-13
            target: x86_64-apple-darwin
            ext: ""
            apt: ""
          - asset: windows-x86_64
            runner: windows-latest
            target: x86_64-pc-windows-msvc
            ext: ".exe"
            apt: ""
    steps:
      - uses: actions/checkout@v6

      - name: Install musl-tools
        if: matrix.apt != ''
        run: sudo apt-get update && sudo apt-get install -y ${{ matrix.apt }}

      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - uses: Swatinem/rust-cache@v2
        with:
          key: ${{ matrix.target }}

      - name: Build
        env:
          CC_x86_64_unknown_linux_musl: musl-gcc
          CC_aarch64_unknown_linux_musl: musl-gcc
        run: cargo build --bin tinysurfx --no-default-features --features api-only --profile bsr2 --target ${{ matrix.target }}

      - name: Strip (unix)
        if: runner.os != 'Windows'
        run: strip target/${{ matrix.target }}/bsr2/tinysurfx${{ matrix.ext }} || true

      - name: Stage artifact
        shell: bash
        run: |
          mkdir -p dist
          NAME="tinysurfx-${{ matrix.asset }}${{ matrix.ext }}"
          cp "target/${{ matrix.target }}/bsr2/tinysurfx${{ matrix.ext }}" "dist/$NAME"
          (cd dist && shasum -a 256 "$NAME" > "$NAME.sha256")

      - uses: actions/upload-artifact@v7
        with:
          name: tinysurfx-${{ matrix.asset }}
          path: dist/*
          retention-days: 14

  publish:
    name: Publish release assets
    if: startsWith(github.ref, 'refs/tags/v')
    needs: build
    runs-on: ubuntu-latest
    permissions:
      contents: write
    steps:
      - uses: actions/download-artifact@v8
      - uses: softprops/action-gh-release@v3
        with:
          token: ${{ secrets.ADMIN_RIGHTS_TOKEN }}
          generate_release_notes: true
          files: |
            tinysurfx-*/tinysurfx-*
```

- [ ] **Step 10.2 — Commit + push and watch the action**

```bash
git add .github/workflows/build-binaries.yml
git commit -m "$(cat <<'EOF'
ci: cross-platform build matrix for tinysurfx api-only bin

Triggers: push to any branch, tag push (v*), workflow_dispatch.
Targets: linux musl x86_64/aarch64, macos arm64/x86_64, windows msvc.
Tag pushes also publish artifacts to a GitHub Release via softprops.

EOF
)"
git push origin HEAD
```

- [ ] **Step 10.3 — Verify the workflow ran**

```bash
gh run list --workflow=build-binaries.yml --limit=1
gh run view --log-failed   # only if it failed
```

Expected: all 5 matrix jobs go green. If any fail, address per-target issues (most common: missing system tooling in Linux runners, signing issues on macOS — should be none for unsigned binaries).

---

## Task 11: `upstream-sync.yml` workflow

**Files:**
- Create: `.github/workflows/upstream-sync.yml`

- [ ] **Step 11.1 — Write the workflow**

```yaml
name: Sync upstream

on:
  schedule:
    - cron: '0 8 * * 1'  # Mondays 08:00 UTC
  workflow_dispatch:

permissions:
  contents: write
  pull-requests: write

jobs:
  sync:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v6
        with:
          ref: rolling
          fetch-depth: 0
          token: ${{ secrets.ADMIN_RIGHTS_TOKEN }}

      - name: Configure git
        run: |
          git config user.name "tinysurfx-bot"
          git config user.email "tinysurfx-bot@users.noreply.github.com"

      - name: Add upstream remote and fetch
        run: |
          git remote add upstream https://github.com/neon-mmd/websurfx.git
          git fetch upstream rolling

      - name: Create sync branch
        id: branch
        run: |
          BRANCH="upstream-sync/$(date -u +%Y-%m-%d)"
          if git ls-remote --exit-code --heads origin "$BRANCH"; then
            echo "Branch $BRANCH already exists, skipping."
            echo "skip=true" >> "$GITHUB_OUTPUT"
            exit 0
          fi
          git checkout -b "$BRANCH"
          echo "branch=$BRANCH" >> "$GITHUB_OUTPUT"

      - name: Merge upstream/rolling
        if: steps.branch.outputs.skip != 'true'
        id: merge
        run: |
          set +e
          git merge upstream/rolling --no-ff --no-edit
          STATUS=$?
          set -e
          if [ "$STATUS" -ne 0 ]; then
            CONFLICTS=$(git diff --name-only --diff-filter=U)
            git add -A
            git commit --no-verify -m "chore: merge upstream/rolling (conflicts pending resolution)"
            echo "conflicts<<EOF" >> "$GITHUB_OUTPUT"
            echo "$CONFLICTS" >> "$GITHUB_OUTPUT"
            echo "EOF" >> "$GITHUB_OUTPUT"
          fi

      - name: Push and open PR
        if: steps.branch.outputs.skip != 'true'
        env:
          GH_TOKEN: ${{ secrets.ADMIN_RIGHTS_TOKEN }}
        run: |
          git push origin "${{ steps.branch.outputs.branch }}"

          BODY=$(cat <<'EOM'
          Automated weekly sync from `upstream/rolling`.

          ## Per-patch verification checklist (from PATCHES.md)
          - [ ] **P1** — `cargo build --bin tinysurfx --no-default-features --features api-only` succeeds
          - [ ] **P1** — `cargo build` (default features) still succeeds
          - [ ] **P2** — `cargo test --test api_only --no-default-features --features api-only -- --test-threads=1` passes
          - [ ] **P3** — integration tests still pass
          - [ ] **P4** — `build-binaries.yml` workflow runs green for this branch
          - [ ] **P5** — N/A (workflow file)
          - [ ] **P6 (HIGH RISK)** — confirm `value.calculate_relevance` cfg gate still applies; confirm `models/aggregation.rs` cfg gates still apply
          EOM
          )

          if [ -n "${{ steps.merge.outputs.conflicts }}" ]; then
            BODY="$BODY

          ## Conflicts to resolve
          \`\`\`
          ${{ steps.merge.outputs.conflicts }}
          \`\`\`"
          fi

          gh pr create \
            --base rolling \
            --head "${{ steps.branch.outputs.branch }}" \
            --title "chore: merge upstream/rolling" \
            --body "$BODY" \
            --label upstream-sync
```

- [ ] **Step 11.2 — Commit + push**

```bash
git add .github/workflows/upstream-sync.yml
git commit -m "$(cat <<'EOF'
ci: weekly upstream-sync workflow with PATCHES.md verification checklist

Mondays 08:00 UTC. Creates upstream-sync/<date> branch, merges
upstream/rolling, opens a PR pre-populated with the per-patch
verify-after-rebase checklist. Conflict markers committed inside the PR
so a reviewer can resolve in-browser or locally; workflow does not fail
on conflicts.

EOF
)"
git push origin HEAD
```

- [ ] **Step 11.3 — Manual smoke test**

```bash
gh workflow run "Sync upstream"
sleep 30
gh run list --workflow=upstream-sync.yml --limit=1
```

Expected: a workflow run appears. If no upstream changes since last sync, the action no-ops gracefully. If there are changes, a PR appears.

---

## Final verification

- [ ] **All tests pass:**

```bash
cargo test --test api_only --no-default-features --features api-only -- --test-threads=1
cargo build --bin websurfx                                # HTML edition still builds
cargo build --bin tinysurfx --no-default-features --features api-only --profile bsr2
```

- [ ] **Live happy path:**

```bash
TINYSURFX_BIND=127.0.0.1:18080 ./target/bsr2/tinysurfx &
sleep 2
curl -fsS 'http://127.0.0.1:18080/healthz'
curl -fsS 'http://127.0.0.1:18080/search?q=rust' | jq '.results | length'
curl -fsS -i 'http://127.0.0.1:18080/search' | head -1   # expect 400
curl -fsS -i 'http://127.0.0.1:18080/search?q=hi&safesearch=3' | head -1  # expect 400
kill %1
```

- [ ] **CI green:** `gh run list --workflow=build-binaries.yml --limit=1` shows the latest run as `completed/success`.

- [ ] **PATCHES.md is current:** every commit that touches a non-new file (Cargo.toml, src/lib.rs, src/aggregator.rs, src/models/aggregation.rs) is reflected in PATCHES.md.

- [ ] **Push final state:**

```bash
git push origin rolling
```
