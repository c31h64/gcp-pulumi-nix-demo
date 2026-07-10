use anyhow::anyhow;
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use std::sync::{Arc, Mutex};
use std::{env, path::PathBuf};

#[derive(Clone)]
pub struct VecEmbed {
    model: Arc<Mutex<TextEmbedding>>,
}

impl VecEmbed {
    pub async fn try_new() -> anyhow::Result<Self> {
        let path = env::var("FASTEMBED_CACHE_DIR")?;
        let path = PathBuf::from(path);

        tracing::info!("Initializing the fastembed models.");

        let model = TextEmbedding::try_new(
            InitOptions::new(EmbeddingModel::AllMiniLML6V2Q)
                .with_show_download_progress(true)
                .with_cache_dir(path),
        )?;
        let model = Arc::new(Mutex::new(model));

        Ok(VecEmbed { model })
    }

    pub async fn embed_text(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        let mut model = self.model.lock().map_err(|_| anyhow!("Mutex poisoning!"))?;
        let emb = model.embed([text], None)?;

        emb.first()
            .cloned()
            .ok_or_else(|| anyhow!("Failed to generate even a single embedding!"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init_logs;

    #[tokio::test]
    async fn test_vecembed_text_embed() {
        init_logs();

        let vecembed = VecEmbed::try_new().await.unwrap();

        let embs = vecembed.embed_text("Hello world").await.unwrap();

        tracing::info!("Embs: {:?}", embs);
    }
}
