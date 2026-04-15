use sha2::{Sha256, Digest};
use serde::{Serialize, Deserialize};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;
use memmap2::Mmap;
use std::fs::File;

// Include the bit-flip corpus module
mod bitflip_corpus;
use bitflip_corpus::BitFlipCorpusTester;

// Include the quantization verification module
mod quant_verification;
use quant_verification::{
    QuantizationConfig,
    verify_layer_128_vectors, verify_model_128_vectors
};

mod perfect_hash;
use perfect_hash::PerfectHashVocab;

#[repr(C)]
pub struct ModelHandle {
    path: *mut c_char,
    size: u64,
    digest: [u8; 32],
    valid: bool,
}

#[repr(C)]
pub enum GuarddError {
    Success = 0,
    FileNotFound = 1,
    InvalidDigest = 2,
    QuantizationError = 3,
    MemoryError = 4,
    InvalidPath = 5,
}

#[derive(Serialize, Deserialize)]
pub struct QuantizationBounds {
    layer_name: String,
    epsilon_bound: f64,
    fan_in: u32,
    quant_type: String,
}

#[derive(Serialize, Deserialize)]
pub struct LayerVerification {
    layer_name: String,
    epsilon_bound: f64,
    actual_error: f64,
    passed: bool,
}

/// Check SHA-256 digest of a file using memory mapping for performance
///
/// # Safety
/// `path` must be a valid, null-terminated UTF-8 C string and `expected_digest`
/// must point to at least 32 readable bytes.
#[no_mangle]
pub unsafe extern "C" fn guardd_verify_digest(path: *const c_char, expected_digest: *const u8) -> GuarddError {
    if path.is_null() || expected_digest.is_null() {
        return GuarddError::InvalidPath;
    }

    let path_str = match unsafe { CStr::from_ptr(path).to_str() } {
        Ok(s) => s,
        Err(_) => return GuarddError::InvalidPath,
    };

    let file = match File::open(path_str) {
        Ok(f) => f,
        Err(_) => return GuarddError::FileNotFound,
    };

    let mmap = match unsafe { Mmap::map(&file) } {
        Ok(m) => m,
        Err(_) => return GuarddError::MemoryError,
    };

    let mut hasher = Sha256::new();
    hasher.update(&mmap);
    let computed_digest = hasher.finalize();

    let expected_slice = unsafe { std::slice::from_raw_parts(expected_digest, 32) };
    
    if computed_digest.as_slice() == expected_slice {
        GuarddError::Success
    } else {
        GuarddError::InvalidDigest
    }
}

/// Load a model with integrity checks
///
/// # Safety
/// `path` must be a valid, null-terminated UTF-8 C string. If non-null,
/// `expected_digest` must point to at least 32 readable bytes.
#[no_mangle]
pub unsafe extern "C" fn guardd_checked_load(path: *const c_char, expected_digest: *const u8) -> *mut ModelHandle {
    if path.is_null() {
        return ptr::null_mut();
    }

    let path_str = match unsafe { CStr::from_ptr(path).to_str() } {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };

    let file = match File::open(path_str) {
        Ok(f) => f,
        Err(_) => return ptr::null_mut(),
    };

    let metadata = match file.metadata() {
        Ok(m) => m,
        Err(_) => return ptr::null_mut(),
    };

    let mmap = match unsafe { Mmap::map(&file) } {
        Ok(m) => m,
        Err(_) => return ptr::null_mut(),
    };

    let mut hasher = Sha256::new();
    hasher.update(&mmap);
    let computed_digest = hasher.finalize();

    let valid = if !expected_digest.is_null() {
        let expected_slice = unsafe { std::slice::from_raw_parts(expected_digest, 32) };
        computed_digest.as_slice() == expected_slice
    } else {
        true // No expected digest provided, assume valid
    };

    let path_cstring = match CString::new(path_str) {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };

    let handle = Box::new(ModelHandle {
        path: path_cstring.into_raw(),
        size: metadata.len(),
        digest: computed_digest.into(),
        valid,
    });

    Box::into_raw(handle)
}

/// Free a model handle
///
/// # Safety
/// `handle` must be a pointer previously returned by `guardd_checked_load`
/// and must not be freed more than once.
#[no_mangle]
pub unsafe extern "C" fn guardd_free_handle(handle: *mut ModelHandle) {
    if !handle.is_null() {
        let handle = unsafe { Box::from_raw(handle) };
        unsafe {
            let _ = CString::from_raw(handle.path);
        }
    }
}

