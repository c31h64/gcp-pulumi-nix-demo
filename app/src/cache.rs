use crate::MODEL_NAME;
use anyhow::{Context, anyhow};
use ferriskey::{Client, ClientBuilder};
use std::{env, sync::Arc};
use threatflux_vertex_rust_sdk::{GenerateContentRequest, GenerateContentResponse, VertexClient};

async fn get_valkey_client(host: &str, port: u16) -> ferriskey::Result<Client> {
    return ClientBuilder::new().host(host, port).build().await;
}

#[derive(Clone)]
pub struct Cache {
    valkey_client: Arc<Client>,
    vertexai_client: Arc<VertexClient>,
}

impl Cache {
    pub async fn try_new(vertexai_client: Arc<VertexClient>) -> anyhow::Result<Self> {
        let host = env::var("VALKEY_HOST").unwrap_or_else(|_| "localhost".to_string());
        let port = env::var("VALKEY_PORT").unwrap_or_else(|_| "6379".to_string());
        let port: u16 = port.parse()?;

        let valkey_client = get_valkey_client(host.as_str(), port).await?;

        return Ok(Cache {
            valkey_client: Arc::new(valkey_client),
            vertexai_client: vertexai_client,
        });
    }

    pub async fn create_hnsw_index(&self) -> anyhow::Result<()> {
        // FT.CREATE idx:cache ON HASH PREFIX 1 request: SCHEMA prompt_vec VECTOR HNSW 16 TYPE FLOAT32 DIM 1536 DISTANCE_METRIC COSINE
        self.valkey_client
            .cmd("FT.CREATE")
            .arg("idx:cache")
            .arg("ON HASH PREFIX 1 request:")
            .arg("SCHEMA prompt_vec")
            .arg("VECTOR HNSW 16 TYPE FLOAT32 DIM 1536")
            .arg("DISTANCE_METRIC COSINE")
            .execute()
            .await
            .context("")
    }

    /// Fetch with exact hash matching.
    // TODO: Use bincode here for more efficient packing instead of just textual JSON!
    pub async fn fetch_exact(
        &self,
        req: GenerateContentRequest,
    ) -> anyhow::Result<GenerateContentResponse> {
        let encoded_req: String = serde_json::to_string(&req)?;
        let cache_res: Option<String> = self.valkey_client.get(encoded_req.clone()).await?;
        match cache_res {
            Some(encoded_resp) => {
                tracing::debug!("Cache hit: {} => {}", &encoded_req, &encoded_resp);
                return Ok(serde_json::from_str(&encoded_resp)?);
            }
            None => {
                let tools_cnt = req.tools.as_ref().map(|tools| tools.len()).unwrap_or(0);
                let has_tools = tools_cnt > 0;

                let response = if has_tools {
                    self.vertexai_client
                        .generate_with_functions(MODEL_NAME, &req)
                        .await?
                } else {
                    self.vertexai_client
                        .generate_content(MODEL_NAME, &req)
                        .await?
                };

                let encoded_resp = serde_json::to_string(&response)?;
                self.valkey_client
                    .set(encoded_req.clone(), encoded_resp.clone())
                    .await?;
                tracing::debug!("Cache set: {} => {}", encoded_req, encoded_resp);
                return Ok(response);
            }
        }
    }

    /// Fetch with appropximate nearest neighbours.
    pub async fn fetch_approximate(
        _req: GenerateContentRequest,
    ) -> anyhow::Result<GenerateContentResponse> {
        Err(anyhow!("TODO: Implement me"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{create_vertex_client, logs::init_logs};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_cache_fetch_exact() {
        init_logs();

        let client = Arc::new(create_vertex_client().await.unwrap());
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
}
