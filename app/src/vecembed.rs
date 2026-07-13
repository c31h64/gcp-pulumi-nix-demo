use anyhow::anyhow;
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use std::sync::{Arc, Mutex};
use std::{env, path::Path, path::PathBuf};

#[derive(Clone)]
pub struct VecEmbed {
    model: Arc<Mutex<TextEmbedding>>,
}

impl VecEmbed {
    pub async fn try_new() -> anyhow::Result<Self> {
        let path =
            env::var("FASTEMBED_CACHE_DIR").unwrap_or_else(|_| "/var/cache/fastembed".to_string());
        let path = PathBuf::from(path);

        Self::ensure_model_cache(&path)?;
        tracing::info!("Initializing the fastembed models from {}.", path.display());

        let model = TextEmbedding::try_new(
            InitOptions::new(EmbeddingModel::AllMiniLML6V2Q)
                .with_show_download_progress(true)
                .with_cache_dir(path.clone()),
        )?;
        let model = Arc::new(Mutex::new(model));

        Ok(VecEmbed { model })
    }

    fn ensure_model_cache(cache_dir: &Path) -> anyhow::Result<()> {
        let model_root = cache_dir.join("models--Xenova--all-MiniLM-L6-v2");
        let snapshot_root = model_root.join("snapshots");
        let onnx_model = snapshot_root
            .read_dir()
            .map_err(|err| {
                anyhow!(
                    "Failed to read fastembed model cache directory at {}: {err}",
                    cache_dir.display()
                )
            })?
            .flatten()
            .map(|entry| entry.path())
            .find(|path| path.join("onnx").join("model_quantized.onnx").is_file());

        if let Some(path) = onnx_model {
            let onnx_path = path.join("onnx").join("model_quantized.onnx");
            tracing::info!("Fastembed model cache found at {}.", onnx_path.display());
            return Ok(());
        }

        Err(anyhow!(
            "Fastembed model cache is missing at {}. Expected {}. The container image must include the cached model.",
            cache_dir.display(),
            model_root
                .join("snapshots")
                .join("<snapshot-id>")
                .join("onnx")
                .join("model_quantized.onnx")
                .display()
        ))
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
