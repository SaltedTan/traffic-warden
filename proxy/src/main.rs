use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::{Router, body::to_bytes, response::IntoResponse};
use reqwest::Client;

const PROXY_ADDR: &str = "127.0.0.1:3000";
const UPSTREAM_URL: &str = "http://127.0.0.1:8080";
const MAX_BODY_SIZE: usize = 5 * 1024 * 1024;
const MAX_REQUESTS: u32 = 5;

type RateLimitMap = Arc<Mutex<HashMap<String, u32>>>;

#[derive(Clone)]
struct AppState {
    client: reqwest::Client,
    rate_limit: RateLimitMap,
}

#[tokio::main]
async fn main() {
    let client = Client::new();
    let rate_limit = Arc::new(Mutex::new(HashMap::new()));

    let state = AppState { client, rate_limit };

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

    {
        let mut counts = state.rate_limit.lock().unwrap();
        let count = counts.entry(ip).or_insert(0);
        *count += 1;
        if *count > MAX_REQUESTS {
            return Err(axum::http::StatusCode::TOO_MANY_REQUESTS);
        }
    }

    let (parts, body) = req.into_parts();
    let mut headers = parts.headers;
    headers.remove(axum::http::header::HOST);

    let path_query = parts
        .uri
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or(parts.uri.path());

    let url = format!("{}{}", UPSTREAM_URL, path_query);

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
