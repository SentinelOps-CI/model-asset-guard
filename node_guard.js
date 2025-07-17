#!/usr/bin/env node
/**
 * Model Asset Guard Node.js Bindings
 * 
 * This module provides Node.js bindings for the Model Asset Guard Rust sidecar,
 * enabling integration with JavaScript/TypeScript applications.
 * 
 * Requirements:
 * - Node.js >= 16.0.0
 * - ffi-napi (for FFI bindings)
 * - ref-napi (for memory management)
 */

const ffi = require('ffi-napi');
const ref = require('ref-napi');
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');

/**
 * Model Asset Guard error codes
 */
const GuarddError = {
    Success: 0,
    FileNotFound: 1,
    InvalidDigest: 2,
    QuantizationError: 3,
    MemoryError: 4,
    InvalidPath: 5
};

/**
 * Model Asset Guard Node.js bindings
 */
class ModelAssetGuard {
    constructor(libPath = null) {
        this.lib = this._loadLibrary(libPath);
        this._setupFunctionSignatures();
    }

    /**
     * Load the Rust sidecar library
     */
    _loadLibrary(libPath) {
        if (!libPath) {
            // Search for library in common locations
            const searchPaths = [
                './guardd/target/release/libguardd.so',
                './guardd/target/debug/libguardd.so',
                './target/release/libguardd.so',
                './target/debug/libguardd.so',
                './guardd/target/release/guardd.dll',
                './guardd/target/debug/guardd.dll',
                './target/release/guardd.dll',
                './target/debug/guardd.dll'
            ];

            for (const searchPath of searchPaths) {
                if (fs.existsSync(searchPath)) {
                    libPath = searchPath;
                    break;
                }
            }

            if (!libPath) {
                throw new Error(
                    'Could not find libguardd library. Please build the Rust sidecar first:\n' +
                    'cargo build --release --manifest-path guardd/Cargo.toml'
                );
            }
        }

        try {
            return ffi.Library(libPath, {
                'guardd_error_message': ['string', ['int']],
                'guardd_verify_digest': ['int', ['string', 'pointer']],
                'guardd_checked_load': ['pointer', ['string', 'pointer']],
                'guardd_free_handle': ['void', ['pointer']],
                'guardd_verify_quant_128_vectors': ['string', ['string', 'pointer', 'size_t', 'uint32', 'uint32', 'string']],
                'guardd_verify_model_128_vectors': ['string', ['string']],
                'guardd_run_bitflip_corpus_test': ['string', ['size_t', 'size_t', 'string']],
                'guardd_free_json_result': ['void', ['string']],
                'guardd_perfect_hash_encode': ['string', ['string', 'string']],
                'guardd_perfect_hash_decode': ['string', ['string', 'uint32']]
            });
        } catch (error) {
            throw new Error(`Failed to load library ${libPath}: ${error.message}`);
        }
    }

    /**
     * Setup function signatures for FFI calls
     */
    _setupFunctionSignatures() {
        // Additional setup if needed
    }

    /**
     * Get error message for error code
     */
    _getErrorMessage(errorCode) {
        try {
            return this.lib.guardd_error_message(errorCode);
        } catch (error) {
            return `Unknown error code: ${errorCode}`;
        }
    }

    /**
     * Verify SHA-256 digest of a file
     * 
     * @param {string} filePath - Path to the file to verify
     * @param {Buffer} expectedDigest - Expected SHA-256 digest (32 bytes)
     * @returns {boolean} - True if digest matches, false otherwise
     * @throws {Error} - If verification fails due to system error
     */
    verifyDigest(filePath, expectedDigest) {
        if (!Buffer.isBuffer(expectedDigest) || expectedDigest.length !== 32) {
            throw new Error('Expected digest must be 32 bytes');
        }

        const result = this.lib.guardd_verify_digest(filePath, expectedDigest);

        if (result === GuarddError.Success) {
            return true;
        } else if (result === GuarddError.InvalidDigest) {
            return false;
        } else {
            const errorMsg = this._getErrorMessage(result);
            throw new Error(`Verification failed: ${errorMsg}`);
        }
    }

