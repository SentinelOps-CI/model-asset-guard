use memmap2::Mmap;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::ffi::{CStr, CString};
use std::fs::File;
use std::os::raw::c_char;
use std::path::Path;
use std::ptr;

pub mod bitflip_corpus;
use bitflip_corpus::BitFlipCorpusTester;

pub mod quant_verification;
use quant_verification::{
    verify_layer_128_vectors, verify_model_128_vectors, LayerSpec, QuantizationConfig,
};

pub mod perfect_hash;
use perfect_hash::PerfectHashVocab;

pub mod bpe;
pub mod sentencepiece;
pub mod sp_lattice;
pub mod sp_normalizer;

/// Safe Rust view of a checked-load result (not part of the C ABI).
#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub path: String,
    pub size: u64,
    pub digest: [u8; 32],
    pub valid: bool,
}

/// Safe digest verification. Returns `Ok(true)` on match, `Ok(false)` on mismatch.
pub fn verify_digest(path: &str, expected_digest: &[u8]) -> Result<bool, GuarddError> {
    if expected_digest.len() != 32 {
        return Err(GuarddError::InvalidDigest);
    }
    let digest = compute_file_digest(path)?;
    Ok(digest.as_slice() == expected_digest)
}

/// Safe checked load with optional expected digest.
pub fn checked_load(path: &str, expected_digest: Option<&[u8]>) -> Result<ModelInfo, GuarddError> {
    if let Some(expected) = expected_digest {
        if expected.len() != 32 {
            return Err(GuarddError::InvalidDigest);
        }
    }

    let file = File::open(path).map_err(|_| GuarddError::FileNotFound)?;
    let metadata = file.metadata().map_err(|_| GuarddError::FileNotFound)?;
    let mmap = unsafe { Mmap::map(&file) }.map_err(|_| GuarddError::MemoryError)?;

    let mut hasher = Sha256::new();
    hasher.update(&mmap);
    let computed: [u8; 32] = hasher.finalize().into();

    let valid = match expected_digest {
        Some(expected) => computed.as_slice() == expected,
        None => true,
    };

    Ok(ModelInfo {
        path: path.to_string(),
        size: metadata.len(),
        digest: computed,
        valid,
    })
}

/// Safe 128-vector quantization check for one layer.
pub fn verify_quantization_128_vectors(
    layer_name: &str,
    weights: &[f32],
    fan_in: u32,
    fan_out: u32,
    quant_type: &str,
) -> quant_verification::LayerVerification128 {
    let config = QuantizationConfig {
        fan_in,
        fan_out,
        quant_type: quant_type.to_string(),
    };
    verify_layer_128_vectors(layer_name, weights, &config)
}

/// Safe multi-layer 128-vector check from object-form [`LayerSpec`] values.
pub fn verify_model_quantization_128_vectors(
    layers: Vec<(String, Vec<f32>, QuantizationConfig)>,
) -> quant_verification::ModelVerification128 {
    verify_model_128_vectors(layers)
}

/// Safe bit-flip corpus check. `temp_dir = None` uses the platform temp dir
/// (see [`bitflip_corpus::default_bitflip_temp_dir`]).
pub fn run_bitflip_corpus_test(
    file_size_gb: usize,
    num_corruptions: usize,
    temp_dir: Option<String>,
) -> Result<bitflip_corpus::BitFlipTestResult, String> {
    let tester = BitFlipCorpusTester::new(1024, temp_dir);
    tester
        .run_bitflip_test(file_size_gb, num_corruptions)
        .map_err(|e| e.to_string())
}

/// Safe open-addressed hash-vocab encode (whitespace-split words → token ids).
pub fn perfect_hash_encode(vocab_json: &str, text: &str) -> Result<Vec<u32>, String> {
    let vocab: PerfectHashVocab =
        serde_json::from_str(vocab_json).map_err(|e| format!("invalid vocab JSON: {e}"))?;
    Ok(text.split_whitespace().map(|w| vocab.encode(w)).collect())
}

/// Safe open-addressed hash-vocab decode for a single token id.
pub fn perfect_hash_decode(vocab_json: &str, token: u32) -> Result<String, String> {
    let vocab: PerfectHashVocab =
        serde_json::from_str(vocab_json).map_err(|e| format!("invalid vocab JSON: {e}"))?;
    Ok(vocab.decode(token).to_string())
}

/// Safe SentencePiece encode from a `.model` path.
pub fn sp_encode(
    model_path: &str,
    text: &str,
    options: sentencepiece::EncodeOptions,
) -> Result<Vec<u32>, String> {
    let model = sentencepiece::load_model_proto_file(model_path)?;
    sentencepiece::encode_with_options(&model, text, options)
}

