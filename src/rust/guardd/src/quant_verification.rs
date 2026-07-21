use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};

/// Configuration for quantization verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantizationConfig {
    pub fan_in: u32,
    pub fan_out: u32,
    pub quant_type: String,
}

/// Object-form layer payload accepted by `guardd_verify_model_128_vectors`.
///
/// JSON shape (matches Python bindings):
/// ```json
/// {"name": "layer", "weights": [0.1, ...], "fan_in": 10, "fan_out": 5, "quant_type": "int8"}
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerSpec {
    pub name: String,
    pub weights: Vec<f32>,
    pub fan_in: u32,
    pub fan_out: u32,
    #[serde(default = "default_quant_type")]
    pub quant_type: String,
}

fn default_quant_type() -> String {
    "int8".to_string()
}

impl LayerSpec {
    pub fn into_tuple(self) -> (String, Vec<f32>, QuantizationConfig) {
        let config = QuantizationConfig {
            fan_in: self.fan_in,
            fan_out: self.fan_out,
            quant_type: self.quant_type,
        };
        (self.name, self.weights, config)
    }
}

/// Detailed verification result with 128 vectors analysis
#[derive(Debug, Serialize, Deserialize)]
pub struct LayerVerification128 {
    pub layer_name: String,
    pub config: QuantizationConfig,
    pub epsilon_bound: f64,
    pub max_error_128_vectors: f64,
    pub mean_error_128_vectors: f64,
    pub error_std_deviation: f64,
    pub passed_128_vectors: bool,
    pub error_distribution: Vec<f64>, // 128 individual errors
    pub computation_time_ms: u64,
}

/// Multi-layer verification result
#[derive(Debug, Serialize, Deserialize)]
pub struct ModelVerification128 {
    pub layers: Vec<LayerVerification128>,
    pub total_layers: usize,
    pub passed_layers: usize,
    pub overall_pass_rate: f64,
    pub mean_max_error: f64,
    pub worst_layer: Option<String>,
    pub total_computation_time_ms: u64,
}

/// Generate a random activation vector with deterministic seed
pub fn generate_random_vector(n: usize, seed: u64) -> Vec<f32> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut vector = Vec::with_capacity(n);

    for _ in 0..n {
        // Generate values in range [-1, 1]
        let value = rng.gen_range(-1.0..1.0);
        vector.push(value);
    }

    vector
}

/// Generate 128 random activation vectors
pub fn generate_128_vectors(n: usize) -> Vec<Vec<f32>> {
    let mut vectors = Vec::with_capacity(128);

    for i in 0..128 {
        let vector = generate_random_vector(n, i as u64);
        vectors.push(vector);
    }

    vectors
}

/// Compute L2 norm of a vector
pub fn l2_norm(vector: &[f32]) -> f32 {
    vector.iter().map(|x| x * x).sum::<f32>().sqrt()
}

/// Matrix-vector multiplication: W * x
pub fn matvec_mul(weights: &[f32], fan_in: usize, fan_out: usize, vector: &[f32]) -> Vec<f32> {
    let mut result = Vec::with_capacity(fan_out);

    for i in 0..fan_out {
        let mut sum = 0.0;
        for j in 0..fan_in {
            sum += weights[i * fan_in + j] * vector[j];
        }
        result.push(sum);
    }

    result
}

/// Quantize weights to int8
pub fn quantize_int8(weights: &[f32]) -> Vec<i8> {
    weights
        .iter()
        .map(|&w| {
            let quantized = (w * 127.0).round().clamp(-127.0, 127.0);
            quantized as i8
        })
        .collect()
}

/// Dequantize int8 weights back to float
pub fn dequantize_int8(quantized: &[i8]) -> Vec<f32> {
    quantized.iter().map(|&q| (q as f32) / 127.0).collect()
}