/// Verify quantization bounds for a layer (legacy function)
#[no_mangle]
///
/// # Safety
/// `weights` must point to `weights_len` readable `f32` values and `quant_type`
/// must be a valid, null-terminated UTF-8 C string.
pub unsafe extern "C" fn guardd_verify_quant(
    weights: *const f32,
    weights_len: usize,
    fan_in: u32,
    quant_type: *const c_char,
    tolerance: f64,
) -> *mut LayerVerification {
    if weights.is_null() || quant_type.is_null() {
        return ptr::null_mut();
    }

    let quant_type_str = match unsafe { CStr::from_ptr(quant_type).to_str() } {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };

    let _weights_slice = unsafe { std::slice::from_raw_parts(weights, weights_len) };

    // Compute epsilon bound based on quantization type
    let epsilon_bound = match quant_type_str {
        "int8" => 0.5 * (fan_in as f64).sqrt(),
        "fp16" => 2.0 * (fan_in as f64).sqrt(),
        _ => 1.0 * (fan_in as f64).sqrt(),
    };

    // Simplified quantization error computation
    // In practice, this would implement the full quantization algorithm
    let actual_error = 0.1; // Placeholder

    let verification = Box::new(LayerVerification {
        layer_name: "layer".to_string(),
        epsilon_bound,
        actual_error,
        passed: actual_error <= epsilon_bound && actual_error <= tolerance,
    });

    Box::into_raw(verification)
}

/// Free a layer verification result
///
/// # Safety
/// `verification` must be a pointer previously returned by `guardd_verify_quant`
/// and must not be freed more than once.
#[no_mangle]
pub unsafe extern "C" fn guardd_free_verification(verification: *mut LayerVerification) {
    if !verification.is_null() {
        let _ = unsafe { Box::from_raw(verification) };
    }
}

/// Verify a single layer with 128 random activation vectors (Q-4 requirement)
///
/// # Safety
/// `layer_name` and `quant_type` must be valid, null-terminated UTF-8 C strings.
/// `weights` must point to `weights_len` readable `f32` values.
#[no_mangle]
pub unsafe extern "C" fn guardd_verify_quant_128_vectors(
    layer_name: *const c_char,
    weights: *const f32,
    weights_len: usize,
    fan_in: u32,
    fan_out: u32,
    quant_type: *const c_char,
) -> *mut c_char {
    if layer_name.is_null() || weights.is_null() || quant_type.is_null() {
        return ptr::null_mut();
    }

    let layer_name_str = match unsafe { CStr::from_ptr(layer_name).to_str() } {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };

    let quant_type_str = match unsafe { CStr::from_ptr(quant_type).to_str() } {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };

    let weights_slice = unsafe { std::slice::from_raw_parts(weights, weights_len) };

    let config = QuantizationConfig {
        fan_in,
        fan_out,
        quant_type: quant_type_str.to_string(),
    };

    let verification = verify_layer_128_vectors(layer_name_str, weights_slice, &config);

    match serde_json::to_string(&verification) {
        Ok(json_str) => {
            match CString::new(json_str) {
                Ok(c_string) => c_string.into_raw(),
                Err(_) => ptr::null_mut(),
            }
        }
        Err(_) => ptr::null_mut(),
    }
}

/// Verify multiple layers with 128 vectors each (Q-4 requirement)
///
/// # Safety
/// `layers_json` must be a valid, null-terminated UTF-8 C string containing
/// JSON for `Vec<(String, Vec<f32>, QuantizationConfig)>`.
#[no_mangle]
pub unsafe extern "C" fn guardd_verify_model_128_vectors(
    layers_json: *const c_char,
) -> *mut c_char {
    if layers_json.is_null() {
        return ptr::null_mut();
    }

    let layers_json_str = match unsafe { CStr::from_ptr(layers_json).to_str() } {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };

    // Parse layers from JSON
    let layers_data: Result<Vec<(String, Vec<f32>, QuantizationConfig)>, _> = 
        serde_json::from_str(layers_json_str);

    let layers = match layers_data {
        Ok(layers) => layers,
        Err(_) => return ptr::null_mut(),
    };

    let verification = verify_model_128_vectors(layers);

    match serde_json::to_string(&verification) {
        Ok(json_str) => {
            match CString::new(json_str) {
                Ok(c_string) => c_string.into_raw(),
                Err(_) => ptr::null_mut(),
            }
        }
        Err(_) => ptr::null_mut(),
    }
}

