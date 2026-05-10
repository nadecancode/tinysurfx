//! JSON-only route handlers for the api-only edition.

use actix_web::{HttpResponse, get, web::ServiceConfig};
use serde_json::json;

/// Register all routes on the provided `ServiceConfig`. Both the bin
/// entry point and integration tests call this so the same set of
/// handlers is exercised in both places.
pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(healthz);
    // /search added in Task 6.
}

/// Liveness probe. Always 200, no auth, no rate limit.
#[get("/healthz")]
async fn healthz() -> HttpResponse {
    HttpResponse::Ok().json(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}