/// Compute epsilon bound based on quantization type and fan_in
pub fn compute_epsilon_bound(config: &QuantizationConfig) -> f64 {
    match config.quant_type.as_str() {
        "int8" => 0.5 * (config.fan_in as f64).sqrt(),
        "fp16" => 2.0 * (config.fan_in as f64).sqrt(),
        _ => 1.0 * (config.fan_in as f64).sqrt(),
    }
}

/// Verify a single layer with 128 random activation vectors.
///
/// Error metric (ε definition): for each activation `x`,
/// `e = ||W_q x - W x||_2 / ||x||_2`, then compare `max e` to `epsilon_bound`.
///
/// Only `quant_type == "int8"` is implemented. Other types fail closed
/// (`passed_128_vectors = false`, infinite max error).
pub fn verify_layer_128_vectors(
    layer_name: &str,
    weights: &[f32],
    config: &QuantizationConfig,
) -> LayerVerification128 {
    let start_time = std::time::Instant::now();

    let epsilon_bound = compute_epsilon_bound(config);
    let fan_in = config.fan_in as usize;
    let fan_out = config.fan_out as usize;
    let expected_len = fan_in.saturating_mul(fan_out);

    if config.quant_type != "int8" {
        let computation_time = start_time.elapsed().as_millis() as u64;
        return LayerVerification128 {
            layer_name: layer_name.to_string(),
            config: config.clone(),
            epsilon_bound,
            max_error_128_vectors: f64::INFINITY,
            mean_error_128_vectors: f64::INFINITY,
            error_std_deviation: 0.0,
            passed_128_vectors: false,
            error_distribution: vec![f64::INFINITY; 128],
            computation_time_ms: computation_time,
        };
    }

    if weights.len() != expected_len || fan_in == 0 || fan_out == 0 {
        let computation_time = start_time.elapsed().as_millis() as u64;
        return LayerVerification128 {
            layer_name: layer_name.to_string(),
            config: config.clone(),
            epsilon_bound,
            max_error_128_vectors: f64::INFINITY,
            mean_error_128_vectors: f64::INFINITY,
            error_std_deviation: 0.0,
            passed_128_vectors: false,
            error_distribution: vec![f64::INFINITY; 128],
            computation_time_ms: computation_time,
        };
    }

    // Quantize and dequantize weights (int8 only)
    let quantized = quantize_int8(weights);
    let dequantized = dequantize_int8(&quantized);

    // Generate 128 random activation vectors
    let vectors = generate_128_vectors(fan_in);

    // Compute errors for all 128 vectors: ||(Wq - W) x|| / ||x||
    let mut error_distribution = Vec::with_capacity(128);
    let mut sum_errors = 0.0f64;
    let mut max_error = 0.0f64;

    for vector in &vectors {
        let original_output = matvec_mul(weights, fan_in, fan_out, vector);
        let quantized_output = matvec_mul(&dequantized, fan_in, fan_out, vector);

        let mut diff = Vec::with_capacity(fan_out);
        for i in 0..fan_out {
            diff.push(quantized_output[i] - original_output[i]);
        }

        let x_norm = l2_norm(vector);
        let error = if x_norm > 0.0 {
            (l2_norm(&diff) / x_norm) as f64
        } else {
            0.0
        };

        error_distribution.push(error);
        sum_errors += error;
        max_error = max_error.max(error);
    }

    // Compute statistics
    let mean_error = sum_errors / 128.0;
    let variance = error_distribution
        .iter()
        .map(|&err| (err - mean_error).powi(2))
        .sum::<f64>()
        / 128.0;
    let std_deviation = variance.sqrt();

    let computation_time = start_time.elapsed().as_millis() as u64;

    LayerVerification128 {
        layer_name: layer_name.to_string(),
        config: config.clone(),
        epsilon_bound,
        max_error_128_vectors: max_error,
        mean_error_128_vectors: mean_error,
        error_std_deviation: std_deviation,
        passed_128_vectors: max_error <= epsilon_bound,
        error_distribution,
        computation_time_ms: computation_time,
    }
}