/// Safe SentencePiece decode from a `.model` path.
pub fn sp_decode(model_path: &str, tokens: &[u32]) -> Result<String, String> {
    sp_decode_with_options(model_path, tokens, sentencepiece::DecodeOptions::default())
}

/// Safe SentencePiece decode with ExtraOptions-style knobs (`reverse`).
pub fn sp_decode_with_options(
    model_path: &str,
    tokens: &[u32],
    options: sentencepiece::DecodeOptions,
) -> Result<String, String> {
    let model = sentencepiece::load_model_proto_file(model_path)?;
    sentencepiece::decode_pieces_with_options(&model, tokens, options)
}

/// Safe UNIGRAM n-best encode (other model types error).
pub fn sp_nbest_encode(
    model_path: &str,
    text: &str,
    nbest_size: usize,
    options: sentencepiece::EncodeOptions,
) -> Result<Vec<(Vec<u32>, f32)>, String> {
    let model = sentencepiece::load_model_proto_file(model_path)?;
    sentencepiece::nbest_encode_with_options(&model, text, nbest_size, options)
}

/// Safe sample encode (UNIGRAM lattice sample / BPE-dropout).
pub fn sp_sample_encode(
    model_path: &str,
    text: &str,
    sample: sentencepiece::SampleEncodeOptions,
) -> Result<Vec<u32>, String> {
    let model = sentencepiece::load_model_proto_file(model_path)?;
    sentencepiece::sample_encode_with_options(&model, text, sample)
}

fn compute_file_digest(path: &str) -> Result<[u8; 32], GuarddError> {
    if !Path::new(path).exists() {
        return Err(GuarddError::FileNotFound);
    }
    let file = File::open(path).map_err(|_| GuarddError::FileNotFound)?;
    let mmap = unsafe { Mmap::map(&file) }.map_err(|_| GuarddError::MemoryError)?;
    let mut hasher = Sha256::new();
    hasher.update(&mmap);
    Ok(hasher.finalize().into())
}

#[repr(C)]
pub struct ModelHandle {
    path: *mut c_char,
    size: u64,
    digest: [u8; 32],
    valid: bool,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
pub unsafe extern "C" fn guardd_verify_digest(
    path: *const c_char,
    expected_digest: *const u8,
) -> GuarddError {
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
pub unsafe extern "C" fn guardd_checked_load(
    path: *const c_char,
    expected_digest: *const u8,
) -> *mut ModelHandle {
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

/// Legacy quantization entry point — **unimplemented** (fail-closed).
///
/// Always returns null; never allocates and never reports a pass/fail verdict.
/// Callers must treat null as “API unavailable”, not as success.
///
/// **Deprecated:** use [`guardd_verify_quant_128_vectors`] (or
/// [`guardd_verify_model_128_vectors`]) for the supported 128-vector sampling path.
///
/// # Safety
/// Pointers are unused; null is returned without reading them.
#[deprecated(
    note = "unimplemented; always returns null. Use guardd_verify_quant_128_vectors"
)]
#[no_mangle]
pub unsafe extern "C" fn guardd_verify_quant(
    _weights: *const f32,
    _weights_len: usize,
    _fan_in: u32,
    _quant_type: *const c_char,
    _tolerance: f64,
) -> *mut LayerVerification {
    // Fail-closed: do not report false pass/fail results.
    ptr::null_mut()
}

/// Free a layer verification result from the legacy [`guardd_verify_quant`] ABI.
///
/// That entry point always returns null today, so this free is a no-op on the
/// supported path. Prefer JSON results from `guardd_verify_quant_128_vectors`
/// (freed with [`guardd_free_json_result`]).
///
/// # Safety
/// `verification` must be null or a pointer previously returned by
/// `guardd_verify_quant`, and must not be freed more than once.
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
        Ok(json_str) => match CString::new(json_str) {
            Ok(c_string) => c_string.into_raw(),
            Err(_) => ptr::null_mut(),
        },
        Err(_) => ptr::null_mut(),
    }
}

