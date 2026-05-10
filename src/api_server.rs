//! JSON-only route handlers for the api-only edition.

use actix_web::{HttpRequest, HttpResponse, get, web, web::ServiceConfig};
use serde::Deserialize;
use serde_json::json;

use crate::api_config::Config;

/// Register all routes on the provided `ServiceConfig`. Both the bin
/// entry point and integration tests call this so the same set of
/// handlers is exercised in both places.
pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(healthz);
    cfg.service(search);
}

/// Liveness probe. Always 200, no auth, no rate limit.
#[get("/healthz")]
async fn healthz() -> HttpResponse {
    HttpResponse::Ok().json(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

/// Query parameters accepted by `/search`. Decoded manually so parse
/// failures can return a JSON 400 instead of actix's default text/plain.
#[derive(Deserialize)]
struct SearchQuery {
    /// Required search query. Empty/whitespace-only treated as missing.
    q: Option<String>,
    /// Zero-indexed page number. Defaults to 0.
    page: Option<u32>,
    /// Optional per-request safesearch override. Falls back to `config.safe_search`.
    safesearch: Option<u8>,
}

/// JSON search endpoint. Validates inputs; aggregator wiring lands in Task 7.
#[get("/search")]
async fn search(req: HttpRequest, config: web::Data<&'static Config>) -> HttpResponse {
    let parsed = web::Query::<SearchQuery>::from_query(req.query_string());
    let params = match parsed {
        Ok(p) => p.into_inner(),
        Err(e) => return bad_request(&e.to_string()),
    };

    let q = params.q.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let _q = match q {
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
    // Aggregator wiring lands in Task 7.
    HttpResponse::NotImplemented().json(json!({
        "error": "search not yet wired",
        "code": "not_implemented",
    }))
}

/// Helper for emitting a structured `bad_request` JSON response.
fn bad_request(msg: &str) -> HttpResponse {
    HttpResponse::BadRequest().json(json!({"error": msg, "code": "bad_request"}))
}