/// Verify multiple layers with 128 vectors each
pub fn verify_model_128_vectors(
    layers: Vec<(String, Vec<f32>, QuantizationConfig)>,
) -> ModelVerification128 {
    let mut layer_results = Vec::with_capacity(layers.len());
    let mut total_computation_time = 0u64;

    for (layer_name, weights, config) in layers {
        let verification = verify_layer_128_vectors(&layer_name, &weights, &config);
        total_computation_time += verification.computation_time_ms;
        layer_results.push(verification);
    }

    // Compute overall statistics
    let total_layers = layer_results.len();
    let passed_layers = layer_results
        .iter()
        .filter(|v| v.passed_128_vectors)
        .count();
    let overall_pass_rate = if total_layers > 0 {
        (passed_layers as f64 / total_layers as f64) * 100.0
    } else {
        0.0
    };

    let max_errors: Vec<f64> = layer_results
        .iter()
        .map(|v| v.max_error_128_vectors)
        .collect();
    let mean_max_error = if !max_errors.is_empty() {
        max_errors.iter().sum::<f64>() / max_errors.len() as f64
    } else {
        0.0
    };

    let worst_layer = layer_results
        .iter()
        .max_by(|a, b| {
            a.max_error_128_vectors
                .partial_cmp(&b.max_error_128_vectors)
                .unwrap()
        })
        .map(|v| v.layer_name.clone());

    ModelVerification128 {
        layers: layer_results,
        total_layers,
        passed_layers,
        overall_pass_rate,
        mean_max_error,
        worst_layer,
        total_computation_time_ms: total_computation_time,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_random_vector() {
        let vector = generate_random_vector(10, 42);
        assert_eq!(vector.len(), 10);

        // Test determinism
        let vector2 = generate_random_vector(10, 42);
        assert_eq!(vector, vector2);
    }

    #[test]
    fn test_generate_128_vectors() {
        let vectors = generate_128_vectors(5);
        assert_eq!(vectors.len(), 128);
        assert_eq!(vectors[0].len(), 5);
    }

    #[test]
    fn test_l2_norm() {
        let vector = vec![3.0, 4.0];
        let norm = l2_norm(&vector);
        assert!((norm - 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_matvec_mul() {
        let weights = vec![1.0, 2.0, 3.0, 4.0]; // 2x2 matrix
        let vector = vec![1.0, 2.0];
        let result = matvec_mul(&weights, 2, 2, &vector);

        assert_eq!(result.len(), 2);
        assert!((result[0] - 5.0).abs() < 1e-6); // 1*1 + 2*2
        assert!((result[1] - 11.0).abs() < 1e-6); // 3*1 + 4*2
    }

    #[test]
    fn test_quantize_dequantize() {
        let weights = vec![0.5, -0.25, 0.75];
        let quantized = quantize_int8(&weights);
        let dequantized = dequantize_int8(&quantized);

        assert_eq!(quantized.len(), 3);
        assert_eq!(dequantized.len(), 3);

        // Check that dequantized values are close to original
        for (orig, deq) in weights.iter().zip(dequantized.iter()) {
            assert!((orig - deq).abs() < 0.01);
        }
    }

    #[test]
    fn test_verify_layer_128_vectors() {
        let config = QuantizationConfig {
            fan_in: 10,
            fan_out: 5,
            quant_type: "int8".to_string(),
        };

        let weights = vec![0.1; 50]; // 5x10 matrix
        let verification = verify_layer_128_vectors("test_layer", &weights, &config);

        assert_eq!(verification.layer_name, "test_layer");
        assert_eq!(verification.error_distribution.len(), 128);
        assert!(verification.computation_time_ms > 0);
        assert!(verification.max_error_128_vectors.is_finite());
        assert!(verification.passed_128_vectors);
    }

    #[test]
    fn test_error_is_vector_delta_not_output_norm_delta() {
        // Construct a case where ||Wq x|| ≈ ||W x|| but Wq x ≠ W x would still
        // be caught by vector error. Here we just assert the metric matches
        // ||(Wq-W)x||/||x|| on a single known vector path via the API shape.
        let config = QuantizationConfig {
            fan_in: 2,
            fan_out: 2,
            quant_type: "int8".to_string(),
        };
        let weights = vec![0.5, -0.25, 0.75, 0.125];
        let verification = verify_layer_128_vectors("metric", &weights, &config);
        let x = generate_random_vector(2, 0);
        let q = dequantize_int8(&quantize_int8(&weights));
        let orig = matvec_mul(&weights, 2, 2, &x);
        let quant = matvec_mul(&q, 2, 2, &x);
        let diff: Vec<f32> = orig.iter().zip(quant.iter()).map(|(a, b)| b - a).collect();
        let expected = (l2_norm(&diff) / l2_norm(&x)) as f64;
        assert!((verification.error_distribution[0] - expected).abs() < 1e-5);
    }

    #[test]
    fn test_non_int8_fails_closed() {
        let config = QuantizationConfig {
            fan_in: 2,
            fan_out: 2,
            quant_type: "fp16".to_string(),
        };
        let weights = vec![0.1, 0.2, 0.3, 0.4];
        let verification = verify_layer_128_vectors("fp16_layer", &weights, &config);
        assert!(!verification.passed_128_vectors);
        assert!(verification.max_error_128_vectors.is_infinite());
    }

    #[test]
    fn test_verify_model_128_vectors() {
        let config = QuantizationConfig {
            fan_in: 5,
            fan_out: 3,
            quant_type: "int8".to_string(),
        };

        let layers = vec![
            ("layer1".to_string(), vec![0.1; 15], config.clone()),
            ("layer2".to_string(), vec![0.1; 15], config),
        ];

        let verification = verify_model_128_vectors(layers);

        assert_eq!(verification.total_layers, 2);
        assert_eq!(verification.layers.len(), 2);
        assert!(verification.total_computation_time_ms > 0);
    }

    #[test]
    fn test_layer_spec_object_json_contract() {
        let json = r#"[
            {
                "name": "linear.weight",
                "weights": [0.1, 0.2, 0.3, 0.4, 0.5, 0.6],
                "fan_in": 3,
                "fan_out": 2,
                "quant_type": "int8"
            }
        ]"#;
        let specs: Vec<LayerSpec> = serde_json::from_str(json).unwrap();
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].name, "linear.weight");
        let layers: Vec<_> = specs.into_iter().map(LayerSpec::into_tuple).collect();
        let verification = verify_model_128_vectors(layers);
        assert_eq!(verification.total_layers, 1);
        assert_eq!(verification.layers[0].layer_name, "linear.weight");
    }

    #[test]
    fn test_shared_epsilon_fixture_int8() {
        // tests/fixtures/quant_epsilon_int8.json — ε = 0.5 * sqrt(fan_in)
        let fixture_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../tests/fixtures/quant_epsilon_int8.json"
        );
        let raw = std::fs::read_to_string(fixture_path).expect("shared epsilon fixture");
        let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let fan_in = v["fan_in"].as_u64().unwrap() as u32;
        let fan_out = v["fan_out"].as_u64().unwrap() as u32;
        let expected_eps = v["epsilon_bound"].as_f64().unwrap();
        let weights: Vec<f32> = v["weights"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_f64().unwrap() as f32)
            .collect();
        assert_eq!(weights.len(), (fan_in * fan_out) as usize);

        let config = QuantizationConfig {
            fan_in,
            fan_out,
            quant_type: "int8".to_string(),
        };
        let eps = compute_epsilon_bound(&config);
        assert!((eps - expected_eps).abs() < 1e-9);
        // Lean LayerBound uses the same closed form: 0.5 * sqrt(fan_in)
        assert!((eps - 0.5 * (fan_in as f64).sqrt()).abs() < 1e-12);

        let verification = verify_layer_128_vectors("fixture.linear", &weights, &config);
        assert_eq!(verification.epsilon_bound, eps);
        assert!(verification.max_error_128_vectors.is_finite());
        assert!(verification.passed_128_vectors);
    }
}