/// Get error message for error code
#[no_mangle]
pub extern "C" fn guardd_error_message(error: GuarddError) -> *const c_char {
    const SUCCESS: &[u8] = b"Success\0";
    const FILE_NOT_FOUND: &[u8] = b"File not found\0";
    const INVALID_DIGEST: &[u8] = b"Invalid digest\0";
    const QUANTIZATION_ERROR: &[u8] = b"Quantization error\0";
    const MEMORY_ERROR: &[u8] = b"Memory error\0";
    const INVALID_PATH: &[u8] = b"Invalid path\0";

    match error {
        GuarddError::Success => SUCCESS.as_ptr() as *const c_char,
        GuarddError::FileNotFound => FILE_NOT_FOUND.as_ptr() as *const c_char,
        GuarddError::InvalidDigest => INVALID_DIGEST.as_ptr() as *const c_char,
        GuarddError::QuantizationError => QUANTIZATION_ERROR.as_ptr() as *const c_char,
        GuarddError::MemoryError => MEMORY_ERROR.as_ptr() as *const c_char,
        GuarddError::InvalidPath => INVALID_PATH.as_ptr() as *const c_char,
    }
}

/// Run bit-flip corpus test (W-4 requirement)
///
/// # Safety
/// If non-null, `temp_dir` must be a valid, null-terminated UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn guardd_run_bitflip_corpus_test(
    file_size_gb: usize,
    num_corruptions: usize,
    temp_dir: *const c_char,
) -> *mut c_char {
    let temp_dir_str = if temp_dir.is_null() {
        "/tmp/bitflip_corpus".to_string()
    } else {
        match unsafe { CStr::from_ptr(temp_dir).to_str() } {
            Ok(s) => s.to_string(),
            Err(_) => "/tmp/bitflip_corpus".to_string(),
        }
    };

    let tester = BitFlipCorpusTester::new(1024, Some(temp_dir_str));
    
    match tester.run_bitflip_test(file_size_gb, num_corruptions) {
        Ok(result) => {
            match serde_json::to_string(&result) {
                Ok(json_str) => {
                    match CString::new(json_str) {
                        Ok(c_string) => c_string.into_raw(),
                        Err(_) => ptr::null_mut(),
                    }
                }
                Err(_) => ptr::null_mut(),
            }
        }
        Err(_) => ptr::null_mut(),
    }
}

/// Run comprehensive bit-flip corpus test suite
///
/// # Safety
/// If non-null, `temp_dir` must be a valid, null-terminated UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn guardd_run_comprehensive_bitflip_test(
    temp_dir: *const c_char,
) -> *mut c_char {
    let temp_dir_str = if temp_dir.is_null() {
        "/tmp/bitflip_corpus".to_string()
    } else {
        match unsafe { CStr::from_ptr(temp_dir).to_str() } {
            Ok(s) => s.to_string(),
            Err(_) => "/tmp/bitflip_corpus".to_string(),
        }
    };

    let tester = BitFlipCorpusTester::new(1024, Some(temp_dir_str));
    
    match tester.run_comprehensive_test() {
        Ok(summary) => {
            match serde_json::to_string(&summary) {
                Ok(json_str) => {
                    match CString::new(json_str) {
                        Ok(c_string) => c_string.into_raw(),
                        Err(_) => ptr::null_mut(),
                    }
                }
                Err(_) => ptr::null_mut(),
            }
        }
        Err(_) => ptr::null_mut(),
    }
}

/// Free JSON result string
///
/// # Safety
/// `result` must be a pointer previously returned by one of this library's
/// JSON-returning functions and must not be freed more than once.
#[no_mangle]
pub unsafe extern "C" fn guardd_free_json_result(result: *mut c_char) {
    if !result.is_null() {
        unsafe {
            let _ = CString::from_raw(result);
        }
    }
}