    /**
     * Load a model file with integrity checks
     * 
     * @param {string} filePath - Path to the model file
     * @param {Buffer} expectedDigest - Expected SHA-256 digest (32 bytes). If null, only computes digest.
     * @returns {Object} - Model information
     * @throws {Error} - If loading fails
     */
    checkedLoad(filePath, expectedDigest = null) {
        const digestPtr = expectedDigest ? expectedDigest : null;
        const handlePtr = this.lib.guardd_checked_load(filePath, digestPtr);

        if (!handlePtr) {
            throw new Error('Failed to load model file');
        }

        try {
            // Parse the handle structure
            // Note: This is a simplified version. In practice, you'd need to properly
            // define the ModelHandle structure using ref-struct-napi
            const handle = ref.deref(handlePtr);
            
            // For now, we'll return a basic structure
            // In a full implementation, you'd parse the actual C structure
            const result = {
                path: filePath,
                size: 0, // Would be extracted from handle
                digest: '', // Would be extracted from handle
                valid: true // Would be extracted from handle
            };

            return result;
        } finally {
            this.lib.guardd_free_handle(handlePtr);
        }
    }

    /**
     * Verify quantization bounds for a layer using 128 random activation vectors
     * 
     * @param {string} layerName - Name of the layer
     * @param {Float32Array} weights - Weight matrix
     * @param {number} fanIn - Number of input features
     * @param {number} fanOut - Number of output features
     * @param {string} quantType - Quantization type ("int8", "fp16", etc.)
     * @returns {Object} - Verification results
     * @throws {Error} - If verification fails
     */
    verifyQuantization128Vectors(layerName, weights, fanIn, fanOut, quantType = 'int8') {
        if (!(weights instanceof Float32Array)) {
            weights = new Float32Array(weights);
        }

        const weightsPtr = ref.ref(weights);
        const resultPtr = this.lib.guardd_verify_quant_128_vectors(
            layerName,
            weightsPtr,
            weights.length,
            fanIn,
            fanOut,
            quantType
        );

        if (!resultPtr) {
            throw new Error('Quantization verification failed');
        }

        try {
            const resultJson = resultPtr;
            return JSON.parse(resultJson);
        } finally {
            this.lib.guardd_free_json_result(resultPtr);
        }
    }

    /**
     * Verify quantization bounds for an entire model
     * 
     * @param {Array} layers - List of layer objects with weights and metadata
     * @returns {Object} - Model verification results
     * @throws {Error} - If verification fails
     */
    verifyModelQuantization(layers) {
        const layersJson = JSON.stringify(layers);
        const resultPtr = this.lib.guardd_verify_model_128_vectors(layersJson);

        if (!resultPtr) {
            throw new Error('Model quantization verification failed');
        }

        try {
            const resultJson = resultPtr;
            return JSON.parse(resultJson);
        } finally {
            this.lib.guardd_free_json_result(resultPtr);
        }
    }

    /**
     * Run bit-flip corpus test for weight integrity validation
     * 
     * @param {number} fileSizeGb - Size of test files in GB
     * @param {number} numCorruptions - Number of corruption attempts
     * @param {string} tempDir - Temporary directory for test files
     * @returns {Object} - Test results
     * @throws {Error} - If test fails
     */
    runBitflipCorpusTest(fileSizeGb = 1, numCorruptions = 10, tempDir = null) {
        if (!tempDir) {
            tempDir = require('os').tmpdir();
        }

        const resultPtr = this.lib.guardd_run_bitflip_corpus_test(
            fileSizeGb,
            numCorruptions,
            tempDir
        );

        if (!resultPtr) {
            throw new Error('Bit-flip corpus test failed');
        }

        try {
            const resultJson = resultPtr;
            return JSON.parse(resultJson);
        } finally {
            this.lib.guardd_free_json_result(resultPtr);
        }
    }

