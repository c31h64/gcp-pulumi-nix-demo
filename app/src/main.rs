pub mod adjudicate;
use crate::adjudicate::*;

use anyhow::anyhow;
use axum::{Router, http::StatusCode, routing::get};
use std::env;

use threatflux_vertex_rust_sdk::{GenerateContentRequest, VertexClient, config::Config};

const MODEL_NAME: &'static str = "gemini-3.5-flash";

async fn create_vertex_client() -> anyhow::Result<VertexClient> {
    let config = Config {
        project_id: env::var("GOOGLE_CLOUD_PROJECT")?,
        region: "global".to_string(), //env::var("GOOGLE_CLOUD_LOCATION")?,
        ..Config::default()
    };

    Ok(VertexClient::new(config).await?)
}

// TODO: This always returns OK so as not to waste tokens but the real world implementation would do the same as ready_check
async fn health_check() -> StatusCode {
    StatusCode::OK
}

async fn ready_check() -> StatusCode {
    let client = create_vertex_client().await;
    let request = GenerateContentRequest::new("Ping!");

    let response = client.map(|c| async move { c.generate_content(MODEL_NAME, &request).await });

    match response {
        Ok(_) => StatusCode::OK,
        Err(e) => {
            eprintln!("{}", e);
            StatusCode::SERVICE_UNAVAILABLE
        }
    }
}

async fn generate_quote() -> anyhow::Result<String> {
    println!("Called generate_quote()");

    let client = create_vertex_client().await?;

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

fn create_axum_app() -> Router {
    let app = Router::new();

    let app = app.route(
        "/",
        get(|| async { "Hello from Cloud Run! Built with Nix, Pulumi, GCP and Love!" }),
    );
    let app = app.route("/ready", get(ready_check));
    let app = app.route("/health", get(health_check));
    let app = app.route("/quote", get(generate_handler));

    return app;
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let addr = format!("0.0.0.0:{}", port);

    let listener = tokio::net::TcpListener::bind(addr).await?;

    let app = create_axum_app();

    println!("Serving on port: {}", port);

    axum::serve(listener, app).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_quote_route() {
        let app = create_axum_app();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/quote")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}
