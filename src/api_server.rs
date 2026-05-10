//! JSON-only HTTP routes for the api-only edition.
//!
//! Uses [`tiny_http`] (sync, ~100 KB compiled) instead of actix-web (~3-4 MB
//! compiled stack) for the smallest possible binary footprint. Bridges sync
//! request handling to the async aggregator via a tokio runtime owned by
//! [`serve`].

use std::io;
use std::sync::Arc;

use serde_json::{Value, json};
use tiny_http::{Header, Request, Response, Server};

use crate::api_config::Config;

/// Bind to `config.binding_ip:config.port` and serve requests until the
/// process is interrupted. Each incoming request is dispatched to a fresh
/// OS thread which `block_on`s the shared multi-thread tokio runtime —
/// bridging sync tiny-http to the async aggregator without wedging it.
pub fn serve(config: &'static Config) -> io::Result<()> {
    let rt = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(config.threads as usize)
            .enable_all()
            .build()?,
    );
    let server = Server::http((config.binding_ip.as_str(), config.port)).map_err(|e| {
        io::Error::other(format!(
            "bind {}:{}: {e}",
            config.binding_ip, config.port
        ))
    })?;
    log::info!(
        "tinysurfx {} listening on {}:{}",
        env!("CARGO_PKG_VERSION"),
        config.binding_ip,
        config.port
    );
    log::info!(
        "engines: {:?}",
        config.upstream_search_engines.keys().collect::<Vec<_>>()
    );

    for request in server.incoming_requests() {
        let rt = rt.clone();
        std::thread::spawn(move || {
            rt.block_on(serve_request(request, config));
        });
    }
    Ok(())
}

/// Per-request handler. Splits the URL into path + query, dispatches via
/// [`route`], serializes the response, and writes it back.
async fn serve_request(request: Request, config: &'static Config) {
    let url = request.url().to_string();
    let (path, query) = url.split_once('?').unwrap_or((url.as_str(), ""));
    let method = method_name(&request);

    let (status, body) = route(method, path, query, config).await;

    let body_bytes = serde_json::to_vec(&body).unwrap_or_else(|_| b"{}".to_vec());
    let mut response = Response::from_data(body_bytes).with_status_code(status);
    if let Ok(h) = Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]) {
        response = response.with_header(h);
    }
    if let Ok(h) = Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]) {
        response = response.with_header(h);
    }

    let log_line = format!("{method} {url} -> {status}");
    if let Err(e) = request.respond(response) {
        log::warn!("respond error: {e} ({log_line})");
    } else {
        log::info!("{log_line}");
    }
}

/// Pure router exposed for unit-testing without binding a port. Dispatches
/// by `(method, path)` and returns `(status, body)`. Handler bodies are
/// async because `/search` calls into the async aggregator.
pub async fn route(
    method: &str,
    path: &str,
    query: &str,
    config: &Config,
) -> (u16, Value) {
    match (method, path) {
        ("GET", "/healthz") => (
            200,
            json!({
                "status": "ok",
                "version": env!("CARGO_PKG_VERSION"),
            }),
        ),
        ("GET", "/search") => handle_search(query, config).await,
        _ => (404, json!({"error": "not found", "code": "not_found"})),
    }
}

/// Async handler for `GET /search`. Validates inputs, builds engine handlers,
/// fetches a UA, calls the aggregator, returns `(status, body)`.
pub async fn handle_search(query_str: &str, config: &Config) -> (u16, Value) {
    use crate::aggregator::aggregate;
    use crate::models::engine::EngineHandler;
    use crate::user_agent::random_user_agent;

    let mut q: Option<String> = None;
    let mut page: u32 = 0;
    let mut safesearch_param: Option<u8> = None;

    for (k, v) in form_urlencoded::parse(query_str.as_bytes()) {
        match k.as_ref() {
            "q" => q = Some(v.into_owned()),
            "page" => match v.parse::<u32>() {
                Ok(n) => page = n,
                Err(_) => return bad_request("page must be a non-negative integer"),
            },
            "safesearch" => match v.parse::<u8>() {
                Ok(n) => safesearch_param = Some(n),
                Err(_) => return bad_request("safesearch must be 0..=2"),
            },
            _ => {}
        }
    }

    let trimmed = q
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
    let q = match trimmed {
        Some(s) => s,
        None => {
            return (
                400,
                json!({"error": "missing query", "code": "empty_query"}),
            );
        }
    };

    let safe_search = safesearch_param.unwrap_or(config.safe_search);
    if safe_search > 2 {
        return bad_request(
            "safesearch level not supported in api-only edition (allowed: 0..=2)",
        );
    }

    let engines: Vec<EngineHandler> = config
        .upstream_search_engines
        .iter()
        .filter(|(_, enabled)| **enabled)
        .filter_map(|(name, _)| EngineHandler::new(name).ok())
        .collect();
    if engines.is_empty() {
        return (
            500,
            json!({"error": "no engines available", "code": "no_engines"}),
        );
    }

    let user_agent = match random_user_agent(config.threads).await {
        Ok(ua) => ua,
        Err(e) => {
            log::warn!("user_agent init error: {e}");
            return (
                500,
                json!({"error": "user agent init failed", "code": "internal"}),
            );
        }
    };

    match aggregate(&q, page, config, &engines, safe_search, user_agent).await {
        Ok(results) => match serde_json::to_value(&results) {
            Ok(v) => (200, v),
            Err(e) => {
                log::warn!("serialize results: {e}");
                (
                    500,
                    json!({"error": "serialize failed", "code": "internal"}),
                )
            }
        },
        Err(e) => {
            log::warn!("aggregator error: {e}");
            (
                502,
                json!({"error": "all engines failed", "code": "upstream_failed"}),
            )
        }
    }
}

/// Helper: structured 400 with a `bad_request` code.
fn bad_request(msg: &str) -> (u16, Value) {
    (400, json!({"error": msg, "code": "bad_request"}))
}

/// Map tiny-http's `Method` enum into the literal string we route on.
/// Keeps [`route`] testable without depending on tiny-http types.
fn method_name(request: &Request) -> &'static str {
    use tiny_http::Method;
    match request.method() {
        Method::Get => "GET",
        Method::Head => "HEAD",
        Method::Post => "POST",
        Method::Put => "PUT",
        Method::Delete => "DELETE",
        Method::Patch => "PATCH",
        Method::Options => "OPTIONS",
        Method::Connect => "CONNECT",
        Method::Trace => "TRACE",
        Method::NonStandard(_) => "OTHER",
    }
}