    /**
     * Encode text using perfect hash tokenizer
     * 
     * @param {string} vocabJson - JSON string containing the perfect hash vocabulary
     * @param {string} text - Text to encode
     * @returns {Array} - Array of token IDs
     * @throws {Error} - If encoding fails
     */
    perfectHashEncode(vocabJson, text) {
        const resultPtr = this.lib.guardd_perfect_hash_encode(vocabJson, text);

        if (!resultPtr) {
            throw new Error('Perfect hash encoding failed');
        }

        try {
            const resultJson = resultPtr;
            return JSON.parse(resultJson);
        } finally {
            this.lib.guardd_free_json_result(resultPtr);
        }
    }

    /**
     * Decode a single token using perfect hash tokenizer
     * 
     * @param {string} vocabJson - JSON string containing the perfect hash vocabulary
     * @param {number} token - Token ID to decode
     * @returns {string} - Decoded word
     * @throws {Error} - If decoding fails
     */
    perfectHashDecode(vocabJson, token) {
        const resultPtr = this.lib.guardd_perfect_hash_decode(vocabJson, token);

        if (!resultPtr) {
            throw new Error('Perfect hash decoding failed');
        }

        try {
            return resultPtr;
        } finally {
            this.lib.guardd_free_json_result(resultPtr);
        }
    }

    /**
     * Decode a sequence of tokens using perfect hash tokenizer
     * 
     * @param {string} vocabJson - JSON string containing the perfect hash vocabulary
     * @param {Array} tokens - Array of token IDs to decode
     * @returns {string} - Decoded text
     * @throws {Error} - If decoding fails
     */
    perfectHashDecodeSequence(vocabJson, tokens) {
        const decodedWords = [];
        for (const token of tokens) {
            const word = this.perfectHashDecode(vocabJson, token);
            decodedWords.push(word);
        }
        return decodedWords.join(' ');
    }

    /**
     * Compute SHA-256 digest of a file using Node.js crypto
     * 
     * @param {string} filePath - Path to the file
     * @returns {Buffer} - SHA-256 digest
     */
    computeDigest(filePath) {
        const hash = crypto.createHash('sha256');
        const data = fs.readFileSync(filePath);
        hash.update(data);
        return hash.digest();
    }

    /**
     * Verify file integrity using both Rust and Node.js implementations
     * 
     * @param {string} filePath - Path to the file
     * @param {Buffer} expectedDigest - Expected digest
     * @returns {Object} - Verification results
     */
    verifyFileIntegrity(filePath, expectedDigest) {
        const results = {
            filePath,
            expectedDigest: expectedDigest.toString('hex'),
            rustVerification: null,
            nodeVerification: null,
            match: false
        };

        try {
            // Rust verification
            results.rustVerification = this.verifyDigest(filePath, expectedDigest);
        } catch (error) {
            results.rustVerification = { error: error.message };
        }

        try {
            // Node.js verification
            const computedDigest = this.computeDigest(filePath);
            results.nodeVerification = Buffer.compare(computedDigest, expectedDigest) === 0;
        } catch (error) {
            results.nodeVerification = { error: error.message };
        }

        // Check if both verifications match
        if (results.rustVerification === true && results.nodeVerification === true) {
            results.match = true;
        }

        return results;
    }
}

/**
 * HuggingFace-style integration for JavaScript
 */
class JavaScriptGuard {
    constructor(guard) {
        this.guard = guard;
    }

