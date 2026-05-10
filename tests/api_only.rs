//! Integration tests for the api-only edition.

#![cfg(feature = "api-only")]
#![allow(unsafe_code)]

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
