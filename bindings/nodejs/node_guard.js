#!/usr/bin/env node
/**
 * Model Asset Guard Node.js bindings (koffi over guardd C ABI).
 *
 * Build the sidecar first:
 *   cargo build --release --manifest-path ../../src/rust/guardd/Cargo.toml
 *
 * Optional override:
 *   set GUARDD_LIB=/absolute/path/to/guardd.dll|libguardd.so|libguardd.dylib
 */

'use strict';

const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const os = require('os');

let koffi;
try {
  koffi = require('koffi');
} catch (err) {
  throw new Error(
    'Missing dependency `koffi`. From bindings/nodejs run: npm install\n' +
      `Underlying error: ${err.message}`
  );
}

const GuarddError = {
  Success: 0,
  FileNotFound: 1,
  InvalidDigest: 2,
  QuantizationError: 3,
  MemoryError: 4,
  InvalidPath: 5,
};

function candidateLibPaths(explicit) {
  if (explicit) return [explicit];
  if (process.env.GUARDD_LIB) return [process.env.GUARDD_LIB];

  const repoRoot = path.resolve(__dirname, '..', '..');
  const names = [
    'libguardd.so',
    'guardd.dll',
    'libguardd.dylib',
    // Windows often emits guardd.dll; MSVC may also emit guardd.dll only.
  ];
  const roots = [
    // Staged native packaging path (scripts/stage_guardd_native.sh)
    path.join(__dirname, 'native'),
    path.join(repoRoot, 'dist', 'native'),
    path.join(repoRoot, 'bindings', 'python', 'native'),
    path.join(repoRoot, 'src/rust/guardd/target/release'),
    path.join(repoRoot, 'src/rust/guardd/target/debug'),
    path.join(repoRoot, 'src/rust/guardd/target/x86_64-unknown-linux-gnu/release'),
    path.join(repoRoot, 'src/rust/guardd/target/x86_64-unknown-linux-gnu/debug'),
    path.join(repoRoot, 'src/rust/guardd/target/aarch64-apple-darwin/release'),
    path.join(repoRoot, 'src/rust/guardd/target/aarch64-apple-darwin/debug'),
    __dirname,
    process.cwd(),
  ];
  const out = [];
  for (const root of roots) {
    for (const name of names) {
      out.push(path.join(root, name));
    }
  }
  return out;
}

function resolveLibPath(libPath) {
  for (const candidate of candidateLibPaths(libPath)) {
    if (candidate && fs.existsSync(candidate)) return path.resolve(candidate);
  }
  throw new Error(
    'Could not find libguardd / guardd shared library.\n' +
      'Build + stage it with:\n' +
      '  cargo build --release --manifest-path src/rust/guardd/Cargo.toml\n' +
      '  bash scripts/stage_guardd_native.sh\n' +
      'Or set GUARDD_LIB to the absolute library path.\n' +
      'See docs/native-packaging.md (CI uploads platform artifacts; npm does not ship binaries).'
  );
}

class ModelAssetGuard {
  constructor(libPath = null) {
    this.libPath = resolveLibPath(libPath);
    this.lib = koffi.load(this.libPath);
    this._bind();
  }

  _bind() {
    this._error_message = this.lib.func('guardd_error_message', 'str', ['int']);
    this._verify_digest = this.lib.func('guardd_verify_digest', 'int', [
      'str',
      'void *',
    ]);
    this._checked_load = this.lib.func('guardd_checked_load', 'void *', [
      'str',
      'void *',
    ]);
    this._free_handle = this.lib.func('guardd_free_handle', 'void', ['void *']);
    this._verify_quant_128 = this.lib.func(
      'guardd_verify_quant_128_vectors',
      'void *',
      ['str', 'void *', 'size_t', 'uint32', 'uint32', 'str']
    );
    this._verify_model_128 = this.lib.func(
      'guardd_verify_model_128_vectors',
      'void *',
      ['str']
    );
    this._bitflip = this.lib.func('guardd_run_bitflip_corpus_test', 'void *', [
      'size_t',
      'size_t',
      'str',
    ]);
    this._free_json = this.lib.func('guardd_free_json_result', 'void', ['void *']);
    this._ph_encode = this.lib.func('guardd_perfect_hash_encode', 'void *', [
      'str',
      'str',
    ]);
    this._ph_decode = this.lib.func('guardd_perfect_hash_decode', 'void *', [
      'str',
      'uint32',
    ]);
    this._sp_encode = this.lib.func('guardd_sp_encode', 'void *', ['str', 'str']);
    this._sp_encode_opts = this.lib.func('guardd_sp_encode_with_options', 'void *', [
      'str',
      'str',
      'str',
    ]);
    this._sp_nbest = this.lib.func('guardd_sp_nbest_encode', 'void *', ['str', 'str', 'str']);
    this._sp_sample = this.lib.func('guardd_sp_sample_encode', 'void *', ['str', 'str', 'str']);
    this._sp_sample_score = this.lib.func('guardd_sp_sample_encode_and_score', 'void *', [
      'str',
      'str',
      'str',
    ]);
    this._sp_decode = this.lib.func('guardd_sp_decode', 'void *', ['str', 'str']);
    this._sp_decode_opts = this.lib.func('guardd_sp_decode_with_options', 'void *', [
      'str',
      'str',
      'str',
    ]);
  }

