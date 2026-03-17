use axum::{Router, routing::get};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    let ports = [8080, 8081, 8082];

    for port in ports {
        tokio::spawn(async move {
            let app =
                Router::new().route(
                    "/",
                    get(move || async move {
                        format!("Hello from Upstream Server on port {}!\n", port)
                    }),
                );

            let listener = TcpListener::bind(format!("127.0.0.1:{}", port))
                .await
                .unwrap();

            println!("Mock server listening on port {}", port);
            axum::serve(listener, app).await.unwrap();
        });
    }

    // Keep the main thread alive indefinitely
    std::future::pending::<()>().await;
}
