//! This module provides the functionality to generate random user agent string.

#[cfg(not(feature = "api-only"))]
use fake_useragent::{Browsers, UserAgents, UserAgentsBuilder};
#[cfg(not(feature = "api-only"))]
use tokio::sync::OnceCell;

/// A static variable which stores the initially build `UserAgents` struct. So as it can be resused
/// again and again without the need of reinitializing the `UserAgents` struct.
#[cfg(not(feature = "api-only"))]
static USER_AGENTS: OnceCell<UserAgents> = OnceCell::const_new();

/// A function to generate random user agent to improve privacy of the user.
///
/// # Returns
///
/// A randomly generated user agent string.
#[cfg(not(feature = "api-only"))]
pub async fn random_user_agent(threads: u8) -> Result<&'static str, Box<dyn std::error::Error>> {
    Ok(USER_AGENTS
        .get_or_try_init(|| async move {
            tokio::task::spawn_blocking(move || {
                UserAgentsBuilder::new()
                    .cache(false)
                    .dir("/tmp")
                    .thread(threads as u32)
                    .set_browsers(
                        Browsers::new()
                            .set_chrome()
                            .set_safari()
                            .set_edge()
                            .set_firefox()
                            .set_mozilla(),
                    )
                    .build()
            })
            .await
        })
        .await?
        .random())
}

// ---------------------------------------------------------------------------
// api-only edition: vendored static UA list
// ---------------------------------------------------------------------------
//
// The upstream `fake-useragent` crate scrapes useragentstring.com at runtime
// and depends on a 2019-vintage `reqwest 0.9` that pulls in `native-tls` →
// `openssl-sys`. For the api-only edition we vendor a curated list of recent
// browser UAs, eliminating the dep entirely (no openssl-sys, no /tmp cache,
// no startup network call).
//
// Refresh roughly yearly; concept lifted from
// https://github.com/aurexav/fake-useragent (which scrapes the same source).

/// Drop-in replacement for the upstream impl. The `_threads` parameter is
/// ignored (no parallel scraping needed). Returns a randomly-selected UA
/// from the bundled list on each call.
#[cfg(feature = "api-only")]
pub async fn random_user_agent(_threads: u8) -> Result<&'static str, Box<dyn std::error::Error>> {
    Ok(pick_user_agent())
}

/// Picks a UA from `USER_AGENTS` using nanosecond-granularity entropy from the
/// system clock. Not cryptographic; sufficient for rotating UA strings across
/// requests served by the same process.
#[cfg(feature = "api-only")]
fn pick_user_agent() -> &'static str {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as usize)
        .unwrap_or(0);
    USER_AGENTS
        .get(nanos % USER_AGENTS.len())
        .copied()
        .unwrap_or(USER_AGENTS[0])
}

/// Curated list of recent (early-2026) desktop browser UAs across Chrome,
/// Firefox, Safari and Edge on Windows, macOS and Linux. Refresh roughly
/// once a year so they stay plausible to upstream search engines.
#[cfg(feature = "api-only")]
const USER_AGENTS: &[&str] = &[
    // Chrome — Windows
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/132.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36",
    // Chrome — macOS
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/132.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
    // Chrome — Linux
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/132.0.0.0 Safari/537.36",
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
    // Firefox — Windows
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:134.0) Gecko/20100101 Firefox/134.0",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:133.0) Gecko/20100101 Firefox/133.0",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:128.0) Gecko/20100101 Firefox/128.0",
    // Firefox — macOS
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 14.7; rv:134.0) Gecko/20100101 Firefox/134.0",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 14.7; rv:133.0) Gecko/20100101 Firefox/133.0",
    // Firefox — Linux
    "Mozilla/5.0 (X11; Linux x86_64; rv:134.0) Gecko/20100101 Firefox/134.0",
    "Mozilla/5.0 (X11; Linux x86_64; rv:133.0) Gecko/20100101 Firefox/133.0",
    "Mozilla/5.0 (X11; Linux x86_64; rv:128.0) Gecko/20100101 Firefox/128.0",
    // Safari — macOS
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.2 Safari/605.1.15",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.6 Safari/605.1.15",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Safari/605.1.15",
    // Edge — Windows
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/132.0.0.0 Safari/537.36 Edg/132.0.0.0",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 Edg/131.0.0.0",
    // Edge — macOS
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/132.0.0.0 Safari/537.36 Edg/132.0.0.0",
];