  _readCString(ptr) {
    if (!ptr) throw new Error('Native call returned null pointer');
    return koffi.decode(ptr, 'str');
  }

  _getErrorMessage(errorCode) {
    try {
      return this._error_message(errorCode);
    } catch (_) {
      return `Unknown error code: ${errorCode}`;
    }
  }

  _digestPtr(expectedDigest) {
    if (expectedDigest == null) return null;
    if (!Buffer.isBuffer(expectedDigest) || expectedDigest.length !== 32) {
      throw new Error('Expected digest must be 32 bytes');
    }
    return expectedDigest;
  }

  verifyDigest(filePath, expectedDigest) {
    const dig = this._digestPtr(expectedDigest);
    const result = this._verify_digest(filePath, dig);
    if (result === GuarddError.Success) return true;
    if (result === GuarddError.InvalidDigest) return false;
    throw new Error(`Verification failed: ${this._getErrorMessage(result)}`);
  }

  /**
   * Load with integrity check. `valid` reflects a real digest comparison when
   * `expectedDigest` is supplied (never hard-coded success).
   */
  checkedLoad(filePath, expectedDigest = null) {
    const dig = this._digestPtr(expectedDigest);
    const handlePtr = this._checked_load(filePath, dig);
    if (!handlePtr) {
      throw new Error('Failed to load model file (guardd_checked_load returned null)');
    }
    try {
      const digestBuf = this.computeDigest(filePath);
      const valid = expectedDigest ? digestBuf.equals(expectedDigest) : true;
      return {
        path: filePath,
        size: fs.statSync(filePath).size,
        digest: digestBuf.toString('hex'),
        valid,
        libPath: this.libPath,
      };
    } finally {
      this._free_handle(handlePtr);
    }
  }

  verifyQuantization128Vectors(layerName, weights, fanIn, fanOut, quantType = 'int8') {
    const arr =
      weights instanceof Float32Array ? weights : new Float32Array(weights);
    const resultPtr = this._verify_quant_128(
      layerName,
      arr,
      arr.length,
      fanIn,
      fanOut,
      quantType
    );
    if (!resultPtr) throw new Error('Quantization verification failed');
    try {
      return JSON.parse(this._readCString(resultPtr));
    } finally {
      this._free_json(resultPtr);
    }
  }

  verifyModelQuantization(layers) {
    const resultPtr = this._verify_model_128(JSON.stringify(layers));
    if (!resultPtr) throw new Error('Model quantization verification failed');
    try {
      return JSON.parse(this._readCString(resultPtr));
    } finally {
      this._free_json(resultPtr);
    }
  }

  runBitflipCorpusTest(fileSizeGb = 0, numCorruptions = 10, tempDir = null) {
    const dir = tempDir || os.tmpdir();
    const resultPtr = this._bitflip(fileSizeGb, numCorruptions, dir);
    if (!resultPtr) throw new Error('Bit-flip corpus test failed');
    try {
      return JSON.parse(this._readCString(resultPtr));
    } finally {
      this._free_json(resultPtr);
    }
  }

  perfectHashEncode(vocabJson, text) {
    const resultPtr = this._ph_encode(vocabJson, text);
    if (!resultPtr) throw new Error('Perfect hash encoding failed');
    try {
      return JSON.parse(this._readCString(resultPtr));
    } finally {
      this._free_json(resultPtr);
    }
  }

  perfectHashDecode(vocabJson, token) {
    const resultPtr = this._ph_decode(vocabJson, token >>> 0);
    if (!resultPtr) throw new Error('Perfect hash decoding failed');
    try {
      return this._readCString(resultPtr);
    } finally {
      this._free_json(resultPtr);
    }
  }

