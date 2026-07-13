mod resp3;

pub use resp3::{SearchDocument, ValkeySearchResult};

use crate::{MODEL_NAME, vecembed::VecEmbed};
use anyhow::{Context, anyhow};
use fastembed::similarity::cosine_similarity;
use ferriskey::{Client, ClientBuilder, FromValue, Value};
use std::{env, sync::Arc};
use threatflux_vertex_rust_sdk::{GenerateContentRequest, GenerateContentResponse, VertexClient};

async fn get_valkey_client(host: &str, port: u16, pass: &str) -> ferriskey::Result<Client> {
    ClientBuilder::new()
        .host(host, port)
        .password(pass)
        .build()
        .await
}

fn vec_floats_to_bytes(floats: &[f32]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(floats.len() * 4);
    floats
        .iter()
        .for_each(|value| buf.extend_from_slice(&value.to_le_bytes()));
    buf
}

fn cosine_distance(lhs: &[f32], rhs: &[f32]) -> anyhow::Result<f32> {
    if lhs.len() != rhs.len() {
        return Err(anyhow!("Embedding dimensions do not match"));
    }

    let similarity = cosine_similarity(lhs, rhs);
    Ok((1.0 - similarity).max(0.0))
}

#[derive(Clone)]
pub struct Cache {
    valkey_client: Arc<Client>,
    vertexai_client: Arc<VertexClient>,
    embed: Arc<VecEmbed>,
}

impl Cache {
    pub async fn try_new(vertexai_client: Arc<VertexClient>) -> anyhow::Result<Self> {
        let host = env::var("VALKEY_HOST")?;
        let port = env::var("VALKEY_PORT")?;
        let port: u16 = port.parse()?;

        let pass = env::var("VALKEY_PASSWORD")?;

        tracing::info!("About to instantiate valkey client!");
        let valkey_client = get_valkey_client(host.as_str(), port, pass.as_str()).await?;
        tracing::info!("Instantiated valkey client!");
        let embed = VecEmbed::try_new().await?;

        let cache = Cache {
            valkey_client: Arc::new(valkey_client),
            vertexai_client,
            embed: Arc::new(embed),
        };
        cache.create_hnsw_index().await?;
        Ok(cache)
    }

    async fn call_vertexai(
        &self,
        req: GenerateContentRequest,
    ) -> anyhow::Result<GenerateContentResponse> {
        let tools_cnt = req.tools.as_ref().map(|tools| tools.len()).unwrap_or(0);
        let has_tools = tools_cnt > 0;

        if has_tools {
            self.vertexai_client
                .generate_with_functions(MODEL_NAME, &req)
                .await
                .map_err(Into::into)
        } else {
            self.vertexai_client
                .generate_content(MODEL_NAME, &req)
                .await
                .map_err(Into::into)
        }
    }

    pub async fn fetch_exact(
        &self,
        req: GenerateContentRequest,
    ) -> anyhow::Result<GenerateContentResponse> {
        let encoded_req = serde_json::to_string(&req)?;
        if let Some(encoded_resp) = self
            .valkey_client
            .get::<String>(encoded_req.clone())
            .await?
        {
            tracing::debug!("Cache hit: {} => {}", &encoded_req, &encoded_resp);
            return serde_json::from_str(&encoded_resp).map_err(Into::into);
        }

        let response = self.call_vertexai(req.clone()).await?;
        let encoded_resp = serde_json::to_string(&response)?;
        self.valkey_client
            .set(encoded_req.clone(), encoded_resp.clone())
            .await?;
        tracing::debug!("Cache set: {} => {}", encoded_req, encoded_resp);
        Ok(response)
    }

