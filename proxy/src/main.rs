use std::usize;

use axum::{Router, body::to_bytes, response::IntoResponse};
use reqwest::Client;

const PROXY_ADDR: &str = "127.0.0.1:3000";
const UPSTREAM_URL: &str = "http://127.0.0.1:8080";

#[tokio::main]
async fn main() {
    let app = Router::new().fallback(proxy_handler);

    let listener = tokio::net::TcpListener::bind(PROXY_ADDR).await.unwrap();

    println!("Traffic Warden Proxy Listening on http://{}", PROXY_ADDR);

    axum::serve(listener, app).await.unwrap();
}

async fn proxy_handler(req: axum::extract::Request) -> impl IntoResponse {
    let (parts, body) = req.into_parts();

    let url = format!("{}{}", UPSTREAM_URL, parts.uri.path());
    let body_bytes = to_bytes(body, usize::MAX).await.unwrap();

    let client = Client::new();

    let response = client
        .request(parts.method, url)
        .headers(parts.headers)
        .body(body_bytes)
        .send()
        .await
        .unwrap();

    let status_code = response.status();
    let headers = response.headers().clone();
    let bytes = response.bytes().await.unwrap();

    (status_code, headers, bytes)
}
