use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use serde::{Serialize, Deserialize};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;

/// Configuration for quantization verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantizationConfig {
    pub fan_in: u32,
    pub fan_out: u32,
    pub quant_type: String,
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
    weights.iter().map(|&w| {
        let quantized = (w * 127.0).round().clamp(-127.0, 127.0);
        quantized as i8
    }).collect()
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

/// Verify a single layer with 128 random activation vectors
pub fn verify_layer_128_vectors(
    layer_name: &str,
    weights: &[f32],
    config: &QuantizationConfig,
) -> LayerVerification128 {
    let start_time = std::time::Instant::now();
    
    let epsilon_bound = compute_epsilon_bound(config);
    let fan_in = config.fan_in as usize;
    let fan_out = config.fan_out as usize;
    
    // Quantize and dequantize weights
    let quantized = quantize_int8(weights);
    let dequantized = dequantize_int8(&quantized);
    
    // Generate 128 random activation vectors
    let vectors = generate_128_vectors(fan_in);
    
    // Compute errors for all 128 vectors
    let mut error_distribution = Vec::with_capacity(128);
    let mut sum_errors = 0.0f64;
    let mut max_error = 0.0f64;
    
    for vector in &vectors {
        // Compute original output
        let original_output = matvec_mul(weights, fan_in, fan_out, vector);
        let original_norm = l2_norm(&original_output);
        
        // Compute quantized output
        let quantized_output = matvec_mul(&dequantized, fan_in, fan_out, vector);
        let quantized_norm = l2_norm(&quantized_output);
        
        // Compute error
        let error = if original_norm > 0.0 {
            (quantized_norm - original_norm).abs() / original_norm
        } else {
            0.0
        };
        
        error_distribution.push(error as f64);
        sum_errors += error as f64;
        max_error = max_error.max(error as f64);
    }
    
    // Compute statistics
    let mean_error = sum_errors / 128.0;
    let variance = error_distribution.iter()
        .map(|&err| (err - mean_error).powi(2))
        .sum::<f64>() / 128.0;
    let std_deviation = variance.sqrt();
    
    let computation_time = start_time.elapsed().as_millis() as u64;
    
    LayerVerification128 {
        layer_name: layer_name.to_string(),
        config: config.clone(),
        epsilon_bound,
        max_error_128_vectors: max_error,
        mean_error_128_vectors: mean_error as f64,
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
    let start_time = std::time::Instant::now();
    
    let mut layer_results = Vec::with_capacity(layers.len());
    let mut total_computation_time = 0u64;
    
    for (layer_name, weights, config) in layers {
        let verification = verify_layer_128_vectors(&layer_name, &weights, &config);
        total_computation_time += verification.computation_time_ms;
        layer_results.push(verification);
    }
    
    // Compute overall statistics
    let total_layers = layer_results.len();
    let passed_layers = layer_results.iter().filter(|v| v.passed_128_vectors).count();
    let overall_pass_rate = if total_layers > 0 {
        (passed_layers as f64 / total_layers as f64) * 100.0
    } else {
        0.0
    };
    
    let max_errors: Vec<f64> = layer_results.iter().map(|v| v.max_error_128_vectors).collect();
    let mean_max_error = if !max_errors.is_empty() {
        max_errors.iter().sum::<f64>() / max_errors.len() as f64
    } else {
        0.0
    };
    
    let worst_layer = layer_results.iter()
        .max_by(|a, b| a.max_error_128_vectors.partial_cmp(&b.max_error_128_vectors).unwrap())
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
} 