    pub async fn create_hnsw_index(&self) -> anyhow::Result<()> {
        let info: Result<ferriskey::Value, _> = self
            .valkey_client
            .cmd("FT.INFO")
            .arg("idx:cache")
            .execute()
            .await;

        if info.is_ok() {
            return Ok(());
        }

        self.valkey_client
            .cmd("FT.CREATE")
            .arg("idx:cache")
            .arg("ON")
            .arg("HASH")
            .arg("PREFIX")
            .arg("1")
            .arg("request:")
            .arg("SCHEMA")
            .arg("prompt_vec")
            .arg("VECTOR")
            .arg("HNSW")
            .arg("6")
            .arg("TYPE")
            .arg("FLOAT32")
            .arg("DIM")
            .arg("384")
            .arg("DISTANCE_METRIC")
            .arg("COSINE")
            .execute::<()>()
            .await
            .context("Failed to create HNSW index")?;

        Ok(())
    }

    async fn store_request(
        &self,
        id: &str,
        embedding: &[f32],
        request: &GenerateContentRequest,
        response: &GenerateContentResponse,
    ) -> anyhow::Result<()> {
        let bytes = vec_floats_to_bytes(embedding);
        let key = format!("request:{}", id);

        self.valkey_client
            .cmd("HSET")
            .arg(&key)
            .arg("prompt_vec")
            .arg(bytes)
            .execute::<i64>()
            .await?;

        let request_enc = serde_json::to_string(request)?;
        self.valkey_client
            .cmd("HSET")
            .arg(&key)
            .arg("encoded_request")
            .arg(request_enc)
            .execute::<i64>()
            .await?;

        let response_enc = serde_json::to_string(response)?;
        self.valkey_client
            .cmd("HSET")
            .arg(&key)
            .arg("encoded_response")
            .arg(response_enc)
            .execute::<i64>()
            .await?;

        Ok(())
    }

    async fn query_similar(
        &self,
        embedding: &[f32],
        max_distance: f32,
    ) -> anyhow::Result<Option<GenerateContentResponse>> {
        let bytes = vec_floats_to_bytes(embedding);

        let results: Value = self
            .valkey_client
            .cmd("FT.SEARCH")
            .arg("idx:cache")
            .arg("*=>[KNN 1 @prompt_vec $query_vec]")
            .arg("RETURN")
            .arg(3)
            .arg("prompt_vec")
            .arg("encoded_request")
            .arg("encoded_response")
            .arg("PARAMS")
            .arg(2)
            .arg("query_vec")
            .arg(bytes)
            .arg("DIALECT")
            .arg(2)
            .execute()
            .await?;

        tracing::debug!("Raw FT.SEARCH response: {:?}", results);
        let results = ValkeySearchResult::from_value(&results)?;
        tracing::debug!("Results from FT.SEARCH => {:?}", results);

        let Some(document) = results.documents.first() else {
            return Ok(None);
        };

        let Some(response) = document.response.clone() else {
            return Err(anyhow!("FT.SEARCH result did not include encoded_response"));
        };

        let Some(stored_embedding) = document.embedding.as_deref() else {
            return Ok(None);
        };

        let distance = cosine_distance(embedding, stored_embedding)?;
        tracing::debug!("Approximate cache distance: {}", distance);

        if distance > max_distance {
            return Ok(None);
        }

        Ok(Some(response))
    }

    pub async fn fetch_approximate_with_threshold(
        &self,
        req: GenerateContentRequest,
        prompt: &str,
        max_distance: f32,
    ) -> anyhow::Result<GenerateContentResponse> {
        let embedding = self.embed.embed_text(prompt).await?;
        if let Some(response) = self
            .query_similar(embedding.as_slice(), max_distance)
            .await?
        {
            return Ok(response);
        }

        let response = self.call_vertexai(req.clone()).await?;
        let id = prompt
            .chars()
            .filter(|ch| !ch.is_whitespace())
            .collect::<String>();
        self.store_request(&id, embedding.as_slice(), &req, &response)
            .await?;

        Ok(response)
    }

