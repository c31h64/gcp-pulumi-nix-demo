use axum::{Router, routing::get};
use std::env;

use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let addr = format!("0.0.0.0:{}", port);

    let app = Router::new().route(
        "/",
        get(|| async { "Hello from Cloud Run! Built with Nix, Pulumi, GCP and Love!" }),
    );

    let listener = tokio::net::TcpListener::bind(addr).await?;

    // Await the server here to keep it running.
    // The '?' operator will propagate any startup/runtime errors.
    axum::serve(listener, app).await?;

    Ok(())
}