  perfectHashDecodeSequence(vocabJson, tokens) {
    return tokens.map((t) => this.perfectHashDecode(vocabJson, t)).join(' ');
  }

  /**
   * SentencePiece encode (CHAR / BPE / UNIGRAM when the Rust support gate passes).
   * See docs/sentencepiece-bpe.md — not full SP parity.
   * @param {string} modelPath
   * @param {string} text
   * @param {{allowSilentUnk?: boolean, forcedPieces?: string[], wordOfficialSplit?: boolean, mergeConsecutiveUnk?: boolean, reverse?: boolean, addBos?: boolean, addEos?: boolean}} [options]
   */
  spEncode(modelPath, text, options = {}) {
    const optsJson = JSON.stringify({
      allow_silent_unk: Boolean(options.allowSilentUnk),
      forced_pieces: Array.isArray(options.forcedPieces) ? options.forcedPieces : [],
      word_official_split: Boolean(options.wordOfficialSplit),
      merge_consecutive_unk:
        options.mergeConsecutiveUnk === undefined ? true : Boolean(options.mergeConsecutiveUnk),
      reverse: Boolean(options.reverse),
      add_bos: Boolean(options.addBos),
      add_eos: Boolean(options.addEos),
    });
    const resultPtr = this._sp_encode_opts(modelPath, text, optsJson);
    if (!resultPtr) throw new Error('spEncode returned null');
    try {
      const payload = JSON.parse(this._readCString(resultPtr));
      if (!payload.ok) {
        throw new Error(payload.error || 'spEncode failed');
      }
      return payload.tokens;
    } finally {
      this._free_json(resultPtr);
    }
  }

  /**
   * UNIGRAM lattice n-best. Returns `[{tokens, score}, ...]`.
   * @param {{allowSilentUnk?: boolean, forcedPieces?: string[], mergeConsecutiveUnk?: boolean, reverse?: boolean, addBos?: boolean, addEos?: boolean, nbestStockCompat?: boolean}} [options]
   */
  spNbestEncode(modelPath, text, nbestSize = 1, options = {}) {
    const optsJson = JSON.stringify({
      allow_silent_unk: Boolean(options.allowSilentUnk),
      forced_pieces: Array.isArray(options.forcedPieces) ? options.forcedPieces : [],
      merge_consecutive_unk:
        options.mergeConsecutiveUnk === undefined ? true : Boolean(options.mergeConsecutiveUnk),
      reverse: Boolean(options.reverse),
      add_bos: Boolean(options.addBos),
      add_eos: Boolean(options.addEos),
      nbest_stock_compat: Boolean(options.nbestStockCompat),
      nbest_size: Number(nbestSize) || 1,
    });
    const resultPtr = this._sp_nbest(modelPath, text, optsJson);
    if (!resultPtr) throw new Error('spNbestEncode returned null');
    try {
      const payload = JSON.parse(this._readCString(resultPtr));
      if (!payload.ok) {
        throw new Error(payload.error || 'spNbestEncode failed');
      }
      return payload.nbests;
    } finally {
      this._free_json(resultPtr);
    }
  }

  /**
   * UNIGRAM sample-encode or BPE-dropout (alpha = dropout probability).
   * @param {{nbestSize?: number, alpha?: number, seed?: number, allowSilentUnk?: boolean, forcedPieces?: string[], mergeConsecutiveUnk?: boolean, reverse?: boolean, addBos?: boolean, addEos?: boolean}} [options]
   */
  spSampleEncode(modelPath, text, options = {}) {
    const body = {
      allow_silent_unk: Boolean(options.allowSilentUnk),
      forced_pieces: Array.isArray(options.forcedPieces) ? options.forcedPieces : [],
      merge_consecutive_unk:
        options.mergeConsecutiveUnk === undefined ? true : Boolean(options.mergeConsecutiveUnk),
      reverse: Boolean(options.reverse),
      add_bos: Boolean(options.addBos),
      add_eos: Boolean(options.addEos),
      nbest_size: options.nbestSize === undefined ? -1 : Number(options.nbestSize),
      alpha: options.alpha === undefined ? 0.0 : Number(options.alpha),
    };
    if (options.seed !== undefined && options.seed !== null) {
      body.seed = Number(options.seed);
    }
    const resultPtr = this._sp_sample(modelPath, text, JSON.stringify(body));
    if (!resultPtr) throw new Error('spSampleEncode returned null');
    try {
      const payload = JSON.parse(this._readCString(resultPtr));
      if (!payload.ok) {
        throw new Error(payload.error || 'spSampleEncode failed');
      }
      return payload.tokens;
    } finally {
      this._free_json(resultPtr);
    }
  }

