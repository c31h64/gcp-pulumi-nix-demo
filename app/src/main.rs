pub mod adjudicate;
pub mod cache;
pub mod logs;
pub mod vecembed;

use adjudicate::*;
use cache::*;
use vecembed::*;

use anyhow::anyhow;
use axum::{
    Router, debug_handler,
    extract::{Json, State},
    http::StatusCode,
    routing::{get, post},
};
use axum_anyhow::ApiResult;
use std::env;
use std::sync::Arc;

use logs::init_logs;

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

#[derive(Clone)]
struct AppState {
    client: Arc<VertexClient>,
    cache: Cache,
    embed: VecEmbed,
}

impl AppState {
    async fn try_new() -> anyhow::Result<AppState> {
        let client = Arc::new(create_vertex_client().await?);
        let cache = Cache::try_new(client.clone()).await?;
        let embed = VecEmbed::try_new().await?;
        Ok(AppState {
            client: client,
            cache: cache,
            embed: embed,
        })
    }
}

// TODO: This always returns OK so as not to waste tokens but the real world implementation would do the same as ready_check
async fn health_check() -> StatusCode {
    StatusCode::OK
}

async fn ready_check(State(state): State<AppState>) -> StatusCode {
    let client = state.client;
    let request = GenerateContentRequest::new("Ping!");

    let response = client.generate_content(MODEL_NAME, &request).await;

    match response {
        Ok(_) => StatusCode::OK,
        Err(e) => {
            tracing::error!("{}", e);
            StatusCode::SERVICE_UNAVAILABLE
        }
    }
}

#[debug_handler]
async fn adjudicate_handler(
    State(state): State<AppState>,
    Json(request): Json<AdjudicateRequest>,
) -> ApiResult<Json<AdjudicateOutcome>> {
    let outcome = adjudicate(state.cache, request).await.map_err(|e| {
        tracing::error!(error = ?e, "Adjudication failed");
        axum_anyhow::ApiError::from(e)
    })?;

    Ok(Json(outcome))
}

async fn generate_quote(state: AppState) -> anyhow::Result<String> {
    tracing::info!("Called generate_quote()");

    let cache = state.cache;

    let request = GenerateContentRequest::new("Why is the sky blue?");
    let response = cache.fetch_exact(request).await?;

    if let Some(text) = response.text() {
        return Ok(text);
    }

    Err(anyhow!(
        "Failed to connect to gemini via vertex AI to get output"
    ))
}

async fn generate_handler(State(state): State<AppState>) -> String {
    match generate_quote(state).await {
        Ok(t) => dbg!(t),
        Err(t) => dbg!(t.to_string()),
    }
}

fn create_axum_app(state: AppState) -> Router {
    let app = Router::new();

    let app = app.route(
        "/",
        get(|| async { "Hello from Cloud Run! Built with Nix, Pulumi, GCP and Love!" }),
    );
    let app = app.route("/ready", get(ready_check));
    let app = app.route("/health", get(health_check));
    let app = app.route("/quote", get(generate_handler));
    let app = app.route("/adjudicate", post(adjudicate_handler));
    let app = app.with_state(state);

    return app;
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_logs();

    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let addr = format!("0.0.0.0:{}", port);

    let listener = tokio::net::TcpListener::bind(addr).await?;

    let state = AppState::try_new().await?;

    let app = create_axum_app(state);

    tracing::info!("Serving on port: {}", port);

    axum::serve(listener, app).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_quote_route() {
        let state = AppState::try_new().await.unwrap();
        let app = create_axum_app(state);
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

    #[tokio::test]
    async fn test_adjudicate_endpoint() {
        let state = AppState::try_new().await.unwrap();
        let app = create_axum_app(state);

        let body = serde_json::json!({
            "problem_text": "Is this working?",
            "side_a_text": "Yes",
            "side_b_text": "No"
        });

        let request = axum::http::Request::builder()
            .uri("/adjudicate")
            .method("POST")
            .header("Content-Type", "application/json")
            .body(body.to_string())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        let status = response.status();

        println!("Status: {:?}", status);

        let (_parts, body) = response.into_parts();

        let body_bytes = to_bytes(body, 1024 * 64).await.unwrap();
        let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();

        println!("Body: {}", body_str);

        assert_eq!(status, StatusCode::OK);
    }
}