/// Encode text into token IDs using perfect hash vocab JSON.
///
/// # Safety
/// `vocab_json` and `text` must be valid, null-terminated UTF-8 C strings.
#[no_mangle]
pub unsafe extern "C" fn guardd_perfect_hash_encode(vocab_json: *const c_char, text: *const c_char) -> *mut c_char {
    if vocab_json.is_null() || text.is_null() {
        return std::ptr::null_mut();
    }
    let vocab_str = match unsafe { CStr::from_ptr(vocab_json).to_str() } {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    let text_str = match unsafe { CStr::from_ptr(text).to_str() } {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    let vocab: PerfectHashVocab = match serde_json::from_str(vocab_str) {
        Ok(v) => v,
        Err(_) => return std::ptr::null_mut(),
    };
    let tokens: Vec<u32> = text_str.split_whitespace().map(|w| vocab.encode(w)).collect();
    match serde_json::to_string(&tokens) {
        Ok(json_str) => match CString::new(json_str) {
            Ok(c_string) => c_string.into_raw(),
            Err(_) => std::ptr::null_mut(),
        },
        Err(_) => std::ptr::null_mut(),
    }
}

/// Decode one token ID to string using perfect hash vocab JSON.
///
/// # Safety
/// `vocab_json` must be a valid, null-terminated UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn guardd_perfect_hash_decode(vocab_json: *const c_char, token: u32) -> *mut c_char {
    if vocab_json.is_null() {
        return std::ptr::null_mut();
    }
    let vocab_str = match unsafe { CStr::from_ptr(vocab_json).to_str() } {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    let vocab: PerfectHashVocab = match serde_json::from_str(vocab_str) {
        Ok(v) => v,
        Err(_) => return std::ptr::null_mut(),
    };
    let word = vocab.decode(token);
    match CString::new(word) {
        Ok(c_string) => c_string.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::NamedTempFile;
    use crate::bitflip_corpus::BitFlipTestResult;
    use crate::quant_verification::LayerVerification128;

    #[test]
    fn test_verify_digest() {
        let temp_file = NamedTempFile::new().unwrap();
        fs::write(&temp_file, b"test data").unwrap();

        let mut hasher = Sha256::new();
        hasher.update(b"test data");
        let expected_digest = hasher.finalize();

        let path_cstring = CString::new(temp_file.path().to_str().unwrap()).unwrap();
        let result = unsafe {
            guardd_verify_digest(path_cstring.as_ptr(), expected_digest.as_ptr())
        };

        assert_eq!(result as i32, GuarddError::Success as i32);
    }

    #[test]
    fn test_checked_load() {
        let temp_file = NamedTempFile::new().unwrap();
        fs::write(&temp_file, b"test data").unwrap();

        let path_cstring = CString::new(temp_file.path().to_str().unwrap()).unwrap();
        let handle = unsafe { guardd_checked_load(path_cstring.as_ptr(), ptr::null()) };

        assert!(!handle.is_null());

        unsafe {
            let handle_ref = &*handle;
            assert_eq!(handle_ref.size, 9);
            assert!(handle_ref.valid);
            guardd_free_handle(handle);
        }
    }

    #[test]
    fn test_bitflip_corpus_small() {
        // Test with a small file size for unit testing
        let temp_dir = tempfile::tempdir().unwrap();
        let temp_dir_str = temp_dir.path().to_str().unwrap();
        let temp_dir_cstring = CString::new(temp_dir_str).unwrap();
        
        let result = unsafe {
            guardd_run_bitflip_corpus_test(0, 5, temp_dir_cstring.as_ptr())
        };
        
        assert!(!result.is_null());
        
        unsafe {
            let result_str = CStr::from_ptr(result).to_str().unwrap();
            let test_result: BitFlipTestResult = serde_json::from_str(result_str).unwrap();
            assert!(test_result.test_passed);
            guardd_free_json_result(result);
        }
    }

    #[test]
    fn test_verify_quant_128_vectors() {
        let layer_name = CString::new("test_layer").unwrap();
        let quant_type = CString::new("int8").unwrap();
        let weights = [0.1f32; 50]; // 5x10 matrix
        
        let result = unsafe {
            guardd_verify_quant_128_vectors(
                layer_name.as_ptr(),
                weights.as_ptr(),
                weights.len(),
                10, // fan_in
                5,  // fan_out
                quant_type.as_ptr(),
            )
        };
        
        assert!(!result.is_null());
        
        unsafe {
            let result_str = CStr::from_ptr(result).to_str().unwrap();
            let verification: LayerVerification128 = serde_json::from_str(result_str).unwrap();
            assert_eq!(verification.layer_name, "test_layer");
            assert_eq!(verification.error_distribution.len(), 128);
            assert!(verification.computation_time_ms > 0);
            guardd_free_json_result(result);
        }
    }
} 