    pub async fn fetch_approximate(
        &self,
        req: GenerateContentRequest,
        prompt: &str,
    ) -> anyhow::Result<GenerateContentResponse> {
        self.fetch_approximate_with_threshold(req, prompt, f32::INFINITY)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{create_vertex_client, logs::init_logs};
    use std::sync::Arc;

    async fn reset_cache_state(cache: &Cache) {
        let _: () = cache.valkey_client.cmd("FLUSHDB").execute().await.unwrap();
    }

    #[tokio::test]
    async fn test_cache_fetch_exact() {
        init_logs();

        let client = Arc::new(create_vertex_client().await.unwrap());
        let cache = Cache::try_new(client.clone()).await.unwrap();
        reset_cache_state(&cache).await;
        let cache = Cache::try_new(client).await.unwrap();

        let req = GenerateContentRequest::new("Test cache!");
        let encoded_request = serde_json::to_string(&req).unwrap();

        let exists = cache
            .valkey_client
            .get::<Option<String>>(encoded_request.clone())
            .await
            .unwrap()
            .is_some();

        if exists {
            cache.valkey_client.del(&[encoded_request]).await.unwrap();
        }

        let ans1 = cache.fetch_exact(req.clone()).await.unwrap();
        let ans2 = cache.fetch_exact(req.clone()).await.unwrap();

        assert_eq!(ans1.full_response(), ans2.full_response());
    }

    #[tokio::test]
    async fn test_cache_fetch_approximate() {
        init_logs();

        let client = Arc::new(create_vertex_client().await.unwrap());
        let cache = Cache::try_new(client.clone()).await.unwrap();
        reset_cache_state(&cache).await;
        let cache = Cache::try_new(client).await.unwrap();

        let prompt = "Approx cache test!";
        let req = GenerateContentRequest::new(prompt);

        let first = cache.fetch_approximate(req.clone(), prompt).await.unwrap();
        let second = cache.fetch_approximate(req, prompt).await.unwrap();

        assert_eq!(first.full_response(), second.full_response());
    }

    #[tokio::test]
    async fn test_cache_fetch_approximate_thresholds() {
        init_logs();

        let client = Arc::new(create_vertex_client().await.unwrap());
        let cache = Cache::try_new(client.clone()).await.unwrap();
        reset_cache_state(&cache).await;
        let cache = Cache::try_new(client).await.unwrap();

        let anchor_prompt = "Approx threshold anchor: a small blue bird sings at sunrise";
        let anchor_request = GenerateContentRequest::new(anchor_prompt);
        let anchor_response = cache
            .fetch_approximate(anchor_request.clone(), anchor_prompt)
            .await
            .unwrap();

        let close_prompt = "Approx threshold close: a small blue bird sings in the morning";
        let close_request = GenerateContentRequest::new(close_prompt);
        let far_prompt = "Approx threshold far: the capital of France is Paris";
        let far_request = GenerateContentRequest::new(far_prompt);

        let anchor_embedding = cache.embed.embed_text(anchor_prompt).await.unwrap();
        let close_embedding = cache.embed.embed_text(close_prompt).await.unwrap();
        let far_embedding = cache.embed.embed_text(far_prompt).await.unwrap();

        let close_distance =
            cosine_distance(anchor_embedding.as_slice(), close_embedding.as_slice()).unwrap();
        let far_distance =
            cosine_distance(anchor_embedding.as_slice(), far_embedding.as_slice()).unwrap();
        let threshold = (close_distance + far_distance) / 2.0;

        assert!(close_distance < far_distance);
        assert!(close_distance < threshold);
        assert!(far_distance > threshold);

        let close_hit = cache
            .fetch_approximate_with_threshold(close_request, close_prompt, threshold)
            .await
            .unwrap();
        assert_eq!(close_hit.full_response(), anchor_response.full_response());

        let far_hit = cache
            .fetch_approximate_with_threshold(far_request, far_prompt, threshold)
            .await
            .unwrap();
        assert!(far_hit.function_calls().is_empty());
    }
}
