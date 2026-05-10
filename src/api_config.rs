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
