//! tinysurfx — JSON-only api-only edition entry point.
//! See docs/superpowers/specs/2026-05-10-tinysurfx-api-only-edition-design.md.

#![allow(unsafe_code)]

use std::process::exit;

use websurfx::api_config::{Config, ConfigError};
use websurfx::api_server;

fn main() -> std::io::Result<()> {
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
        // SAFETY: single-threaded at this point — tokio runtime not started yet.
        unsafe {
            std::env::set_var("RUST_LOG", &level);
        }
    }
    env_logger::init();

    let config: &'static Config = Box::leak(Box::new(config));

    api_server::serve(config)
}
