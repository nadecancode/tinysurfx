//! tinysurfx — JSON-only api-only edition entry point.
//! See docs/superpowers/specs/2026-05-10-tinysurfx-api-only-edition-design.md.

#![allow(unsafe_code)]

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