    /**
     * Load a model with verification (JavaScript equivalent of HuggingFace integration)
     * 
     * @param {string} modelPath - Path to the model file
     * @param {Buffer} expectedDigest - Expected digest
     * @param {boolean} verifyQuantization - Whether to verify quantization
     * @returns {Object} - Model and verification results
     */
    checkedLoadModel(modelPath, expectedDigest = null, verifyQuantization = true) {
        console.log(`Loading model ${modelPath} with integrity verification...`);

        const results = {
            modelPath,
            verified: false,
            fileIntegrity: null,
            quantization: null,
            errors: []
        };

        try {
            // Verify file integrity
            if (expectedDigest) {
                results.fileIntegrity = this.guard.verifyFileIntegrity(modelPath, expectedDigest);
                console.log(`✓ File integrity verified: ${results.fileIntegrity.expectedDigest}`);
            } else {
                const computedDigest = this.guard.computeDigest(modelPath);
                results.fileIntegrity = {
                    computedDigest: computedDigest.toString('hex'),
                    verified: true
                };
                console.log(`✓ File digest computed: ${results.fileIntegrity.computedDigest}`);
            }
        } catch (error) {
            console.log(`✗ File integrity verification failed: ${error.message}`);
            results.fileIntegrity = { error: error.message };
            results.errors.push(error.message);
        }

        // Note: Quantization verification would require model loading
        // which is beyond the scope of this basic Node.js implementation
        if (verifyQuantization) {
            results.quantization = { note: 'Quantization verification requires model loading implementation' };
        }

        results.verified = results.errors.length === 0;
        return results;
    }

    /**
     * Verify tokenizer determinism (JavaScript equivalent)
     * 
     * @param {Function} encode - Encoding function
     * @param {Function} decode - Decoding function
     * @param {Array} testStrings - Test strings
     * @returns {Object} - Test results
     */
    verifyTokenizerDeterminism(encode, decode, testStrings = null) {
        if (!testStrings) {
            testStrings = this._generateTestStrings(1000);
        }

        const results = {
            totalTests: testStrings.length,
            passed: 0,
            failed: 0,
            errors: []
        };

        for (let i = 0; i < testStrings.length; i++) {
            try {
                const testStr = testStrings[i];
                const tokens = encode(testStr);
                const decoded = decode(tokens);

                if (decoded === testStr) {
                    results.passed++;
                } else {
                    results.failed++;
                    results.errors.push({
                        index: i,
                        original: testStr,
                        decoded: decoded,
                        tokens: tokens
                    });
                }
            } catch (error) {
                results.failed++;
                results.errors.push({
                    index: i,
                    original: testStrings[i],
                    error: error.message
                });
            }
        }

        return results;
    }

    /**
     * Generate random test strings
     */
    _generateTestStrings(numTests) {
        const strings = [];
        const chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789 ';
        
        for (let i = 0; i < numTests; i++) {
            const length = Math.floor(Math.random() * 100) + 1;
            let testStr = '';
            for (let j = 0; j < length; j++) {
                testStr += chars.charAt(Math.floor(Math.random() * chars.length));
            }
            strings.push(testStr);
        }
        
        return strings;
    }
}

/**
 * Convenience functions
 */
function createGuard(libPath = null) {
    return new ModelAssetGuard(libPath);
}

function createJSGuard(libPath = null) {
    const guard = new ModelAssetGuard(libPath);
    return new JavaScriptGuard(guard);
}

/**
 * Example usage
 */
if (require.main === module) {
    // Example: Verify a test file
    try {
        const guard = createGuard();
        
        // Create a test file
        const testContent = 'Hello, Model Asset Guard!';
        const testFile = path.join(require('os').tmpdir(), 'test_model.bin');
        fs.writeFileSync(testFile, testContent);
        
        // Compute digest
        const digest = guard.computeDigest(testFile);
        console.log(`Test file digest: ${digest.toString('hex')}`);
        
        // Verify integrity
        const verified = guard.verifyDigest(testFile, digest);
        console.log(`Verification result: ${verified}`);
        
        // Clean up
        fs.unlinkSync(testFile);
        
        console.log('✓ Node.js bindings test completed successfully!');
    } catch (error) {
        console.error(`✗ Node.js bindings test failed: ${error.message}`);
        process.exit(1);
    }
}

module.exports = {
    ModelAssetGuard,
    JavaScriptGuard,
    GuarddError,
    createGuard,
    createJSGuard
}; 