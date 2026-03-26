use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use axum::{Router, body::to_bytes, response::IntoResponse};
use reqwest::Client;

const PROXY_ADDR: &str = "127.0.0.1:3000";
const MAX_BODY_SIZE: usize = 5 * 1024 * 1024;
const CAPACITY: f32 = 5.0;
const REFILL_RATE: f32 = 5.0 / 60.0;
const NUM_SHARDS: usize = 64;

struct ClientState {
    tokens: f32,
    last_updated: Instant,
}

type RateLimitMap = Arc<[Mutex<HashMap<String, ClientState>>; NUM_SHARDS]>;

#[derive(Clone)]
struct AppState {
    client: reqwest::Client,
    rate_limit: RateLimitMap,
    all_upstreams: Vec<String>,
    healthy_upstreams: Arc<RwLock<Vec<String>>>,
    current_upstream: Arc<AtomicUsize>,
}

impl AppState {
    /// Checks the rate limit for a given IP.
    /// Returns `Ok(())` if allowed, or `Err(StatusCode::TOO_MANY_REQUESTS)` if blocked.
    fn check_rate_limit(&self, ip: &str) -> Result<(), axum::http::StatusCode> {
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

#[tokio::main]
async fn main() {
    let client = Client::new();
    let shards = std::array::from_fn(|_| Mutex::new(HashMap::<String, ClientState>::new()));
    let rate_limit = Arc::new(shards);

    let upstreams = vec![
        "http://127.0.0.1:8080".to_string(),
        "http://127.0.0.1:8081".to_string(),
        "http://127.0.0.1:8082".to_string(),
    ];

    let state = AppState {
        client,
        rate_limit,
        healthy_upstreams: Arc::new(RwLock::new(upstreams.clone())),
        all_upstreams: upstreams,
        current_upstream: Arc::new(AtomicUsize::new(0)),
    };

    // Clone the Arc so the background task can own a reference to the Mutex
    let garbage_collector_state = state.rate_limit.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
            let now = Instant::now();

            for shard in garbage_collector_state.iter() {
                let mut map = shard.lock().unwrap();

                map.retain(|_ip, state| {
                    (now - state.last_updated).as_secs_f32() <= CAPACITY / REFILL_RATE
                });
                // The lock for this specific shard drops here, before moving to the next one
            }
        }
    });

    let health_checker_state = state.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(10)).await;

            let mut new_healthy_list = Vec::new();

            for upstream in &health_checker_state.all_upstreams {
                let ping_url = format!("{}/", upstream);

                let result = health_checker_state
                    .client
                    .get(&ping_url)
                    .timeout(Duration::from_secs(2))
                    .send()
                    .await;

                if let Ok(res) = result {
                    if res.status().is_success() {
                        new_healthy_list.push(upstream.clone());
                    }
                }
            }

            let mut current_healthy = health_checker_state.healthy_upstreams.write().unwrap();
            *current_healthy = new_healthy_list;
        }
    });

    let app = Router::new().fallback(proxy_handler).with_state(state);
    let listener = tokio::net::TcpListener::bind(PROXY_ADDR).await.unwrap();

    println!("Traffic Warden Proxy Listening on http://{}", PROXY_ADDR);

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await
    .unwrap();
}

async fn proxy_handler(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<std::net::SocketAddr>,
    req: axum::extract::Request,
) -> Result<axum::response::Response, axum::http::StatusCode> {
    let ip = addr.ip().to_string();

    state.check_rate_limit(&ip)?;

    let selected_upstream = {
        let healthy = state.healthy_upstreams.read().unwrap();

        if healthy.is_empty() {
            return Err(axum::http::StatusCode::BAD_GATEWAY);
        }

        let previous_count = state.current_upstream.fetch_add(1, Ordering::Relaxed);
        let target_index = previous_count % healthy.len();

        healthy[target_index].clone()
    };

    let (parts, body) = req.into_parts();
    let mut headers = parts.headers;
    headers.remove(axum::http::header::HOST);

    let path_query = parts
        .uri
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or(parts.uri.path());

    let url = format!("{}{}", selected_upstream, path_query);

    let body_bytes = to_bytes(body, MAX_BODY_SIZE)
        .await
        .map_err(|_| axum::http::StatusCode::PAYLOAD_TOO_LARGE)?;

    let response = state
        .client
        .request(parts.method, url)
        .headers(headers)
        .body(body_bytes)
        .send()
        .await
        .map_err(|_| axum::http::StatusCode::BAD_GATEWAY)?;

    let status_code = response.status();
    let headers = response.headers().clone();
    let bytes = response
        .bytes()
        .await
        .map_err(|_| axum::http::StatusCode::BAD_GATEWAY)?;

    Ok((status_code, headers, bytes).into_response())
}

fn hash_ip(ip: &str) -> usize {
    let mut hasher = DefaultHasher::new();
    ip.hash(&mut hasher);
    hasher.finish() as usize
}
