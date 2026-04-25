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
        let mut map = self.state.lock().unwrap();
        let now = Instant::now();

        // Clean up stale entries every 100 checks
        if map.len() > 100 {
            let cutoff = now - std::time::Duration::from_secs(self.window_secs * 2);
            map.retain(|_, entry| entry.window_start > cutoff);
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

pub async fn rate_limit_middleware(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Extension(limiter): Extension<RateLimiter>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let key = addr.ip().to_string();
    if limiter.check(&key) {
        next.run(request).await
    } else {
        (StatusCode::TOO_MANY_REQUESTS, "rate limit exceeded").into_response()
    }
}
