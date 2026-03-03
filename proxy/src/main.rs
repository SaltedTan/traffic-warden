use std::usize;

use axum::{Router, body::to_bytes, response::IntoResponse};
use reqwest::Client;

const PROXY_ADDR: &str = "127.0.0.1:3000";
const UPSTREAM_URL: &str = "http://127.0.0.1:8080";
const MAX_BODY_SIZE: usize = 5 * 1024 * 1024;

#[tokio::main]
async fn main() {
    let client = Client::new();
    let app = Router::new().fallback(proxy_handler).with_state(client);

    let listener = tokio::net::TcpListener::bind(PROXY_ADDR).await.unwrap();

    println!("Traffic Warden Proxy Listening on http://{}", PROXY_ADDR);

    axum::serve(listener, app).await.unwrap();
}

async fn proxy_handler(
    axum::extract::State(client): axum::extract::State<Client>,
    req: axum::extract::Request,
) -> Result<impl IntoResponse, axum::http::StatusCode> {
    let (parts, body) = req.into_parts();
    let path_query = parts
        .uri
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or(parts.uri.path());

    let url = format!("{}{}", UPSTREAM_URL, path_query);

    let body_bytes = to_bytes(body, MAX_BODY_SIZE)
        .await
        .map_err(|_| axum::http::StatusCode::PAYLOAD_TOO_LARGE)?;

    let response = client
        .request(parts.method, url)
        .headers(parts.headers)
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

    Ok((status_code, headers, bytes))
}
