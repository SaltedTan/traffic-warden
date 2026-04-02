use axum::{Router, routing::get};
use proxy::build_app;
use proxy::state::AppState;
use std::net::SocketAddr;

/// Spins up a dummy upstream server and returns its URL
async fn spawn_mock_upstream() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    let app = Router::new().route("/", get(|| async { "Mock OK" }));

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    format!("http://127.0.0.1:{}", port)
}

/// Spins up a dummy upstream server that returns a specific custom response string
async fn spawn_custom_mock(response_body: &'static str) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    let app = Router::new().route(
        "/",
        axum::routing::get(move || async move { response_body }),
    );

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    format!("http://127.0.0.1:{}", port)
}

/// Spins up the Traffic Warden proxy pointed at the given upstreams, and returns its URL
async fn spawn_proxy(upstreams: Vec<String>) -> String {
    let state = AppState::new(upstreams);
    let app = build_app(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });

    format!("http://127.0.0.1:{}", port)
}

#[tokio::test]
async fn test_basic_proxy_routing() {
    let upstream_url = spawn_mock_upstream().await;
    let proxy_url = spawn_proxy(vec![upstream_url]).await;

    let client = reqwest::Client::new();
    let response = client.get(&proxy_url).send().await.unwrap();

    assert_eq!(response.status(), reqwest::StatusCode::OK);

    let body = response.text().await.unwrap();
    assert_eq!(body, "Mock OK");
}

#[tokio::test]
async fn test_rate_limiter_prevents_bursts() {
    let upstream_url = spawn_mock_upstream().await;
    let proxy_url = spawn_proxy(vec![upstream_url]).await;

    let client = reqwest::Client::new();
    let mut tasks = Vec::new();

    for _ in 0..6 {
        let client_clone = client.clone();
        let url_clone = proxy_url.clone();

        tasks.push(tokio::spawn(async move {
            client_clone.get(&url_clone).send().await.unwrap()
        }));
    }

    let results = futures::future::join_all(tasks).await;

    let mut success_count = 0;
    let mut rate_limited_count = 0;

    for result in results {
        let response = result.unwrap();
        if response.status() == reqwest::StatusCode::OK {
            success_count += 1;
        } else if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            rate_limited_count += 1;
        }
    }

    assert_eq!(success_count, 5);
    assert_eq!(rate_limited_count, 1);
}

#[tokio::test]
async fn test_round_robin_distribution() {
    let upstream_a = spawn_custom_mock("Server A").await;
    let upstream_b = spawn_custom_mock("Server B").await;
    let upstream_c = spawn_custom_mock("Server C").await;

    let proxy_url = spawn_proxy(vec![upstream_a, upstream_b, upstream_c]).await;
    let client = reqwest::Client::new();

    let mut responses = Vec::new();
    for _ in 0..3 {
        let res = client.get(&proxy_url).send().await.unwrap();
        responses.push(res.text().await.unwrap());
    }

    assert!(responses.contains(&"Server A".to_string()));
    assert!(responses.contains(&"Server B".to_string()));
    assert!(responses.contains(&"Server C".to_string()));
}
