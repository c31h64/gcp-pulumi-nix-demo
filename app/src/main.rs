use anyhow::anyhow;
use axum::{Router, http::StatusCode, routing::get};
use std::env;

use threatflux_vertex_rust_sdk::{GenerateContentRequest, VertexClient, config::Config};

async fn health_check() -> StatusCode {
    StatusCode::OK
}

async fn ready_check() -> StatusCode {
    match generate_quote().await {
        Ok(_) => StatusCode::OK,
        Err(e) => {
            eprintln!("{}", e);
            StatusCode::SERVICE_UNAVAILABLE
        }
    }
}

async fn generate_quote() -> anyhow::Result<String> {
    println!("Called generate_quote()");

    let config = Config {
        project_id: env::var("GOOGLE_CLOUD_PROJECT")?,
        region: "global".to_string(), //env::var("GOOGLE_CLOUD_LOCATION")?,
        ..Config::default()
    };
    let client = VertexClient::new(config).await?;

    let request = GenerateContentRequest::new("Why is the sky blue?");
    let response = client
        .generate_content("gemini-3.5-flash", &request)
        .await?;

    if let Some(text) = response.text() {
        return Ok(text);
    }

    Err(anyhow!(
        "Failed to connect to gemini via vertex AI to get output"
    ))
}

async fn generate_handler() -> String {
    match generate_quote().await {
        Ok(t) => dbg!(t),
        Err(t) => dbg!(t.to_string()),
    }
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
    let app = app.route("/quote", get(generate_handler));

    let listener = tokio::net::TcpListener::bind(addr).await?;

    // Await the server here to keep it running.
    // The '?' operator will propagate any startup/runtime errors.
    println!("Serving on port: {}", port);

    axum::serve(listener, app).await?;

    Ok(())
}
