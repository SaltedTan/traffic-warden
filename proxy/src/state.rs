use reqwest::Client;
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

pub const CAPACITY: f32 = 5.0;
pub const REFILL_RATE: f32 = 5.0 / 60.0;
pub const NUM_SHARDS: usize = 64;

pub struct ClientState {
    pub tokens: f32,
    pub last_updated: Instant,
}

pub type RateLimitMap = Arc<[Mutex<HashMap<String, ClientState>>; NUM_SHARDS]>;

#[derive(Clone)]
pub struct AppState {
    pub client: Client,
    pub rate_limit: RateLimitMap,
    pub all_upstreams: Vec<String>,
    pub healthy_upstreams: Arc<RwLock<Vec<String>>>,
    pub current_upstream: Arc<AtomicUsize>,
}

impl AppState {
    /// Construct a new AppState with the given list of upstream servers.
    pub fn new(upstreams: Vec<String>) -> Self {
        let client = Client::new();
        let shards = std::array::from_fn(|_| Mutex::new(HashMap::<String, ClientState>::new()));
        let rate_limit = Arc::new(shards);

        Self {
            client,
            rate_limit,
            healthy_upstreams: Arc::new(RwLock::new(upstreams.clone())),
            all_upstreams: upstreams,
            current_upstream: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Checks the rate limit for a given IP.
    /// Returns `Ok(())` if allowed, or `Err(StatusCode::TOO_MANY_REQUESTS)` if blocked.
    pub fn check_rate_limit(&self, ip: &str) -> Result<(), axum::http::StatusCode> {
        let shard_index = hash_ip(ip) % NUM_SHARDS;
        let now = Instant::now();

        let mut client_states = self.rate_limit[shard_index].lock().unwrap();

        let client_state = client_states
            .entry(ip.to_string())
            .or_insert_with(|| ClientState {
                tokens: CAPACITY,
                last_updated: now,
            });

        let time_elapsed = (now - client_state.last_updated).as_secs_f32();
        client_state.tokens += time_elapsed * REFILL_RATE;
        client_state.tokens = f32::min(client_state.tokens, CAPACITY);

        if client_state.tokens >= 1.0 {
            client_state.tokens -= 1.0;
            client_state.last_updated = now;
            Ok(())
        } else {
            client_state.last_updated = now;
            Err(axum::http::StatusCode::TOO_MANY_REQUESTS)
        }
    }
}

pub fn hash_ip(ip: &str) -> usize {
    let mut hasher = DefaultHasher::new();
    ip.hash(&mut hasher);
    hasher.finish() as usize
}
