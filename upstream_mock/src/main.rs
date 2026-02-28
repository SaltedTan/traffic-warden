use axum::{routing::get, Router};

#[tokio::main]
async fn main() {
    let app = Router::new().route("/", get(|| async { "Hello from Upstream!"}));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080")   
        .await
        .unwrap();

    println!("Upstream mock listening on http://127.0.0.1:8080");

    axum::serve(listener, app).await.unwrap();
}
