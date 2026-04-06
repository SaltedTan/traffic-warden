use reqwest::Client;
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

#[cfg(feature = "bench")]
pub const CAPACITY: f32 = 1_000_000.0;
#[cfg(not(feature = "bench"))]
pub const CAPACITY: f32 = 5.0;

pub const REFILL_RATE: f32 = 5.0 / 60.0;
pub const NUM_SHARDS: usize = 64;

pub struct ClientState {
    pub tokens: f32,
    pub last_updated: Instant,
}

#[cfg(not(feature = "single-lock"))]
pub type RateLimitMap = Arc<[Mutex<HashMap<String, ClientState>>; NUM_SHARDS]>;

#[cfg(feature = "single-lock")]
pub type RateLimitMap = Arc<Mutex<HashMap<String, ClientState>>>;

#[derive(Clone)]
pub struct AppState {
    pub client: Client,
    pub rate_limit: RateLimitMap,
    pub all_upstreams: Vec<String>,
    pub healthy_upstreams: Arc<RwLock<Vec<String>>>,
    pub current_upstream: Arc<AtomicUsize>,
}

fn update_tokens(client_state: &mut ClientState) -> Result<(), axum::http::StatusCode> {
    let now = Instant::now();
    let elapsed = (now - client_state.last_updated).as_secs_f32();
    client_state.tokens = f32::min(client_state.tokens + elapsed * REFILL_RATE, CAPACITY);
    client_state.last_updated = now;
    if client_state.tokens >= 1.0 {
        client_state.tokens -= 1.0;
        Ok(())
    } else {
        Err(axum::http::StatusCode::TOO_MANY_REQUESTS)
    }
}

impl AppState {
    #[cfg(not(feature = "single-lock"))]
    pub fn new(upstreams: Vec<String>) -> Self {
        let shards = std::array::from_fn(|_| Mutex::new(HashMap::<String, ClientState>::new()));

        Self {
            client: Client::new(),
            rate_limit: Arc::new(shards),
            healthy_upstreams: Arc::new(RwLock::new(upstreams.clone())),
            all_upstreams: upstreams,
            current_upstream: Arc::new(AtomicUsize::new(0)),
        }
    }

    #[cfg(feature = "single-lock")]
    pub fn new(upstreams: Vec<String>) -> Self {
        Self {
            client: Client::new(),
            rate_limit: Arc::new(Mutex::new(HashMap::new())),
            healthy_upstreams: Arc::new(RwLock::new(upstreams.clone())),
            all_upstreams: upstreams,
            current_upstream: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Checks the rate limit for a given IP.
    /// Returns `Ok(())` if allowed, or `Err(StatusCode::TOO_MANY_REQUESTS)` if blocked.
    pub fn check_rate_limit(&self, ip: &str) -> Result<(), axum::http::StatusCode> {
        let now = Instant::now();

        #[cfg(not(feature = "single-lock"))]
        let mut guard = {
            let shard_index = hash_ip(ip) % NUM_SHARDS;
            self.rate_limit[shard_index].lock().unwrap()
        };

        #[cfg(feature = "single-lock")]
        let mut guard = self.rate_limit.lock().unwrap();

        let client_state = guard.entry(ip.to_string()).or_insert_with(|| ClientState {
            tokens: CAPACITY,
            last_updated: now,
        });
        update_tokens(client_state)
    }
}

pub fn hash_ip(ip: &str) -> usize {
    let mut hasher = DefaultHasher::new();
    ip.hash(&mut hasher);
    hasher.finish() as usize
}
