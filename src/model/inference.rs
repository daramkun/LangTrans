use std::path::{Path, PathBuf};
use std::sync::Mutex;

use candle_core::{DType, Device, IndexOp, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::qwen2::{Config, ModelForCausalLM as Model};
use hf_hub::{api::tokio::Api, Repo, RepoType};
use tokenizers::Tokenizer;

use super::language::Language;
use super::prompt::build_translation_prompt;

const MAX_NEW_TOKENS: usize = 128; // Reduced from 512 - translations are usually short

pub struct InferenceEngine {
    model: Mutex<Model>,
    device: Device,
    tokenizer: Tokenizer,
    eos_token_ids: Vec<u32>,
}

impl InferenceEngine {
    /// Download model files from HuggingFace Hub
    async fn download_model_files(model_id: &str, _cache_dir: &Path) -> anyhow::Result<PathBuf> {
        tracing::info!("Downloading model {} from HuggingFace Hub...", model_id);

        let api = Api::new()?;
        let repo = api.repo(Repo::new(model_id.to_string(), RepoType::Model));

        // Download required files
        let config_path = repo.get("config.json").await?;
        let _tokenizer_path = repo.get("tokenizer.json").await?;

        tracing::info!("Downloaded config and tokenizer");

        // Download safetensors files (try both patterns)
        if let Ok(_single_file) = repo.get("model.safetensors").await {
            tracing::info!("Downloaded model.safetensors");
        } else {
            // Try sharded model files
            tracing::info!("Downloading sharded model files...");
            let mut index = 1;
            loop {
                // Get actual filename from index.json or try common patterns
                let actual_filename = if index == 1 {
                    "model-00001-of-00002.safetensors".to_string()
                } else if index == 2 {
                    "model-00002-of-00002.safetensors".to_string()
                } else {
                    break;
                };

                match repo.get(&actual_filename).await {
                    Ok(_path) => {
                        tracing::info!("Downloaded {}", actual_filename);
                        index += 1;
                    }
                    Err(_) => {
                        if index == 1 {
                            return Err(anyhow::anyhow!("No safetensors files found for model {}", model_id));
                        }
                        break;
                    }
                }
            }
        }

        // Return the directory containing the model files
        Ok(config_path.parent().unwrap().to_path_buf())
    }

    pub async fn new(model_id: &str, cache_dir: &Path) -> anyhow::Result<Self> {
        // Download model from HuggingFace if needed
        let model_dir = if cache_dir.join("config.json").exists() {
            tracing::info!("Using cached model from {}", cache_dir.display());
            cache_dir.to_path_buf()
        } else {
            Self::download_model_files(model_id, cache_dir).await?
        };

        // Device selection: Metal on macOS, CPU otherwise
        let device = Self::select_device()?;
        tracing::info!("Using device: {:?}", device);

        // Load configuration
        let config_path = model_dir.join("config.json");
        let config_file = std::fs::File::open(&config_path)
            .map_err(|e| anyhow::anyhow!("Failed to open config.json: {}", e))?;

        let config: Config = serde_json::from_reader(config_file)
            .map_err(|e| anyhow::anyhow!("Failed to parse config into Qwen2 Config: {}", e))?;

        tracing::info!(
            "Model config: {} layers, {} heads, {} vocab",
            config.num_hidden_layers,
            config.num_attention_heads,
            config.vocab_size
        );

        // Load tokenizer
        let tokenizer_path = model_dir.join("tokenizer.json");
        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| anyhow::anyhow!("Failed to load tokenizer: {}", e))?;

        // Determine EOS token IDs for Qwen2
        let eos_token_ids = vec![
            tokenizer.token_to_id("<|im_end|>").unwrap_or(151645),
            tokenizer.token_to_id("<|endoftext|>").unwrap_or(151643),
            151645, // Default Qwen2 EOS
            151643, // Alternative EOS
        ];
        tracing::info!("EOS token IDs: {:?}", eos_token_ids);

        // Load model weights from safetensors
        let safetensors_path = model_dir.join("model.safetensors");

        // Check if file exists
        if !safetensors_path.exists() {
            // Try model-*.safetensors pattern
            let pattern = model_dir.join("model-*.safetensors");
            let glob_pattern = pattern.to_str()
                .ok_or_else(|| anyhow::anyhow!("Invalid path pattern"))?;

            let files: Vec<_> = glob::glob(glob_pattern)
                .map_err(|e| anyhow::anyhow!("Glob pattern error: {}", e))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| anyhow::anyhow!("Glob error: {}", e))?;

            if files.is_empty() {
                return Err(anyhow::anyhow!(
                    "No safetensors files found in {}. Expected model.safetensors or model-*.safetensors",
                    model_dir.display()
                ));
            }

            tracing::info!("Loading model from {} safetensors files", files.len());

            // Use F32 on CPU devices to avoid unsupported BF16 matmul on CPU.
            // Use BF16 on GPU/Metal when available to save memory and potentially improve perf.
            let dtype = match &device {
                Device::Cpu => DType::F32,
                _ => DType::BF16,
            };

            tracing::info!("Loading model with dtype: {:?}", dtype);

            let vb = unsafe {
                VarBuilder::from_mmaped_safetensors(&files, dtype, &device)?
            };
            let model = Model::new(&config, vb)?;

            Ok(InferenceEngine {
                model: Mutex::new(model),
                device,
                tokenizer,
                eos_token_ids,
            })
        } else {
            tracing::info!("Loading model from single safetensors file");

            // Use F32 on CPU devices to avoid unsupported BF16 matmul on CPU.
            // Use BF16 on GPU/Metal when available to save memory and potentially improve perf.
            let dtype = match &device {
                Device::Cpu => DType::F32,
                _ => DType::BF16,
            };

            tracing::info!("Loading model with dtype: {:?}", dtype);

            let vb = unsafe {
                VarBuilder::from_mmaped_safetensors(&[safetensors_path], dtype, &device)?
            };
            let model = Model::new(&config, vb)?;

            Ok(InferenceEngine {
                model: Mutex::new(model),
                device,
                tokenizer,
                eos_token_ids,
            })
        }
    }

    fn select_device() -> anyhow::Result<Device> {
        // 우선순위: cuda (NVIDIA) > metal (Apple) > cpu
        // ROCm (AMD): candle 미지원으로 현재 CPU fallback
        #[cfg(feature = "cuda")]
        {
            match Device::new_cuda(0) {
                Ok(device) => {
                    tracing::info!("CUDA device initialized successfully (GPU acceleration enabled)");
                    return Ok(device);
                }
                Err(e) => {
                    tracing::warn!("CUDA initialization failed ({}), trying next backend", e);
                }
            }
        }

        #[cfg(feature = "metal")]
        {
            match Device::new_metal(0) {
                Ok(device) => {
                    tracing::info!("Metal device initialized successfully (GPU acceleration enabled)");
                    return Ok(device);
                }
                Err(e) => {
                    tracing::warn!("Metal initialization failed ({}), falling back to CPU", e);
                }
            }
        }

        #[cfg(feature = "rocm")]
        tracing::warn!("ROCm is not yet supported by candle, using CPU (https://github.com/huggingface/candle/discussions)");

        tracing::info!("Using CPU device");
        Ok(Device::Cpu)
    }



    pub fn translate(
        &self,
        from: Language,
        to: Language,
        text: &str,
    ) -> anyhow::Result<String> {
        let prompt = build_translation_prompt(from, to, text);

        // Tokenize
        let encoding = self
            .tokenizer
            .encode(prompt, false)
            .map_err(|e| anyhow::anyhow!("Tokenization failed: {}", e))?;
        let input_ids: Vec<u32> = encoding.get_ids().to_vec();

        tracing::debug!("Input tokens: {} tokens", input_ids.len());

        // Convert to tensor
        let input_tensor = Tensor::new(input_ids.as_slice(), &self.device)?
            .unsqueeze(0)?
            .contiguous()?; // Ensure contiguous memory layout

        // Lock model for inference
        let mut model = self
            .model
            .lock()
            .map_err(|e| anyhow::anyhow!("Model lock poisoned: {}", e))?;

        // Clear KV cache before new generation
        model.clear_kv_cache();

        // Prefill pass
        let logits = model.forward(&input_tensor, 0)?.contiguous()?;
        let mut next_token = Self::sample_token(&logits)?;

        let mut generated_tokens = Vec::new();
        let mut pos = input_ids.len();

        // Autoregressive decode loop
        for _step in 0..MAX_NEW_TOKENS {
            if self.eos_token_ids.contains(&next_token) {
                tracing::debug!("EOS token {} encountered at step {}", next_token, _step);
                break;
            }

            generated_tokens.push(next_token);

            // Prepare next token tensor
            let next_token_tensor = Tensor::new(&[next_token], &self.device)?
                .unsqueeze(0)?
                .contiguous()?;

            // Forward pass with KV cache
            let logits = model.forward(&next_token_tensor, pos)?.contiguous()?;
            next_token = Self::sample_token(&logits)?;
            pos += 1;
        }

        tracing::debug!("Generated {} tokens", generated_tokens.len());

        // Decode
        let output_text = self
            .tokenizer
            .decode(&generated_tokens, true)
            .map_err(|e| anyhow::anyhow!("Decoding failed: {}", e))?;

        Ok(output_text.trim().to_string())
    }

    fn sample_token(logits: &Tensor) -> anyhow::Result<u32> {
        // Simple greedy sampling (argmax)
        // logits shape: [batch=1, seq_len, vocab_size]
        // Get last position logits: [batch=1, vocab_size]
        let seq_len = logits.dim(1)?;
        let logits = logits.i((.., seq_len - 1, ..))?.contiguous()?;
        let logits = logits.squeeze(0)?.contiguous()?; // Remove batch dim

        // Use Candle's argmax for better performance
        let token = logits.argmax(0)?.to_scalar::<u32>()?;
        Ok(token)
    }
}
