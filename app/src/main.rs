use axum::{Router, http::StatusCode, routing::get};
use std::env;

async fn health_check() -> StatusCode {
    StatusCode::OK
}

async fn ready_check() -> StatusCode {
    StatusCode::OK
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let addr = format!("0.0.0.0:{}", port);

    let app = Router::new();

    let app = app.route(
        "/",
        get(|| async { "Hello from Cloud Run! Built with Nix, Pulumi, GCP and Love!" }),
    );
    let app = app.route("/ready", get(ready_check));
    let app = app.route("/health", get(health_check));

    let listener = tokio::net::TcpListener::bind(addr).await?;

    // Await the server here to keep it running.
    // The '?' operator will propagate any startup/runtime errors.
    println!("Serving on port: {}", port);

    axum::serve(listener, app).await?;

    Ok(())
}
