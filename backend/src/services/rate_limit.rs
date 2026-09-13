use axum::{
    extract::ConnectInfo,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Extension,
};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::Instant,
};

struct Entry {
    count: u32,
    window_start: Instant,
}

#[derive(Clone)]
pub struct RateLimiter {
    state: Arc<Mutex<HashMap<String, Entry>>>,
    max_requests: u32,
    window_secs: u64,
}

impl RateLimiter {
    pub fn new(max_requests: u32, window_secs: u64) -> Self {
        Self {
            state: Arc::new(Mutex::new(HashMap::new())),
            max_requests,
            window_secs,
        }
    }

    fn check(&self, key: &str) -> bool {
        // Never propagate a poisoned mutex: a single panic while the lock was
        // held would otherwise take down every auth endpoint until restart.
        let mut map = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();

        // Clean up stale entries every 100 checks.
        // `checked_sub` because on a freshly booted host `Instant::now()` can be
        // smaller than the window, and plain subtraction would panic.
        if map.len() > 100 {
            if let Some(cutoff) = now.checked_sub(std::time::Duration::from_secs(self.window_secs.saturating_mul(2))) {
                map.retain(|_, entry| entry.window_start > cutoff);
            }
        }
        let entry = map.entry(key.to_string()).or_insert(Entry {
            count: 0,
            window_start: now,
        });
        if now.duration_since(entry.window_start).as_secs() >= self.window_secs {
            entry.count = 1;
            entry.window_start = now;
            true
        } else if entry.count < self.max_requests {
            entry.count += 1;
            true
        } else {
            false
        }
    }
}

/// Identify the client for rate-limiting purposes.
///
/// Behind a reverse proxy every request arrives from the proxy's address, so
/// keying on the socket address alone puts all users in one shared bucket:
/// a single noisy client locks everyone out of /auth, while credential
/// stuffing from many IPs stays unthrottled.
///
/// `TRUST_PROXY_HEADERS=1` opts into using the left-most X-Forwarded-For entry.
/// It is off by default because a client can forge that header when the service
/// is exposed directly.
fn client_key(addr: &SocketAddr, headers: &axum::http::HeaderMap) -> String {
    if std::env::var("TRUST_PROXY_HEADERS").map(|v| v == "1").unwrap_or(false) {
        if let Some(fwd) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
            if let Some(first) = fwd.split(',').next() {
                let candidate = first.trim();
                if !candidate.is_empty() {
                    return candidate.to_string();
                }
            }
        }
    }
    addr.ip().to_string()
}

pub async fn rate_limit_middleware(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Extension(limiter): Extension<RateLimiter>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let key = client_key(&addr, request.headers());
    if limiter.check(&key) {
        next.run(request).await
    } else {
        (StatusCode::TOO_MANY_REQUESTS, "rate limit exceeded").into_response()
    }
}
