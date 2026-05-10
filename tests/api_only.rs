//! Integration tests for the api-only edition.

#![cfg(feature = "api-only")]
#![allow(unsafe_code)]

use websurfx::api_config::{Config, ConfigError};

use std::sync::{Mutex, MutexGuard};

/// Serializes env-mutating tests so they're correct regardless of --test-threads.
/// Using a global mutex is more robust than relying on the cargo flag.
static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Acquire the env lock. Tolerate poisoning (a panicking test still leaves env in a known state via EnvGuard's Drop).
fn env_lock() -> MutexGuard<'static, ()> {
    ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

#[test]
fn from_env_uses_defaults_when_unset() {
    let _lock = env_lock();
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
    let _lock = env_lock();
    let _guard = EnvGuard::set("TINYSURFX_BIND", "0.0.0.0:9090");
    let cfg = Config::from_env().expect("ok");
    assert_eq!(cfg.binding_ip, "0.0.0.0");
    assert_eq!(cfg.port, 9090);
}

#[test]
fn from_env_handles_ipv6_bracketed_bind() {
    let _lock = env_lock();
    let _guard = EnvGuard::set("TINYSURFX_BIND", "[::1]:8080");
    let cfg = Config::from_env().expect("ok");
    assert_eq!(cfg.binding_ip, "::1");
    assert_eq!(cfg.port, 8080);
}

#[test]
fn from_env_reads_engines_csv() {
    let _lock = env_lock();
    let _guard = EnvGuard::set("TINYSURFX_ENGINES", "duckduckgo,brave,searx");
    let cfg = Config::from_env().expect("ok");
    assert_eq!(cfg.upstream_search_engines.len(), 3);
    assert_eq!(cfg.upstream_search_engines.get("brave"), Some(&true));
}

#[test]
fn from_env_reads_numeric_fields() {
    let _lock = env_lock();
    let _g1 = EnvGuard::set("TINYSURFX_THREADS", "8");
    let _g2 = EnvGuard::set("TINYSURFX_REQUEST_TIMEOUT_SECS", "45");
    let _g3 = EnvGuard::set("TINYSURFX_RATE_LIMIT_RPS", "100");
    let cfg = Config::from_env().expect("ok");
    assert_eq!(cfg.threads, 8);
    assert_eq!(cfg.request_timeout, 45);
    assert_eq!(cfg.rate_limiter.number_of_requests, 100);
}

#[test]
fn cli_flag_overrides_env() {
    let _lock = env_lock();
    let _g = EnvGuard::set("TINYSURFX_BIND", "127.0.0.1:7000");
    let args = vec!["--bind".to_string(), "0.0.0.0:9999".to_string()];
    let cfg = Config::from_env_and_args(&args).expect("ok");
    assert_eq!(cfg.binding_ip, "0.0.0.0");
    assert_eq!(cfg.port, 9999);
}

#[test]
fn cli_engines_replaces_env() {
    let _lock = env_lock();
    let _g = EnvGuard::set("TINYSURFX_ENGINES", "duckduckgo");
    let args = vec!["--engines".to_string(), "brave,searx".to_string()];
    let cfg = Config::from_env_and_args(&args).expect("ok");
    assert_eq!(cfg.upstream_search_engines.len(), 2);
    assert!(cfg.upstream_search_engines.contains_key("searx"));
}

#[test]
fn cli_help_returns_help_request() {
    let _lock = env_lock();
    let args = vec!["--help".to_string()];
    let err = Config::from_env_and_args(&args);
    assert!(matches!(err, Err(ConfigError::HelpRequested)));
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
        // SAFETY: tests acquire ENV_LOCK before mutating env (see env_lock()).
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