/// Verify multiple layers with 128 vectors each (Q-4 requirement)
///
/// # Safety
/// `layers_json` must be a valid, null-terminated UTF-8 C string containing
/// a JSON array of [`LayerSpec`] objects:
/// `[{"name","weights","fan_in","fan_out","quant_type"}, ...]`.
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

    let layers_data: Result<Vec<LayerSpec>, _> = serde_json::from_str(layers_json_str);

    let layers = match layers_data {
        Ok(specs) => specs.into_iter().map(LayerSpec::into_tuple).collect(),
        Err(_) => return ptr::null_mut(),
    };

    let verification = verify_model_128_vectors(layers);

    match serde_json::to_string(&verification) {
        Ok(json_str) => match CString::new(json_str) {
            Ok(c_string) => c_string.into_raw(),
            Err(_) => ptr::null_mut(),
        },
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
        None
    } else {
        match unsafe { CStr::from_ptr(temp_dir).to_str() } {
            Ok(s) => Some(s.to_string()),
            Err(_) => None,
        }
    };

    let tester = BitFlipCorpusTester::new(1024, temp_dir_str);

    match tester.run_bitflip_test(file_size_gb, num_corruptions) {
        Ok(result) => match serde_json::to_string(&result) {
            Ok(json_str) => match CString::new(json_str) {
                Ok(c_string) => c_string.into_raw(),
                Err(_) => ptr::null_mut(),
            },
            Err(_) => ptr::null_mut(),
        },
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
        None
    } else {
        match unsafe { CStr::from_ptr(temp_dir).to_str() } {
            Ok(s) => Some(s.to_string()),
            Err(_) => None,
        }
    };

    let tester = BitFlipCorpusTester::new(1024, temp_dir_str);

    match tester.run_comprehensive_test() {
        Ok(summary) => match serde_json::to_string(&summary) {
            Ok(json_str) => match CString::new(json_str) {
                Ok(c_string) => c_string.into_raw(),
                Err(_) => ptr::null_mut(),
            },
            Err(_) => ptr::null_mut(),
        },
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
pub unsafe extern "C" fn guardd_perfect_hash_encode(
    vocab_json: *const c_char,
    text: *const c_char,
) -> *mut c_char {
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
    let tokens: Vec<u32> = text_str
        .split_whitespace()
        .map(|w| vocab.encode(w))
        .collect();
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
pub unsafe extern "C" fn guardd_perfect_hash_decode(
    vocab_json: *const c_char,
    token: u32,
) -> *mut c_char {
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

fn sp_json_ok_tokens(tokens: &[u32], model_type: &str) -> *mut c_char {
    let payload = serde_json::json!({
        "ok": true,
        "tokens": tokens,
        "model_type": model_type,
    });
    match CString::new(payload.to_string()) {
        Ok(c) => c.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

fn sp_json_ok_text(text: &str) -> *mut c_char {
    let payload = serde_json::json!({
        "ok": true,
        "text": text,
    });
    match CString::new(payload.to_string()) {
        Ok(c) => c.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

fn sp_json_err(msg: &str) -> *mut c_char {
    let payload = serde_json::json!({
        "ok": false,
        "error": msg,
    });
    match CString::new(payload.to_string()) {
        Ok(c) => c.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

fn sp_json_ok_nbest(nbests: &[(Vec<u32>, f32)], model_type: &str) -> *mut c_char {
    let rows: Vec<serde_json::Value> = nbests
        .iter()
        .map(|(tokens, score)| {
            serde_json::json!({
                "tokens": tokens,
                "score": score,
            })
        })
        .collect();
    let payload = serde_json::json!({
        "ok": true,
        "nbests": rows,
        "model_type": model_type,
    });
    match CString::new(payload.to_string()) {
        Ok(c) => c.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

fn parse_sp_encode_opts(opt_str: &str) -> Result<sentencepiece::EncodeOptions, String> {
    #[derive(Deserialize)]
    struct SpEncodeOpts {
        #[serde(default)]
        allow_silent_unk: bool,
        #[serde(default)]
        forced_pieces: Vec<String>,
        #[serde(default)]
        word_official_split: bool,
        #[serde(default = "default_merge_consecutive_unk")]
        merge_consecutive_unk: bool,
        #[serde(default)]
        reverse: bool,
        #[serde(default)]
        add_bos: bool,
        #[serde(default)]
        add_eos: bool,
        #[serde(default)]
        nbest_stock_compat: bool,
    }
    fn default_merge_consecutive_unk() -> bool {
        true
    }
    let o: SpEncodeOpts =
        serde_json::from_str(opt_str).map_err(|e| format!("invalid options JSON: {e}"))?;
    Ok(sentencepiece::EncodeOptions {
        allow_silent_unk: o.allow_silent_unk,
        forced_pieces: o.forced_pieces,
        word_official_split: o.word_official_split,
        merge_consecutive_unk: o.merge_consecutive_unk,
        reverse: o.reverse,
        add_bos: o.add_bos,
        add_eos: o.add_eos,
        nbest_stock_compat: o.nbest_stock_compat,
    })
}

fn parse_sp_decode_opts(opt_str: &str) -> Result<sentencepiece::DecodeOptions, String> {
    #[derive(Deserialize)]
    struct SpDecodeOpts {
        #[serde(default)]
        reverse: bool,
    }
    let o: SpDecodeOpts =
        serde_json::from_str(opt_str).map_err(|e| format!("invalid decode options JSON: {e}"))?;
    Ok(sentencepiece::DecodeOptions { reverse: o.reverse })
}

/// SentencePiece encode from a `.model` / `.proto` path (CHAR/BPE/UNIGRAM when gated).
///
/// Returns owned JSON: `{"ok":true,"tokens":[...],"model_type":"..."}` or
/// `{"ok":false,"error":"..."}`. Caller must free with `guardd_free_json_result`.
///
/// Fail-closed OOV default (no silent `<unk>`).
///
/// # Safety
/// `model_path` and `text` must be valid, null-terminated UTF-8 C strings.
#[no_mangle]
pub unsafe extern "C" fn guardd_sp_encode(
    model_path: *const c_char,
    text: *const c_char,
) -> *mut c_char {
    unsafe { guardd_sp_encode_with_options(model_path, text, ptr::null()) }
}

/// SentencePiece encode with optional JSON options.
///
/// `options_json` may be null or `{}` for defaults. Supported keys:
/// - `allow_silent_unk` (bool, default false): stock-SP OOV → UNKNOWN id
/// - `forced_pieces` (string[], default []): extra PrefixMatcher pieces; each
///   must already exist in the model vocab
/// - `word_official_split` (bool, default false): WORD flag-blind SplitIntoWords
/// - `merge_consecutive_unk` (bool, default true): stock SP processor merge of
///   consecutive UNKNOWN ids (ignored when `byte_fallback` is enabled)
/// - `reverse` (bool, default false): ExtraOptions reverse **before** bos/eos
///   (matches stock documented `reverse:bos:eos`; not free-form colon order)
/// - `add_bos` / `add_eos` (bool, default false): ExtraOptions CONTROL wrap after merge
/// - `nbest_stock_compat` (bool, default false): UNIGRAM n-best stock agenda
///   shrink + timeout→Viterbi (default refuses pathological cases)
///
/// # Safety
/// Pointers must be valid null-terminated UTF-8 C strings when non-null.
#[no_mangle]
pub unsafe extern "C" fn guardd_sp_encode_with_options(
    model_path: *const c_char,
    text: *const c_char,
    options_json: *const c_char,
) -> *mut c_char {
    if model_path.is_null() || text.is_null() {
        return sp_json_err("null argument");
    }
    let path = match unsafe { CStr::from_ptr(model_path).to_str() } {
        Ok(s) => s,
        Err(_) => return sp_json_err("model_path is not valid UTF-8"),
    };
    let text_str = match unsafe { CStr::from_ptr(text).to_str() } {
        Ok(s) => s,
        Err(_) => return sp_json_err("text is not valid UTF-8"),
    };
    let mut opts = sentencepiece::EncodeOptions::default();
    if !options_json.is_null() {
        let opt_str = match unsafe { CStr::from_ptr(options_json).to_str() } {
            Ok(s) => s,
            Err(_) => return sp_json_err("options_json is not valid UTF-8"),
        };
        if !opt_str.is_empty() {
            match parse_sp_encode_opts(opt_str) {
                Ok(o) => opts = o,
                Err(e) => return sp_json_err(&e),
            }
        }
    }
    let model = match sentencepiece::load_model_proto_file(path) {
        Ok(m) => m,
        Err(e) => return sp_json_err(&e),
    };
    match sentencepiece::encode_with_options(&model, text_str, opts) {
        Ok(tokens) => sp_json_ok_tokens(&tokens, model.model_type.as_str()),
        Err(e) => sp_json_err(&e),
    }
}

/// UNIGRAM lattice n-best encode.
///
/// `options_json` may include encode opts plus `nbest_size` (default 1, max 1024).
///
/// # Safety
/// Pointers must be valid null-terminated UTF-8 C strings when non-null.
#[no_mangle]
pub unsafe extern "C" fn guardd_sp_nbest_encode(
    model_path: *const c_char,
    text: *const c_char,
    options_json: *const c_char,
) -> *mut c_char {
    if model_path.is_null() || text.is_null() {
        return sp_json_err("null argument");
    }
    let path = match unsafe { CStr::from_ptr(model_path).to_str() } {
        Ok(s) => s,
        Err(_) => return sp_json_err("model_path is not valid UTF-8"),
    };
    let text_str = match unsafe { CStr::from_ptr(text).to_str() } {
        Ok(s) => s,
        Err(_) => return sp_json_err("text is not valid UTF-8"),
    };
    #[derive(Deserialize, Default)]
    struct NbestOpts {
        #[serde(default)]
        allow_silent_unk: bool,
        #[serde(default)]
        forced_pieces: Vec<String>,
        #[serde(default)]
        word_official_split: bool,
        #[serde(default = "default_merge_unk_nbest")]
        merge_consecutive_unk: bool,
        #[serde(default)]
        reverse: bool,
        #[serde(default)]
        add_bos: bool,
        #[serde(default)]
        add_eos: bool,
        #[serde(default)]
        nbest_stock_compat: bool,
        #[serde(default = "default_nbest")]
        nbest_size: usize,
    }
    fn default_nbest() -> usize {
        1
    }
    fn default_merge_unk_nbest() -> bool {
        true
    }
    let mut nbest_size = 1usize;
    let mut opts = sentencepiece::EncodeOptions::default();
    if !options_json.is_null() {
        let opt_str = match unsafe { CStr::from_ptr(options_json).to_str() } {
            Ok(s) => s,
            Err(_) => return sp_json_err("options_json is not valid UTF-8"),
        };
        if !opt_str.is_empty() {
            match serde_json::from_str::<NbestOpts>(opt_str) {
                Ok(o) => {
                    opts.allow_silent_unk = o.allow_silent_unk;
                    opts.forced_pieces = o.forced_pieces;
                    opts.word_official_split = o.word_official_split;
                    opts.merge_consecutive_unk = o.merge_consecutive_unk;
                    opts.reverse = o.reverse;
                    opts.add_bos = o.add_bos;
                    opts.add_eos = o.add_eos;
                    opts.nbest_stock_compat = o.nbest_stock_compat;
                    nbest_size = o.nbest_size;
                }
                Err(e) => return sp_json_err(&format!("invalid options JSON: {e}")),
            }
        }
    }
    let model = match sentencepiece::load_model_proto_file(path) {
        Ok(m) => m,
        Err(e) => return sp_json_err(&e),
    };
    match sentencepiece::nbest_encode_with_options(&model, text_str, nbest_size, opts) {
        Ok(nbests) => sp_json_ok_nbest(&nbests, model.model_type.as_str()),
        Err(e) => sp_json_err(&e),
    }
}

/// UNIGRAM sample-encode.
///
/// Options: encode keys plus `nbest_size` (i32), `alpha` (f32), optional `seed` (u64).
///
/// # Safety
/// Pointers must be valid null-terminated UTF-8 C strings when non-null.
#[no_mangle]
pub unsafe extern "C" fn guardd_sp_sample_encode(
    model_path: *const c_char,
    text: *const c_char,
    options_json: *const c_char,
) -> *mut c_char {
    if model_path.is_null() || text.is_null() {
        return sp_json_err("null argument");
    }
    let path = match unsafe { CStr::from_ptr(model_path).to_str() } {
        Ok(s) => s,
        Err(_) => return sp_json_err("model_path is not valid UTF-8"),
    };
    let text_str = match unsafe { CStr::from_ptr(text).to_str() } {
        Ok(s) => s,
        Err(_) => return sp_json_err("text is not valid UTF-8"),
    };
    #[derive(Deserialize)]
    struct SampleOpts {
        #[serde(default)]
        allow_silent_unk: bool,
        #[serde(default)]
        forced_pieces: Vec<String>,
        #[serde(default)]
        word_official_split: bool,
        #[serde(default = "default_merge_unk_sample")]
        merge_consecutive_unk: bool,
        #[serde(default)]
        reverse: bool,
        #[serde(default)]
        add_bos: bool,
        #[serde(default)]
        add_eos: bool,
        #[serde(default)]
        nbest_stock_compat: bool,
        #[serde(default = "default_sample_nbest")]
        nbest_size: i32,
        #[serde(default)]
        alpha: f32,
        #[serde(default)]
        seed: Option<u64>,
    }
    fn default_sample_nbest() -> i32 {
        -1
    }
    fn default_merge_unk_sample() -> bool {
        true
    }
    let mut sample = sentencepiece::SampleEncodeOptions::default();
    if !options_json.is_null() {
        let opt_str = match unsafe { CStr::from_ptr(options_json).to_str() } {
            Ok(s) => s,
            Err(_) => return sp_json_err("options_json is not valid UTF-8"),
        };
        if !opt_str.is_empty() {
            match serde_json::from_str::<SampleOpts>(opt_str) {
                Ok(o) => {
                    sample.encode.allow_silent_unk = o.allow_silent_unk;
                    sample.encode.forced_pieces = o.forced_pieces;
                    sample.encode.word_official_split = o.word_official_split;
                    sample.encode.merge_consecutive_unk = o.merge_consecutive_unk;
                    sample.encode.reverse = o.reverse;
                    sample.encode.add_bos = o.add_bos;
                    sample.encode.add_eos = o.add_eos;
                    sample.encode.nbest_stock_compat = o.nbest_stock_compat;
                    sample.nbest_size = o.nbest_size;
                    sample.alpha = o.alpha;
                    sample.seed = o.seed;
                }
                Err(e) => return sp_json_err(&format!("invalid options JSON: {e}")),
            }
        }
    }
    let model = match sentencepiece::load_model_proto_file(path) {
        Ok(m) => m,
        Err(e) => return sp_json_err(&e),
    };
    match sentencepiece::sample_encode_with_options(&model, text_str, sample) {
        Ok(tokens) => sp_json_ok_tokens(&tokens, model.model_type.as_str()),
        Err(e) => sp_json_err(&e),
    }
}

/// UNIGRAM SampleEncodeAndScore (with- or without-replacement).
///
/// Options: encode keys plus `num_samples` (usize), `alpha` (f32), `wor` (bool),
/// `include_best` (bool), optional `seed` (u64).
///
/// Returns `{"ok":true,"samples":[{"tokens":[...],"score":...},...],"model_type":"..."}`.
///
/// # Safety
/// Pointers must be valid null-terminated UTF-8 C strings when non-null.
#[no_mangle]
pub unsafe extern "C" fn guardd_sp_sample_encode_and_score(
    model_path: *const c_char,
    text: *const c_char,
    options_json: *const c_char,
) -> *mut c_char {
    if model_path.is_null() || text.is_null() {
        return sp_json_err("null argument");
    }
    let path = match unsafe { CStr::from_ptr(model_path).to_str() } {
        Ok(s) => s,
        Err(_) => return sp_json_err("model_path is not valid UTF-8"),
    };
    let text_str = match unsafe { CStr::from_ptr(text).to_str() } {
        Ok(s) => s,
        Err(_) => return sp_json_err("text is not valid UTF-8"),
    };
    #[derive(Deserialize)]
    struct ScoreOpts {
        #[serde(default)]
        allow_silent_unk: bool,
        #[serde(default)]
        forced_pieces: Vec<String>,
        #[serde(default)]
        word_official_split: bool,
        #[serde(default = "default_merge_unk_score")]
        merge_consecutive_unk: bool,
        #[serde(default)]
        reverse: bool,
        #[serde(default)]
        add_bos: bool,
        #[serde(default)]
        add_eos: bool,
        #[serde(default)]
        nbest_stock_compat: bool,
        #[serde(default = "default_num_samples")]
        num_samples: usize,
        #[serde(default)]
        alpha: f32,
        #[serde(default)]
        wor: bool,
        #[serde(default)]
        include_best: bool,
        #[serde(default)]
        seed: Option<u64>,
    }
    fn default_num_samples() -> usize {
        1
    }
    fn default_merge_unk_score() -> bool {
        true
    }
    let mut sample = sentencepiece::SampleEncodeAndScoreOptions::default();
    if !options_json.is_null() {
        let opt_str = match unsafe { CStr::from_ptr(options_json).to_str() } {
            Ok(s) => s,
            Err(_) => return sp_json_err("options_json is not valid UTF-8"),
        };
        if !opt_str.is_empty() {
            match serde_json::from_str::<ScoreOpts>(opt_str) {
                Ok(o) => {
                    sample.encode.allow_silent_unk = o.allow_silent_unk;
                    sample.encode.forced_pieces = o.forced_pieces;
                    sample.encode.word_official_split = o.word_official_split;
                    sample.encode.merge_consecutive_unk = o.merge_consecutive_unk;
                    sample.encode.reverse = o.reverse;
                    sample.encode.add_bos = o.add_bos;
                    sample.encode.add_eos = o.add_eos;
                    sample.encode.nbest_stock_compat = o.nbest_stock_compat;
                    sample.num_samples = o.num_samples;
                    sample.alpha = o.alpha;
                    sample.wor = o.wor;
                    sample.include_best = o.include_best;
                    sample.seed = o.seed;
                }
                Err(e) => return sp_json_err(&format!("invalid options JSON: {e}")),
            }
        }
    }
    let model = match sentencepiece::load_model_proto_file(path) {
        Ok(m) => m,
        Err(e) => return sp_json_err(&e),
    };
    match sentencepiece::sample_encode_and_score_with_options(&model, text_str, sample) {
        Ok(rows) => {
            let samples: Vec<serde_json::Value> = rows
                .iter()
                .map(|(tokens, score)| {
                    serde_json::json!({
                        "tokens": tokens,
                        "score": score,
                    })
                })
                .collect();
            let payload = serde_json::json!({
                "ok": true,
                "samples": samples,
                "model_type": model.model_type.as_str(),
            });
            match CString::new(payload.to_string()) {
                Ok(c) => c.into_raw(),
                Err(_) => std::ptr::null_mut(),
            }
        }
        Err(e) => sp_json_err(&e),
    }
}

/// SentencePiece decode: concatenate piece strings for token ids (JSON array).
///
/// `tokens_json` is a JSON array of u32, e.g. `[1,2,3]`.
/// Returns owned JSON: `{"ok":true,"text":"..."}` or `{"ok":false,"error":"..."}`.
///
/// # Safety
/// `model_path` and `tokens_json` must be valid, null-terminated UTF-8 C strings.
#[no_mangle]
pub unsafe extern "C" fn guardd_sp_decode(
    model_path: *const c_char,
    tokens_json: *const c_char,
) -> *mut c_char {
    unsafe { guardd_sp_decode_with_options(model_path, tokens_json, ptr::null()) }
}

/// SentencePiece decode with optional JSON options.
///
/// `options_json` may be null or `{}` for defaults. Supported keys:
/// - `reverse` (bool, default false): reverse ids before detokenize (stock
///   decode ExtraOptions `reverse`)
///
/// # Safety
/// Pointers must be valid null-terminated UTF-8 C strings when non-null.
#[no_mangle]
pub unsafe extern "C" fn guardd_sp_decode_with_options(
    model_path: *const c_char,
    tokens_json: *const c_char,
    options_json: *const c_char,
) -> *mut c_char {
    if model_path.is_null() || tokens_json.is_null() {
        return sp_json_err("null argument");
    }
    let path = match unsafe { CStr::from_ptr(model_path).to_str() } {
        Ok(s) => s,
        Err(_) => return sp_json_err("model_path is not valid UTF-8"),
    };
    let tokens_str = match unsafe { CStr::from_ptr(tokens_json).to_str() } {
        Ok(s) => s,
        Err(_) => return sp_json_err("tokens_json is not valid UTF-8"),
    };
    let tokens: Vec<u32> = match serde_json::from_str(tokens_str) {
        Ok(t) => t,
        Err(e) => return sp_json_err(&format!("invalid tokens JSON: {e}")),
    };
    let mut opts = sentencepiece::DecodeOptions::default();
    if !options_json.is_null() {
        let opt_str = match unsafe { CStr::from_ptr(options_json).to_str() } {
            Ok(s) => s,
            Err(_) => return sp_json_err("options_json is not valid UTF-8"),
        };
        if !opt_str.is_empty() {
            match parse_sp_decode_opts(opt_str) {
                Ok(o) => opts = o,
                Err(e) => return sp_json_err(&e),
            }
        }
    }
    let model = match sentencepiece::load_model_proto_file(path) {
        Ok(m) => m,
        Err(e) => return sp_json_err(&e),
    };
    match sentencepiece::decode_pieces_with_options(&model, &tokens, opts) {
        Ok(text) => sp_json_ok_text(&text),
        Err(e) => sp_json_err(&e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bitflip_corpus::BitFlipTestResult;
    use crate::quant_verification::LayerVerification128;
    use std::fs;
    use tempfile::NamedTempFile;

    #[test]
    fn test_verify_digest() {
        let temp_file = NamedTempFile::new().unwrap();
        fs::write(&temp_file, b"test data").unwrap();

        let mut hasher = Sha256::new();
        hasher.update(b"test data");
        let expected_digest = hasher.finalize();

        let path_cstring = CString::new(temp_file.path().to_str().unwrap()).unwrap();
        let result =
            unsafe { guardd_verify_digest(path_cstring.as_ptr(), expected_digest.as_ptr()) };

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

        let result = unsafe { guardd_run_bitflip_corpus_test(0, 5, temp_dir_cstring.as_ptr()) };

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

    #[test]
    fn test_verify_model_object_json_contract() {
        let layers = r#"[{
            "name": "linear.weight",
            "weights": [0.1, 0.2, 0.3, 0.4, 0.5, 0.6],
            "fan_in": 3,
            "fan_out": 2,
            "quant_type": "int8"
        }]"#;
        let layers_c = CString::new(layers).unwrap();
        let result = unsafe { guardd_verify_model_128_vectors(layers_c.as_ptr()) };
        assert!(!result.is_null(), "object-form layers JSON must parse");

        unsafe {
            let result_str = CStr::from_ptr(result).to_str().unwrap();
            let verification: crate::quant_verification::ModelVerification128 =
                serde_json::from_str(result_str).unwrap();
            assert_eq!(verification.total_layers, 1);
            assert_eq!(verification.layers[0].layer_name, "linear.weight");
            guardd_free_json_result(result);
        }
    }

    #[test]
    fn test_guardd_verify_quant_unimplemented() {
        let quant_type = CString::new("int8").unwrap();
        let weights = [0.1f32; 4];
        #[allow(deprecated)]
        let result = unsafe {
            guardd_verify_quant(weights.as_ptr(), weights.len(), 2, quant_type.as_ptr(), 0.1)
        };
        assert!(result.is_null());
    }

    #[test]
    fn test_safe_verify_digest_mismatch() {
        let temp_file = NamedTempFile::new().unwrap();
        fs::write(&temp_file, b"test data").unwrap();
        let wrong = [0u8; 32];
        let ok = verify_digest(temp_file.path().to_str().unwrap(), &wrong).unwrap();
        assert!(!ok);
    }

    #[test]
    fn test_safe_perfect_hash_and_bitflip() {
        let vocab = PerfectHashVocab::from_words(vec!["hello".into(), "world".into()]);
        let json = serde_json::to_string(&vocab).unwrap();
        let tokens = perfect_hash_encode(&json, "hello world").unwrap();
        assert_eq!(tokens, vec![1, 2]);
        assert_eq!(perfect_hash_decode(&json, 1).unwrap(), "hello");

        let dir = tempfile::tempdir().unwrap();
        let result =
            run_bitflip_corpus_test(0, 3, Some(dir.path().to_str().unwrap().to_string())).unwrap();
        assert!(result.test_passed);
    }

    #[test]
    fn test_perfect_hash_ffi_roundtrip() {
        let mph = PerfectHashVocab::from_words(vec!["hello".into(), "world".into(), "test".into()]);
        let vocab_json = serde_json::to_string(&mph).unwrap();
        let vocab_c = CString::new(vocab_json).unwrap();
        let text_c = CString::new("hello world test").unwrap();

        let enc = unsafe { guardd_perfect_hash_encode(vocab_c.as_ptr(), text_c.as_ptr()) };
        assert!(!enc.is_null());
        let tokens: Vec<u32> = unsafe {
            let s = CStr::from_ptr(enc).to_str().unwrap();
            let parsed = serde_json::from_str(s).unwrap();
            guardd_free_json_result(enc);
            parsed
        };
        assert_eq!(tokens, vec![1, 2, 3]);

        for (token, word) in tokens.into_iter().zip(["hello", "world", "test"]) {
            let dec = unsafe { guardd_perfect_hash_decode(vocab_c.as_ptr(), token) };
            assert!(!dec.is_null());
            unsafe {
                assert_eq!(CStr::from_ptr(dec).to_str().unwrap(), word);
                guardd_free_json_result(dec);
            }
        }
    }

    #[test]
    fn test_sp_encode_decode_ffi_unigram() {
        use crate::sentencepiece::ModelProtoFixture;
        use std::io::Write;

        let fixture = ModelProtoFixture {
            pieces: vec![
                ("<unk>".into(), Some(0.0), Some(2)),
                ("▁".into(), Some(-10.0), Some(1)),
                ("a".into(), Some(-5.0), Some(1)),
                ("b".into(), Some(-5.0), Some(1)),
                ("▁ab".into(), Some(-0.5), Some(1)),
            ],
            model_type: Some(1),
            add_dummy_prefix: Some(true),
            remove_extra_whitespaces: Some(true),
            escape_whitespaces: Some(true),
            ..Default::default()
        };
        let mut tmp = NamedTempFile::new().unwrap();
        tmp.write_all(&fixture.encode()).unwrap();
        let path_c = CString::new(tmp.path().to_str().unwrap()).unwrap();
        let text_c = CString::new("ab").unwrap();

        let enc = unsafe { guardd_sp_encode(path_c.as_ptr(), text_c.as_ptr()) };
        assert!(!enc.is_null());
        let enc_json: serde_json::Value = unsafe {
            let s = CStr::from_ptr(enc).to_str().unwrap();
            let v = serde_json::from_str(s).unwrap();
            guardd_free_json_result(enc);
            v
        };
        assert_eq!(enc_json["ok"], true);
        assert_eq!(enc_json["model_type"], "UNIGRAM");
        assert_eq!(enc_json["tokens"], serde_json::json!([4]));

        let tokens_c = CString::new("[4]").unwrap();
        let dec = unsafe { guardd_sp_decode(path_c.as_ptr(), tokens_c.as_ptr()) };
        assert!(!dec.is_null());
        let dec_json: serde_json::Value = unsafe {
            let s = CStr::from_ptr(dec).to_str().unwrap();
            let v = serde_json::from_str(s).unwrap();
            guardd_free_json_result(dec);
            v
        };
        assert_eq!(dec_json["ok"], true);
        assert_eq!(dec_json["text"], "ab");
    }
}
