use anyhow::{Context, Result};
use async_trait::async_trait;
use candle_core::{Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config};
use std::path::Path;
use tokenizers::Tokenizer;

use crate::embeddings::Embedder;

pub struct BGEEmbedder {
    model: BertModel,
    tokenizer: Tokenizer,
    device: Device,
}

impl BGEEmbedder {
    pub fn new() -> Result<Self> {
        // Model path - adjust as needed
        let model_path = Path::new("./models/bge-small-en-v1.5");

        // Setup device: prefer Metal on macOS if available and SURR_USE_METAL != "false"
        #[cfg(target_os = "macos")]
        let device = {
            let use_metal = std::env::var("SURR_USE_METAL")
                .ok()
                .map(|v| v != "false" && v != "0")
                .unwrap_or(true);
            if use_metal {
                match Device::new_metal(0) {
                    Ok(d) => d,
                    Err(_e) => Device::Cpu,
                }
            } else {
                Device::Cpu
            }
        };
        #[cfg(not(target_os = "macos"))]
        let device = Device::Cpu;

        // Load tokenizer
        let tokenizer_path = model_path.join("tokenizer.json");
        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| anyhow::anyhow!("Failed to load tokenizer: {}", e))?;

        // Load model config
        let config_path = model_path.join("config.json");
        let config_str =
            std::fs::read_to_string(&config_path).context("Failed to read config.json")?;
        let config: Config =
            serde_json::from_str(&config_str).context("Failed to parse config.json")?;

        // Load model weights
        let weights_path = model_path.join("model.safetensors");
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&[weights_path], candle_core::DType::F32, &device)?
        };

        let model = BertModel::load(vb, &config)?;

        Ok(Self {
            model,
            tokenizer,
            device,
        })
    }

    fn mean_pooling(&self, embeddings: &Tensor, attention_mask: &Tensor) -> Result<Vec<f32>> {
        // Expand attention mask from [batch, seq] to [batch, seq, hidden]
        let mask_expanded = attention_mask.unsqueeze(2)?;

        // Apply mask to embeddings
        let masked = embeddings.broadcast_mul(&mask_expanded)?;

        // Sum across sequence dimension
        let summed = masked.sum(1)?;

        // Sum the mask values for normalization
        let mask_sum = mask_expanded.sum(1)?;

        // Divide to get mean (avoiding division by zero)
        let mean = summed.broadcast_div(&mask_sum)?;

        // Normalize to unit vector (BGE models expect this)
        let norm = mean.sqr()?.sum_keepdim(1)?.sqrt()?;
        let normalized = mean.broadcast_div(&norm)?;

        // Convert to Vec<f32>
        let result = normalized.squeeze(0)?.to_vec1::<f32>()?;
        Ok(result)
    }
}

#[async_trait]
impl Embedder for BGEEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        // Add instruction prefix for BGE model
        let instructed_text = format!(
            "Represent this sentence for searching relevant passages: {}",
            text
        );

        // Tokenize
        let encoding = self
            .tokenizer
            .encode(instructed_text.as_str(), true)
            .map_err(|e| anyhow::anyhow!("Tokenization failed: {}", e))?;

        let token_ids = encoding.get_ids();
        let attention_mask = encoding.get_attention_mask();
        let type_ids = vec![0u32; token_ids.len()]; // All zeros for single sentence

        // Convert to tensors
        let input_ids = Tensor::new(token_ids, &self.device)?.unsqueeze(0)?;
        let token_type_ids = Tensor::new(type_ids.as_slice(), &self.device)?.unsqueeze(0)?;
        let attention_tensor = Tensor::new(
            attention_mask
                .iter()
                .map(|&x| x as f32)
                .collect::<Vec<_>>()
                .as_slice(),
            &self.device,
        )?
        .unsqueeze(0)?;

        // Run model - BERT forward takes (input_ids, token_type_ids, attention_mask)
        let embeddings =
            self.model
                .forward(&input_ids, &token_type_ids, Some(&attention_tensor))?;

        // Apply mean pooling
        let pooled = self.mean_pooling(&embeddings, &attention_tensor)?;

        Ok(pooled)
    }

    fn dimensions(&self) -> usize {
        384 // BGE-small-en-v1.5 has 384 dimensions
    }
}
