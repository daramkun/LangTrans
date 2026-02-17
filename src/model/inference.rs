use std::borrow::Cow;
use std::path::Path;
use std::sync::Mutex;

use ort::session::builder::GraphOptimizationLevel;
use ort::session::{Session, SessionInputValue, SessionOutputs};
use ort::value::{Tensor, ValueType};
use tokenizers::Tokenizer;

use super::language::Language;
use super::prompt::build_translation_prompt;

const MAX_NEW_TOKENS: usize = 512;

type OrtInputs<'a> = Vec<(Cow<'a, str>, SessionInputValue<'a>)>;

pub struct InferenceEngine {
    session: Mutex<Session>,
    tokenizer: Tokenizer,
    num_layers: usize,
    num_heads: usize,
    head_dim: usize,
    eos_token_id: u32,
}

impl InferenceEngine {
    pub fn new(model_dir: &Path) -> anyhow::Result<Self> {
        ort::init()
            .with_execution_providers([ort::execution_providers::CPUExecutionProvider::default().build()])
            .commit();

        let model_path = model_dir.join("model.onnx");
        let tokenizer_path = model_dir.join("tokenizer.json");

        let session = Session::builder()?
            .with_optimization_level(GraphOptimizationLevel::Level3)?
            .with_intra_threads(4)?
            .commit_from_file(&model_path)?;

        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| anyhow::anyhow!("Failed to load tokenizer: {}", e))?;

        let (num_layers, num_heads, head_dim) = Self::discover_kv_cache_dims(&session)?;

        let eos_token_id = tokenizer
            .token_to_id("<end_of_turn>")
            .unwrap_or(1);

        tracing::info!(
            "InferenceEngine initialized: {} layers, {} heads, {} head_dim, eos_token_id={}",
            num_layers, num_heads, head_dim, eos_token_id
        );

