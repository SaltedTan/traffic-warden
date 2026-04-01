use proxy::build_app;
use proxy::state::AppState;
use proxy::workers::spawn_background_workers;

const PROXY_ADDR: &str = "127.0.0.1:3000";

#[tokio::main]
async fn main() {
    let upstreams = vec![
        "http://127.0.0.1:8080".to_string(),
        "http://127.0.0.1:8081".to_string(),
        "http://127.0.0.1:8082".to_string(),
    ];

    let state = AppState::new(upstreams);

    // We clone the state so the workers own a copy of the Arcs,
    // leaving the original `state` available for the web framework.
    spawn_background_workers(state.clone());

    let app = build_app(state);
    let listener = tokio::net::TcpListener::bind(PROXY_ADDR).await.unwrap();

    println!("Traffic Warden Proxy Listening on http://{}", PROXY_ADDR);

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await
    .unwrap();
}
