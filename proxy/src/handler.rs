use crate::state::AppState;
use axum::{body::Body, response::IntoResponse};
use std::sync::atomic::Ordering;

pub async fn proxy_handler(
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

    let reqwest_body = reqwest::Body::wrap_stream(body.into_data_stream());

    let response = state
        .client
        .request(parts.method, url)
        .headers(headers)
        .body(reqwest_body)
        .send()
        .await
        .map_err(|_| axum::http::StatusCode::BAD_GATEWAY)?;

    let status_code = response.status();
    let headers = response.headers().clone();
    let response_stream = response.bytes_stream();
    let axum_body = Body::from_stream(response_stream);

    Ok((status_code, headers, axum_body).into_response())
}