  /**
   * UNIGRAM SampleEncodeAndScore (WR or WOR / Gumbel inclusion probs).
   * @param {{numSamples?: number, alpha?: number, wor?: boolean, includeBest?: boolean, seed?: number, allowSilentUnk?: boolean, forcedPieces?: string[], mergeConsecutiveUnk?: boolean, reverse?: boolean, addBos?: boolean, addEos?: boolean}} [options]
   */
  spSampleEncodeAndScore(modelPath, text, options = {}) {
    const body = {
      allow_silent_unk: Boolean(options.allowSilentUnk),
      forced_pieces: Array.isArray(options.forcedPieces) ? options.forcedPieces : [],
      merge_consecutive_unk:
        options.mergeConsecutiveUnk === undefined ? true : Boolean(options.mergeConsecutiveUnk),
      reverse: Boolean(options.reverse),
      add_bos: Boolean(options.addBos),
      add_eos: Boolean(options.addEos),
      num_samples: options.numSamples === undefined ? 1 : Number(options.numSamples),
      alpha: options.alpha === undefined ? 0.0 : Number(options.alpha),
      wor: Boolean(options.wor),
      include_best: Boolean(options.includeBest),
    };
    if (options.seed !== undefined && options.seed !== null) {
      body.seed = Number(options.seed);
    }
    const resultPtr = this._sp_sample_score(modelPath, text, JSON.stringify(body));
    if (!resultPtr) throw new Error('spSampleEncodeAndScore returned null');
    try {
      const payload = JSON.parse(this._readCString(resultPtr));
      if (!payload.ok) {
        throw new Error(payload.error || 'spSampleEncodeAndScore failed');
      }
      return payload.samples;
    } finally {
      this._free_json(resultPtr);
    }
  }

  /**
   * SentencePiece decode: ▁/BYTE/unk_surface + optional denormalizer charsmap.
   * @param {string} modelPath
   * @param {number[]} tokens
   * @param {{reverse?: boolean}} [options]
   */
  spDecode(modelPath, tokens, options = {}) {
    const optsJson = JSON.stringify({ reverse: Boolean(options.reverse) });
    const resultPtr = this._sp_decode_opts(modelPath, JSON.stringify(tokens), optsJson);
    if (!resultPtr) throw new Error('spDecode returned null');
    try {
      const payload = JSON.parse(this._readCString(resultPtr));
      if (!payload.ok) {
        throw new Error(payload.error || 'spDecode failed');
      }
      return payload.text;
    } finally {
      this._free_json(resultPtr);
    }
  }

  computeDigest(filePath) {
    const hash = crypto.createHash('sha256');
    hash.update(fs.readFileSync(filePath));
    return hash.digest();
  }

  verifyFileIntegrity(filePath, expectedDigest) {
    const results = {
      filePath,
      expectedDigest: expectedDigest.toString('hex'),
      rustVerification: null,
      nodeVerification: null,
      match: false,
    };
    try {
      results.rustVerification = this.verifyDigest(filePath, expectedDigest);
    } catch (error) {
      results.rustVerification = { error: error.message };
    }
    try {
      results.nodeVerification =
        Buffer.compare(this.computeDigest(filePath), expectedDigest) === 0;
    } catch (error) {
      results.nodeVerification = { error: error.message };
    }
    results.match =
      results.rustVerification === true && results.nodeVerification === true;
    return results;
  }
}

function createGuard(libPath = null) {
  return new ModelAssetGuard(libPath);
}

if (require.main === module) {
  try {
    const guard = createGuard();
    const testFile = path.join(os.tmpdir(), `mag-node-${process.pid}.bin`);
    fs.writeFileSync(testFile, 'Hello, Model Asset Guard!');
    const digest = guard.computeDigest(testFile);
    const verified = guard.verifyDigest(testFile, digest);
    const loaded = guard.checkedLoad(testFile, digest);
    fs.unlinkSync(testFile);
    if (!verified || !loaded.valid) {
      console.error('Node bindings self-check failed: digest mismatch');
      process.exit(1);
    }
    console.log(`Node bindings OK (lib=${guard.libPath})`);
  } catch (error) {
    console.error(`Node bindings test failed: ${error.message}`);
    process.exit(1);
  }
}

module.exports = {
  ModelAssetGuard,
  GuarddError,
  createGuard,
  resolveLibPath,
  candidateLibPaths,
};
