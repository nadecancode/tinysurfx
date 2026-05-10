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
    /// IP address the HTTP server binds to.
    pub binding_ip: String,
    /// TCP port the HTTP server listens on.
    pub port: u16,
    /// Number of Actix worker threads.
    pub threads: u8,
    /// Per-request timeout (seconds) for upstream engine HTTP calls.
    pub request_timeout: u8,
    /// TCP keep-alive (seconds) for upstream connections.
    pub tcp_connection_keep_alive: u8,
    /// Idle timeout (seconds) for pooled upstream HTTPS connections.
    pub pool_idle_connection_timeout: u8,
    /// Maximum number of pooled upstream HTTPS connections.
    pub number_of_https_connections: u8,
    /// Whether to use the operating system's TLS certificate store.
    pub operating_system_tls_certificates: bool,
    /// Whether HTTP/2 adaptive flow-control window is enabled.
    pub adaptive_window: bool,
    /// Optional outbound proxy used when fetching upstream engines.
    pub proxy: Option<reqwest::Proxy>,
    /// Safe-search level (0-4) applied to upstream queries.
    pub safe_search: u8,
    /// Map of upstream search engine name to enabled flag.
    pub upstream_search_engines: HashMap<String, bool>,
    /// Keep-alive (seconds) for inbound client connections to the API server.
    pub client_connection_keep_alive: u8,
    /// Inbound rate limiter configuration.
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

use std::env;
use std::str::FromStr;

/// Errors that can occur while parsing env vars or CLI flags.
#[derive(Debug)]
pub enum ConfigError {
    /// `TINYSURFX_BIND` was not in `host:port` form.
    BadBind(String),
    /// An integer-valued env var or flag did not parse.
    BadInt(String, String),
    /// A boolean-valued env var or flag did not parse.
    BadBool(String, String),
    /// `TINYSURFX_PROXY` was not a valid proxy URL.
    BadProxy(String),
    /// An unknown CLI flag was supplied or required value was missing.
    BadFlag(String),
    /// User passed `--help` / `-h`. Caller should print help and exit 0.
    HelpRequested,
    /// Safesearch level 3 or 4 requires allowlist/blocklist files; not supported in v1.
    SafesearchUnsupported(u8),
    /// A configuration value failed range/sanity validation.
    InvalidValue(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadBind(s) => write!(f, "TINYSURFX_BIND must be `host:port`, got {s:?}"),
            Self::BadInt(k, s) => write!(f, "{k} must be an integer, got {s:?}"),
            Self::BadBool(k, s) => write!(f, "{k} must be true|false, got {s:?}"),
            Self::BadProxy(s) => write!(f, "TINYSURFX_PROXY is not a valid URL: {s}"),
            Self::BadFlag(s) => write!(f, "{s}"),
            Self::HelpRequested => write!(f, "help requested"),
            Self::SafesearchUnsupported(n) => write!(f, "safesearch level {n} is not supported in the api-only edition (allowed: 0..=2)"),
            Self::InvalidValue(s) => write!(f, "{s}"),
        }
    }
}

impl std::error::Error for ConfigError {}

impl Config {
    /// Build a `Config` from environment variables, falling back to `defaults()`
    /// for any unset var. Validates types but does not yet validate value
    /// ranges (Task 4).
    pub fn from_env() -> Result<Self, ConfigError> {
        let mut cfg = Self::defaults();

        if let Ok(bind) = env::var("TINYSURFX_BIND") {
            let (host, port) = bind.rsplit_once(':').ok_or_else(|| ConfigError::BadBind(bind.clone()))?;
            // Strip IPv6 brackets if present (e.g. "[::1]" -> "::1").
            let host = host.strip_prefix('[').and_then(|h| h.strip_suffix(']')).unwrap_or(host);
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

/// Parse an integer-typed env var, returning the default if unset.
fn parse_env_int<T: FromStr>(key: &str, default: T) -> Result<T, ConfigError> {
    match env::var(key) {
        Ok(s) => s.parse().map_err(|_| ConfigError::BadInt(key.to_string(), s)),
        Err(_) => Ok(default),
    }
}

/// Parse a bool-typed env var (true|1|yes|on / false|0|no|off), returning the default if unset.
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

/// Help text rendered by `--help`. Embedded at compile time.
const HELP_TEXT: &str = include_str!("api_config_help.txt");

impl Config {
    /// Build a `Config` from env vars then apply CLI overrides on top.
    /// Returns `ConfigError::HelpRequested` if the user passed `--help` / `-h` —
    /// the caller is expected to print `help_text()` and exit 0.
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
                    let h = h.strip_prefix('[').and_then(|s| s.strip_suffix(']')).unwrap_or(h);
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
                other => return Err(ConfigError::BadFlag(format!("unknown flag {other:?}"))),
            }
        }
        if cfg.safe_search > 2 {
            return Err(ConfigError::SafesearchUnsupported(cfg.safe_search));
        }
        if cfg.threads == 0 {
            return Err(ConfigError::InvalidValue("threads must be > 0".into()));
        }
        if cfg.upstream_search_engines.is_empty() {
            return Err(ConfigError::InvalidValue("at least one engine must be enabled".into()));
        }
        Ok(cfg)
    }

    /// Help text printed by the bin in response to `--help` / `-h`.
    pub fn help_text() -> &'static str { HELP_TEXT }
}

/// Consume the next CLI arg as an integer, returning `BadFlag` if missing or `BadInt` if unparseable.
fn take_int<'a, I, T>(iter: &mut I, name: &str) -> Result<T, ConfigError>
where I: Iterator<Item = &'a String>, T: FromStr {
    let v = iter.next().ok_or_else(|| ConfigError::BadFlag(format!("{name} needs value")))?;
    v.parse().map_err(|_| ConfigError::BadInt(name.to_string(), v.clone()))
}