        Ok(InferenceEngine {
            session: Mutex::new(session),
            tokenizer,
            num_layers,
            num_heads,
            head_dim,
            eos_token_id,
        })
    }

    fn discover_kv_cache_dims(session: &Session) -> anyhow::Result<(usize, usize, usize)> {
        let mut num_layers = 0usize;
        let mut num_heads = 0usize;
        let mut head_dim = 0usize;

        for output in session.outputs() {
            tracing::debug!("Model output: {}", output.name());
        }
        for input in session.inputs() {
            let name = input.name();
            tracing::debug!("Model input: {} {:?}", name, input.dtype());
            if name.contains("past_key_values") && name.ends_with(".key") {
                num_layers += 1;
                // Extract dimensions from dtype shape
                if let ValueType::Tensor { shape, .. } = input.dtype() {
                    // Shape: [batch_size, num_heads, seq_len, head_dim]
                    // Shape derefs to &[i64], dynamic dims are -1
                    if shape.len() == 4 {
                        if shape[1] > 0 {
                            num_heads = shape[1] as usize;
                        }
                        if shape[3] > 0 {
                            head_dim = shape[3] as usize;
                        }
                    }
                }
            }
        }

        if num_layers == 0 {
            return Err(anyhow::anyhow!(
                "Could not discover KV cache dimensions from model inputs. \
                 Expected inputs with names containing 'past_key_values'"
            ));
        }

        // Fallback defaults for Gemma3-4B if dynamic
        if num_heads == 0 {
            num_heads = 8;
        }
        if head_dim == 0 {
            head_dim = 256;
        }

        Ok((num_layers, num_heads, head_dim))
    }

    fn make_empty_kv(&self) -> anyhow::Result<Tensor<f32>> {
        // Shape: [1, num_heads, 1, head_dim] with zeros — minimal dummy cache
        // ONNX Runtime doesn't allow dim=0 via from_array, so we use dim=1 with zeros.
        // The attention_mask controls which positions are actually attended to.
        let size = self.num_heads * self.head_dim;
        Ok(Tensor::from_array((
            vec![1usize, self.num_heads, 1, self.head_dim],
            vec![0.0f32; size],
        ))?)
    }

    pub fn translate(
        &self,
        from: Language,
        to: Language,
        text: &str,
    ) -> anyhow::Result<String> {
        let prompt = build_translation_prompt(from, to, text);

        let encoding = self
            .tokenizer
            .encode(prompt, false)
            .map_err(|e| anyhow::anyhow!("Tokenization failed: {}", e))?;
        let input_ids: Vec<i64> = encoding.get_ids().iter().map(|&id| id as i64).collect();
        let seq_len = input_ids.len();

        let input_ids_tensor = Tensor::from_array((vec![1usize, seq_len], input_ids.clone()))?;
        // attention_mask: 1 leading zero for the dummy KV cache position, then 1s for real tokens
        let mut mask = vec![0i64; 1];
        mask.extend(vec![1i64; seq_len]);
        let attention_mask = Tensor::from_array((vec![1usize, seq_len + 1], mask))?;

        let mut inputs: OrtInputs = vec![
            (Cow::from("input_ids"), input_ids_tensor.into()),
            (Cow::from("attention_mask"), attention_mask.into()),
        ];

        for layer in 0..self.num_layers {
            inputs.push((
                Cow::from(format!("past_key_values.{}.key", layer)),
                self.make_empty_kv()?.into(),
            ));
            inputs.push((
                Cow::from(format!("past_key_values.{}.value", layer)),
                self.make_empty_kv()?.into(),
            ));
        }

        // Prefill pass
        let mut session = self.session.lock().map_err(|e| anyhow::anyhow!("Session lock poisoned: {}", e))?;

        let (mut next_token, mut kv_cache) = {
            let outputs = session.run(inputs)?;
            let (logits_shape, logits_data) = outputs["logits"].try_extract_tensor::<f32>()?;
            let token = Self::argmax_last_position(logits_shape, logits_data);
            let cache = self.extract_kv_cache(&outputs)?;
            (token, cache)
        };

        let mut generated_tokens = Vec::new();

        // Autoregressive decode loop
        for _step in 0..MAX_NEW_TOKENS {
            if next_token == self.eos_token_id {
                break;
            }
            generated_tokens.push(next_token);

            let step_input = Tensor::from_array((vec![1usize, 1], vec![next_token as i64]))?;
            // total_len = past KV cache length (from outputs), +1 for current token
            // The KV cache after prefill has seq_len+1 positions (1 dummy + seq_len real)
            // Each decode step adds 1 more
            let past_kv_len = seq_len + 1 + generated_tokens.len() - 1; // length of present.*.key seq dim
            let mask_len = past_kv_len + 1; // past + current token
            let step_mask = Tensor::from_array((vec![1usize, mask_len], vec![1i64; mask_len]))?;

            let mut step_inputs: OrtInputs = vec![
                (Cow::from("input_ids"), step_input.into()),
                (Cow::from("attention_mask"), step_mask.into()),
            ];

            for layer in 0..self.num_layers {
                let (key_val, value_val) = kv_cache.remove(layer);
                step_inputs.push((Cow::from(format!("past_key_values.{}.key", layer)), key_val));
                step_inputs.push((Cow::from(format!("past_key_values.{}.value", layer)), value_val));
            }

            let (tok, cache) = {
                let step_outputs = session.run(step_inputs)?;
                let (sl_shape, sl_data) = step_outputs["logits"].try_extract_tensor::<f32>()?;
                let t = Self::argmax_last_position(sl_shape, sl_data);
                let c = self.extract_kv_cache(&step_outputs)?;
                (t, c)
            };
            next_token = tok;
            kv_cache = cache;
        }

        let output_text = self
            .tokenizer
            .decode(&generated_tokens, true)
            .map_err(|e| anyhow::anyhow!("Decoding failed: {}", e))?;

        Ok(output_text.trim().to_string())
    }

    fn argmax_last_position(shape: &ort::tensor::Shape, data: &[f32]) -> u32 {
        // shape: [batch=1, seq_len, vocab_size]
        let seq_len = shape[1] as usize;
        let vocab_size = shape[2] as usize;

        let last_start = (seq_len - 1) * vocab_size;
        let last_logits = &data[last_start..last_start + vocab_size];

        let mut max_idx = 0;
        let mut max_val = f32::NEG_INFINITY;
        for (i, &val) in last_logits.iter().enumerate() {
            if val > max_val {
                max_val = val;
                max_idx = i;
            }
        }
        max_idx as u32
    }

    fn extract_kv_cache(&self, outputs: &SessionOutputs) -> anyhow::Result<KvCache> {
        let mut layers = Vec::with_capacity(self.num_layers);

        for layer in 0..self.num_layers {
            let key_name = format!("present.{}.key", layer);
            let value_name = format!("present.{}.value", layer);

            let (k_shape, k_data) = outputs[key_name.as_str()].try_extract_tensor::<f32>()?;
            let (v_shape, v_data) = outputs[value_name.as_str()].try_extract_tensor::<f32>()?;

            // Recreate owned tensors from extracted data
            let k_dims: Vec<usize> = k_shape.iter().map(|&d| d as usize).collect();
            let v_dims: Vec<usize> = v_shape.iter().map(|&d| d as usize).collect();

            let key_tensor = Tensor::from_array((k_dims, k_data.to_vec()))?;
            let value_tensor = Tensor::from_array((v_dims, v_data.to_vec()))?;

            layers.push((
                SessionInputValue::from(key_tensor),
                SessionInputValue::from(value_tensor),
            ));
        }

        Ok(KvCache { layers })
    }
}

struct KvCache {
    layers: Vec<(SessionInputValue<'static>, SessionInputValue<'static>)>,
}

impl KvCache {
    fn remove(&mut self, index: usize) -> (SessionInputValue<'static>, SessionInputValue<'static>) {
        let placeholder = Tensor::from_array((vec![1usize, 1, 1, 1], vec![0.0f32]))
            .unwrap();
        let key = std::mem::replace(&mut self.layers[index].0, SessionInputValue::from(placeholder));

        let placeholder2 = Tensor::from_array((vec![1usize, 1, 1, 1], vec![0.0f32]))
            .unwrap();
        let value = std::mem::replace(&mut self.layers[index].1, SessionInputValue::from(placeholder2));

        (key, value)
    }
}
