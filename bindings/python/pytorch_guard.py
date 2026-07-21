#!/usr/bin/env python3
"""
Model Asset Guard Python Bindings

This module provides Python bindings for the Model Asset Guard Rust sidecar,
enabling integration with HuggingFace transformers and PyTorch models.

Requirements:
- ctypes (built-in)
- Optional: numpy (single-layer quant helper), transformers/torch (HF helpers)
"""

from __future__ import annotations

import ctypes
import ctypes.util
import json
import os
import tempfile
import warnings
from typing import Any

try:
    import numpy as np

    NUMPY_AVAILABLE = True
except ImportError:
    np = None  # type: ignore[assignment]
    NUMPY_AVAILABLE = False

try:
    from transformers import (
        AutoModel,
        AutoTokenizer,
        PreTrainedModel,
        PreTrainedTokenizer,
    )

    TRANSFORMERS_AVAILABLE = True
except ImportError:
    TRANSFORMERS_AVAILABLE = False
    AutoModel = Any  # type: ignore[misc,assignment]
    AutoTokenizer = Any  # type: ignore[misc,assignment]
    PreTrainedModel = Any  # type: ignore[misc,assignment]
    PreTrainedTokenizer = Any  # type: ignore[misc,assignment]
    warnings.warn(
        "transformers or torch not available. HuggingFace integration disabled.",
        stacklevel=2,
    )


class GuarddError(Exception):
    """Exception raised by Model Asset Guard operations"""

    pass


class ModelAssetGuard:
    """
    Python bindings for Model Asset Guard Rust sidecar.

    This class provides high-level Python interface to the Model Asset Guard
    verification functions, including integration with HuggingFace transformers.
    """

    def __init__(self, lib_path: str | None = None):
        """
        Initialize Model Asset Guard bindings.

        Args:
            lib_path: Path to libguardd.so/dll. If None, will search in common locations.
        """
        self.lib = self._load_library(lib_path)
        self._setup_function_signatures()

    def _load_library(self, lib_path: str | None) -> ctypes.CDLL:
        """Load the Rust sidecar library"""
        _pkg_dir = os.path.dirname(os.path.abspath(__file__))
        _repo_root = os.path.abspath(os.path.join(_pkg_dir, "..", ".."))
        if lib_path is None:
            # Search for library in common locations; prefer newest mtime when
            # both debug and release artifacts exist (avoids stale release DLL).
            search_paths = [
                os.environ.get("GUARDD_LIB"),
                # Staged native dir (see scripts/stage_guardd_native.sh / docs/native-packaging.md)
                os.path.join(_pkg_dir, "native", "libguardd.so"),
                os.path.join(_pkg_dir, "native", "libguardd.dylib"),
                os.path.join(_pkg_dir, "native", "guardd.dll"),
                os.path.join(_repo_root, "dist", "native", "libguardd.so"),
                os.path.join(_repo_root, "dist", "native", "libguardd.dylib"),
                os.path.join(_repo_root, "dist", "native", "guardd.dll"),
                os.path.join(_repo_root, "bindings", "nodejs", "native", "libguardd.so"),
                os.path.join(_repo_root, "bindings", "nodejs", "native", "libguardd.dylib"),
                os.path.join(_repo_root, "bindings", "nodejs", "native", "guardd.dll"),
                "./src/rust/guardd/target/release/libguardd.so",
                "./src/rust/guardd/target/debug/libguardd.so",
                "./src/rust/guardd/target/release/libguardd.dylib",
                "./src/rust/guardd/target/debug/libguardd.dylib",
                "./target/release/libguardd.so",
                "./target/debug/libguardd.so",
                "./target/release/libguardd.dylib",
                "./target/debug/libguardd.dylib",
                "./src/rust/guardd/target/release/guardd.dll",
                "./src/rust/guardd/target/debug/guardd.dll",
                "./target/release/guardd.dll",
                "./target/debug/guardd.dll",
                ctypes.util.find_library("guardd"),
            ]

            existing = [p for p in search_paths if p and os.path.exists(p)]
            if existing:
                # GUARDD_LIB (first entry) wins if set and present.
                if search_paths[0] and os.path.exists(search_paths[0]):
                    lib_path = search_paths[0]
                else:
                    lib_path = max(existing, key=lambda p: os.path.getmtime(p))
            else:
                raise FileNotFoundError(
                    "Could not find libguardd library. Please build the Rust sidecar first:\n"
                    "cargo build --release --manifest-path src/rust/guardd/Cargo.toml"
                )

        try:
            lib = ctypes.CDLL(lib_path)
            return lib
        except Exception as e:
            raise RuntimeError(f"Failed to load library {lib_path}: {e}") from e

    def _setup_function_signatures(self):
        """Setup function signatures for FFI calls"""
        # Error codes
        self.lib.guardd_error_message.argtypes = [ctypes.c_int]
        self.lib.guardd_error_message.restype = ctypes.c_char_p

        # Digest verification
        self.lib.guardd_verify_digest.argtypes = [
            ctypes.c_char_p,  # path
            ctypes.POINTER(ctypes.c_uint8),  # expected_digest
        ]
        self.lib.guardd_verify_digest.restype = ctypes.c_int

        # Model loading
        self.lib.guardd_checked_load.argtypes = [
            ctypes.c_char_p,  # path
            ctypes.POINTER(ctypes.c_uint8),  # expected_digest
        ]
        self.lib.guardd_checked_load.restype = ctypes.c_void_p

        # Model handle structure
        class ModelHandle(ctypes.Structure):
            _fields_ = [
                ("path", ctypes.c_char_p),
                ("size", ctypes.c_uint64),
                ("digest", ctypes.c_uint8 * 32),
                ("valid", ctypes.c_bool),
            ]

        self.ModelHandle = ModelHandle

        # Free functions
        self.lib.guardd_free_handle.argtypes = [ctypes.c_void_p]
        self.lib.guardd_free_handle.restype = None

        # Owned C strings must use c_void_p so ctypes does not steal/free the pointer.
        self.lib.guardd_free_json_result.argtypes = [ctypes.c_void_p]
        self.lib.guardd_free_json_result.restype = None

        # Quantization verification
        self.lib.guardd_verify_quant_128_vectors.argtypes = [
            ctypes.c_char_p,  # layer_name
            ctypes.POINTER(ctypes.c_float),  # weights
            ctypes.c_size_t,  # weights_len
            ctypes.c_uint32,  # fan_in
            ctypes.c_uint32,  # fan_out
            ctypes.c_char_p,  # quant_type
        ]
        self.lib.guardd_verify_quant_128_vectors.restype = ctypes.c_void_p

        # Model verification
        self.lib.guardd_verify_model_128_vectors.argtypes = [
            ctypes.c_char_p,  # layers_json
        ]
        self.lib.guardd_verify_model_128_vectors.restype = ctypes.c_void_p

        # Bit-flip corpus testing
        self.lib.guardd_run_bitflip_corpus_test.argtypes = [
            ctypes.c_size_t,  # file_size_gb
            ctypes.c_size_t,  # num_corruptions
            ctypes.c_char_p,  # temp_dir
        ]
        self.lib.guardd_run_bitflip_corpus_test.restype = ctypes.c_void_p

        # Perfect hash tokenizer functions
        self.lib.guardd_perfect_hash_encode.argtypes = [
            ctypes.c_char_p,  # vocab_json
            ctypes.c_char_p,  # text
        ]
        self.lib.guardd_perfect_hash_encode.restype = ctypes.c_void_p

        self.lib.guardd_perfect_hash_decode.argtypes = [
            ctypes.c_char_p,  # vocab_json
            ctypes.c_uint32,  # token
        ]
        self.lib.guardd_perfect_hash_decode.restype = ctypes.c_void_p

        # SentencePiece encode/decode (CHAR / BPE / UNIGRAM when support gate passes)
        self.lib.guardd_sp_encode.argtypes = [
            ctypes.c_char_p,  # model_path
            ctypes.c_char_p,  # text
        ]
        self.lib.guardd_sp_encode.restype = ctypes.c_void_p

        self.lib.guardd_sp_encode_with_options.argtypes = [
            ctypes.c_char_p,  # model_path
            ctypes.c_char_p,  # text
            ctypes.c_char_p,  # options_json
        ]
        self.lib.guardd_sp_encode_with_options.restype = ctypes.c_void_p

        self.lib.guardd_sp_nbest_encode.argtypes = [
            ctypes.c_char_p,
            ctypes.c_char_p,
            ctypes.c_char_p,
        ]
        self.lib.guardd_sp_nbest_encode.restype = ctypes.c_void_p

        self.lib.guardd_sp_sample_encode.argtypes = [
            ctypes.c_char_p,
            ctypes.c_char_p,
            ctypes.c_char_p,
        ]
        self.lib.guardd_sp_sample_encode.restype = ctypes.c_void_p

        self.lib.guardd_sp_sample_encode_and_score.argtypes = [
            ctypes.c_char_p,
            ctypes.c_char_p,
            ctypes.c_char_p,
        ]
        self.lib.guardd_sp_sample_encode_and_score.restype = ctypes.c_void_p

        self.lib.guardd_sp_decode.argtypes = [
            ctypes.c_char_p,  # model_path
            ctypes.c_char_p,  # tokens_json
        ]
        self.lib.guardd_sp_decode.restype = ctypes.c_void_p

        self.lib.guardd_sp_decode_with_options.argtypes = [
            ctypes.c_char_p,  # model_path
            ctypes.c_char_p,  # tokens_json
            ctypes.c_char_p,  # options_json
        ]
        self.lib.guardd_sp_decode_with_options.restype = ctypes.c_void_p

    def _consume_owned_cstring(self, result_ptr: int | None) -> str:
        """Read an owned NUL-terminated C string and free it via guardd."""
        if not result_ptr:
            raise GuarddError("Native call returned null pointer")
        try:
            raw = ctypes.cast(result_ptr, ctypes.c_char_p).value
            if raw is None:
                raise GuarddError("Native call returned empty string pointer")
            return raw.decode("utf-8")
        finally:
            self.lib.guardd_free_json_result(result_ptr)

    def _get_error_message(self, error_code: int) -> str:
        """Get error message for error code"""
        error_ptr = self.lib.guardd_error_message(error_code)
        if error_ptr:
            return error_ptr.decode("utf-8")
        return f"Unknown error code: {error_code}"

    def verify_digest(self, file_path: str, expected_digest: bytes) -> bool:
        """
        Verify SHA-256 digest of a file.

        Args:
            file_path: Path to the file to verify
            expected_digest: Expected SHA-256 digest (32 bytes)

        Returns:
            True if digest matches, False otherwise

        Raises:
            GuarddError: If verification fails due to system error
        """
        if len(expected_digest) != 32:
            raise ValueError("Expected digest must be 32 bytes")

        result = self.lib.guardd_verify_digest(
            file_path.encode("utf-8"),
            (ctypes.c_uint8 * 32).from_buffer_copy(expected_digest),
        )

        if result == 0:  # Success
            return True
        elif result == 2:  # InvalidDigest
            return False
        else:
            error_msg = self._get_error_message(result)
            raise GuarddError(f"Verification failed: {error_msg}")

    def checked_load(
        self, file_path: str, expected_digest: bytes | None = None
    ) -> dict[str, Any]:
        """
        Load a model file with integrity checks.

        Args:
            file_path: Path to the model file
            expected_digest: Expected SHA-256 digest (32 bytes). If None, only computes digest.

        Returns:
            Dictionary with model information:
            - path: File path
            - size: File size in bytes
            - digest: Computed SHA-256 digest (hex string)
            - valid: Whether digest matches expected (if provided)

        Raises:
            GuarddError: If loading fails
        """
        digest_ptr = None
        if expected_digest is not None:
            if len(expected_digest) != 32:
                raise ValueError("Expected digest must be 32 bytes")
            digest_ptr = (ctypes.c_uint8 * 32).from_buffer_copy(expected_digest)

        handle_ptr = self.lib.guardd_checked_load(file_path.encode("utf-8"), digest_ptr)

        if not handle_ptr:
            raise GuarddError("Failed to load model file")

        try:
            handle = ctypes.cast(handle_ptr, ctypes.POINTER(self.ModelHandle)).contents

            # Convert digest to hex string
            digest_bytes = bytes(handle.digest)
            digest_hex = digest_bytes.hex()

            result = {
                "path": file_path,
                "size": handle.size,
                "digest": digest_hex,
                "valid": handle.valid if expected_digest is not None else True,
            }

            return result
        finally:
            self.lib.guardd_free_handle(handle_ptr)

    def verify_quantization_128_vectors(
        self,
        layer_name: str,
        weights: list[float] | np.ndarray,
        fan_in: int,
        fan_out: int,
        quant_type: str = "int8",
    ) -> dict[str, Any]:
        """
        Verify quantization bounds for a layer using 128 random activation vectors.

        Args:
            layer_name: Name of the layer
            weights: Weight matrix as list or numpy array
            fan_in: Number of input features
            fan_out: Number of output features
            quant_type: Quantization type ("int8", "fp16", etc.)

        Returns:
            Dictionary with verification results

        Raises:
            GuarddError: If verification fails
        """
        if not NUMPY_AVAILABLE:
            raise GuarddError(
                "numpy is required for verify_quantization_128_vectors; "
                "use verify_model_quantization with plain lists instead"
            )

        if not isinstance(weights, np.ndarray):
            weights = np.array(weights, dtype=np.float32)

        if weights.dtype != np.float32:
            weights = weights.astype(np.float32)

        weights_ptr = weights.ctypes.data_as(ctypes.POINTER(ctypes.c_float))

        result_ptr = self.lib.guardd_verify_quant_128_vectors(
            layer_name.encode("utf-8"),
            weights_ptr,
            len(weights),
            fan_in,
            fan_out,
            quant_type.encode("utf-8"),
        )
        return json.loads(self._consume_owned_cstring(result_ptr))

    def verify_model_quantization(self, layers: list[dict[str, Any]]) -> dict[str, Any]:
        """
        Verify quantization bounds for an entire model.

        Args:
            layers: List of layer dictionaries with weights and metadata

        Returns:
            Dictionary with model verification results

        Raises:
            GuarddError: If verification fails
        """
        layers_json = json.dumps(layers)
        result_ptr = self.lib.guardd_verify_model_128_vectors(
            layers_json.encode("utf-8")
        )
        return json.loads(self._consume_owned_cstring(result_ptr))

    def run_bitflip_corpus_test(
        self,
        file_size_gb: int = 0,
        num_corruptions: int = 10,
        temp_dir: str | None = None,
    ) -> dict[str, Any]:
        """
        Run bit-flip corpus test for weight integrity validation.

        Args:
            file_size_gb: Size in GB. ``0`` means 1 MiB (CI/default). Values
                above the Rust default cap (64 MiB) require
                ``GUARDD_BITFLIP_ALLOW_LARGE=1`` in the environment.
            num_corruptions: Number of corruption attempts
            temp_dir: Temporary directory for test files

        Returns:
            Dictionary with test results

        Raises:
            GuarddError: If test fails
        """
        if temp_dir is None:
            temp_dir = tempfile.gettempdir()

        result_ptr = self.lib.guardd_run_bitflip_corpus_test(
            file_size_gb, num_corruptions, temp_dir.encode("utf-8")
        )
        return json.loads(self._consume_owned_cstring(result_ptr))

    def perfect_hash_encode(self, vocab_json: str, text: str) -> list[int]:
        """
        Encode text using perfect hash tokenizer.

        Args:
            vocab_json: JSON string containing the perfect hash vocabulary
            text: Text to encode

        Returns:
            List of token IDs

        Raises:
            GuarddError: If encoding fails
        """
        result_ptr = self.lib.guardd_perfect_hash_encode(
            vocab_json.encode("utf-8"), text.encode("utf-8")
        )
        return json.loads(self._consume_owned_cstring(result_ptr))

    def perfect_hash_decode(self, vocab_json: str, token: int) -> str:
        """
        Decode a single token using perfect hash tokenizer.

        Args:
            vocab_json: JSON string containing the perfect hash vocabulary
            token: Token ID to decode

        Returns:
            Decoded word

        Raises:
            GuarddError: If decoding fails
        """
        result_ptr = self.lib.guardd_perfect_hash_decode(
            vocab_json.encode("utf-8"), token
        )
        return self._consume_owned_cstring(result_ptr)

    def sp_encode(
        self,
        model_path: str,
        text: str,
        *,
        allow_silent_unk: bool = False,
        forced_pieces: list[str] | None = None,
        word_official_split: bool = False,
        merge_consecutive_unk: bool = True,
        reverse: bool = False,
        add_bos: bool = False,
        add_eos: bool = False,
    ) -> list[int]:
        """
        Encode text with a SentencePiece ``.model`` (CHAR / BPE / UNIGRAM / WORD).

        Uses the Rust support gate (SP normalize + optional charsmap /
        byte_fallback / USER_DEFINED PrefixMatcher / treat_whitespace_as_suffix;
        incomplete OOV refuse unless ``allow_silent_unk``). See
        ``docs/sentencepiece-bpe.md``.

        Args:
            model_path: Path to a SentencePiece ModelProto ``.model`` file
            text: Text to encode
            allow_silent_unk: When True, emit UNKNOWN for OOV without
                ``byte_fallback`` (stock SP). Default False (fail-closed).
            forced_pieces: Extra PrefixMatcher pieces; each must already be in
                the model vocab (fail-closed otherwise).
            word_official_split: When True, WORD ``SplitIntoWords`` ignores the
                trainer ``treat_whitespace_as_suffix`` flag (stock
                ``word_model.cc``). Default False honors the trainer flag.
            merge_consecutive_unk: When True (default), merge consecutive
                UNKNOWN ids like stock ``SentencePieceProcessor`` (no-op when
                ``byte_fallback`` is enabled). Set False for piece-level ids.
            reverse: Reverse piece ids after merge and before bos/eos (stock
                documented ExtraOptions order ``reverse:bos:eos``).
            add_bos: Prepend CONTROL bos id after merge (stock ExtraOptions
                ``bos``). Refuses if the model has no CONTROL bos piece.
            add_eos: Append CONTROL eos id after merge (stock ExtraOptions
                ``eos``). Refuses if the model has no CONTROL eos piece.

        Returns:
            List of token IDs (piece indices)

        Raises:
            GuarddError: If loading or encoding fails
        """
        options = json.dumps(
            {
                "allow_silent_unk": bool(allow_silent_unk),
                "forced_pieces": list(forced_pieces or []),
                "word_official_split": bool(word_official_split),
                "merge_consecutive_unk": bool(merge_consecutive_unk),
                "reverse": bool(reverse),
                "add_bos": bool(add_bos),
                "add_eos": bool(add_eos),
            }
        )
        result_ptr = self.lib.guardd_sp_encode_with_options(
            model_path.encode("utf-8"),
            text.encode("utf-8"),
            options.encode("utf-8"),
        )
        payload = json.loads(self._consume_owned_cstring(result_ptr))
        if not payload.get("ok"):
            raise GuarddError(payload.get("error", "sp_encode failed"))
        tokens = payload.get("tokens")
        if not isinstance(tokens, list):
            raise GuarddError("sp_encode response missing tokens list")
        return [int(t) for t in tokens]

    def sp_nbest_encode(
        self,
        model_path: str,
        text: str,
        nbest_size: int = 1,
        *,
        allow_silent_unk: bool = False,
        forced_pieces: list[str] | None = None,
        merge_consecutive_unk: bool = True,
        reverse: bool = False,
        add_bos: bool = False,
        add_eos: bool = False,
        nbest_stock_compat: bool = False,
    ) -> list[dict]:
        """UNIGRAM lattice n-best. Returns ``[{tokens, score}, ...]`` best-first.

        ``nbest_stock_compat`` enables stock agenda shrink / timeout→Viterbi;
        default refuses pathological lattices.
        """
        options = json.dumps(
            {
                "allow_silent_unk": bool(allow_silent_unk),
                "forced_pieces": list(forced_pieces or []),
                "merge_consecutive_unk": bool(merge_consecutive_unk),
                "reverse": bool(reverse),
                "add_bos": bool(add_bos),
                "add_eos": bool(add_eos),
                "nbest_stock_compat": bool(nbest_stock_compat),
                "nbest_size": int(nbest_size),
            }
        )
        result_ptr = self.lib.guardd_sp_nbest_encode(
            model_path.encode("utf-8"),
            text.encode("utf-8"),
            options.encode("utf-8"),
        )
        payload = json.loads(self._consume_owned_cstring(result_ptr))
        if not payload.get("ok"):
            raise GuarddError(payload.get("error", "sp_nbest_encode failed"))
        rows = payload.get("nbests")
        if not isinstance(rows, list):
            raise GuarddError("sp_nbest_encode response missing nbests")
        out = []
        for row in rows:
            out.append(
                {
                    "tokens": [int(t) for t in row["tokens"]],
                    "score": float(row["score"]),
                }
            )
        return out

    def sp_sample_encode(
        self,
        model_path: str,
        text: str,
        *,
        nbest_size: int = -1,
        alpha: float = 0.0,
        seed: int | None = None,
        allow_silent_unk: bool = False,
        forced_pieces: list[str] | None = None,
        merge_consecutive_unk: bool = True,
        reverse: bool = False,
        add_bos: bool = False,
        add_eos: bool = False,
    ) -> list[int]:
        """UNIGRAM lattice sample / sample-from-nbest, or BPE-dropout.

        For BPE, ``alpha`` is the dropout probability (``nbest_size`` ignored).
        """
        options: dict = {
            "allow_silent_unk": bool(allow_silent_unk),
            "forced_pieces": list(forced_pieces or []),
            "merge_consecutive_unk": bool(merge_consecutive_unk),
            "reverse": bool(reverse),
            "add_bos": bool(add_bos),
            "add_eos": bool(add_eos),
            "nbest_size": int(nbest_size),
            "alpha": float(alpha),
        }
        if seed is not None:
            options["seed"] = int(seed)
        result_ptr = self.lib.guardd_sp_sample_encode(
            model_path.encode("utf-8"),
            text.encode("utf-8"),
            json.dumps(options).encode("utf-8"),
        )
        payload = json.loads(self._consume_owned_cstring(result_ptr))
        if not payload.get("ok"):
            raise GuarddError(payload.get("error", "sp_sample_encode failed"))
        tokens = payload.get("tokens")
        if not isinstance(tokens, list):
            raise GuarddError("sp_sample_encode response missing tokens list")
        return [int(t) for t in tokens]

    def sp_sample_encode_and_score(
        self,
        model_path: str,
        text: str,
        *,
        num_samples: int = 1,
        alpha: float = 0.0,
        wor: bool = False,
        include_best: bool = False,
        seed: int | None = None,
        allow_silent_unk: bool = False,
        forced_pieces: list[str] | None = None,
        merge_consecutive_unk: bool = True,
        reverse: bool = False,
        add_bos: bool = False,
        add_eos: bool = False,
    ) -> list[dict]:
        """UNIGRAM SampleEncodeAndScore.

        With-replacement (``wor=False``): score = path log-prob − marginal.
        Without-replacement (``wor=True``): Gumbel-Top-k draws; score = log
        inclusion probability. ``include_best`` requires ``wor=True``.
        """
        options: dict = {
            "allow_silent_unk": bool(allow_silent_unk),
            "forced_pieces": list(forced_pieces or []),
            "merge_consecutive_unk": bool(merge_consecutive_unk),
            "reverse": bool(reverse),
            "add_bos": bool(add_bos),
            "add_eos": bool(add_eos),
            "num_samples": int(num_samples),
            "alpha": float(alpha),
            "wor": bool(wor),
            "include_best": bool(include_best),
        }
        if seed is not None:
            options["seed"] = int(seed)
        result_ptr = self.lib.guardd_sp_sample_encode_and_score(
            model_path.encode("utf-8"),
            text.encode("utf-8"),
            json.dumps(options).encode("utf-8"),
        )
        payload = json.loads(self._consume_owned_cstring(result_ptr))
        if not payload.get("ok"):
            raise GuarddError(payload.get("error", "sp_sample_encode_and_score failed"))
        rows = payload.get("samples")
        if not isinstance(rows, list):
            raise GuarddError("sp_sample_encode_and_score response missing samples")
        out = []
        for row in rows:
            out.append(
                {
                    "tokens": [int(t) for t in row["tokens"]],
                    "score": float(row["score"]),
                }
            )
        return out

    def sp_decode(
        self, model_path: str, tokens: list[int], *, reverse: bool = False
    ) -> str:
        """
        Decode token IDs toward official SP (▁ dummy-prefix strip, BYTE→UTF-8,
        UNKNOWN→unk_surface, then optional ``denormalizer_spec`` charsmap).

        Reverse NFKC only applies when the model embeds a denormalizer charsmap.
        ``reverse=True`` reverses ids before detokenize (stock decode
        ExtraOptions ``reverse``). ExtraOption ``unk``/``unk_piece`` is a no-op
        on id APIs (stock only rewrites piece strings).

        Args:
            model_path: Path to a SentencePiece ModelProto ``.model`` file
            tokens: Token IDs to decode
            reverse: Reverse ids before detokenize

        Returns:
            Decoded string

        Raises:
            GuarddError: If loading or decoding fails
        """
        tokens_json = json.dumps([int(t) for t in tokens])
        options = json.dumps({"reverse": bool(reverse)})
        result_ptr = self.lib.guardd_sp_decode_with_options(
            model_path.encode("utf-8"),
            tokens_json.encode("utf-8"),
            options.encode("utf-8"),
        )
        payload = json.loads(self._consume_owned_cstring(result_ptr))
        if not payload.get("ok"):
            raise GuarddError(payload.get("error", "sp_decode failed"))
        text = payload.get("text")
        if not isinstance(text, str):
            raise GuarddError("sp_decode response missing text")
        return text

    def perfect_hash_decode_sequence(self, vocab_json: str, tokens: list[int]) -> str:
        """
        Decode a sequence of tokens using perfect hash tokenizer.

        Args:
            vocab_json: JSON string containing the perfect hash vocabulary
            tokens: List of token IDs to decode

        Returns:
            Decoded text

        Raises:
            GuarddError: If decoding fails
        """
        decoded_words = []
        for token in tokens:
            word = self.perfect_hash_decode(vocab_json, token)
            decoded_words.append(word)
        return " ".join(decoded_words)


class HuggingFaceGuard:
    """
    HuggingFace transformers integration for Model Asset Guard.

    This class provides seamless integration between HuggingFace transformers
    and Model Asset Guard verification functions.
    """

    def __init__(self, guard: ModelAssetGuard):
        """
        Initialize HuggingFace integration.

        Args:
            guard: ModelAssetGuard instance
        """
        if not TRANSFORMERS_AVAILABLE:
            raise ImportError(
                "transformers and torch are required for HuggingFace integration"
            )

        self.guard = guard

    def _resolve_local_weight_file(self, model_path: str) -> str | None:
        """Return a local pytorch weight file path, or None if not found."""
        candidates = [
            os.path.join(model_path, "pytorch_model.bin"),
            os.path.join(model_path, "model.safetensors"),
        ]
        for path in candidates:
            if os.path.exists(path):
                return path
        try:
            for file in os.listdir(model_path):
                if file.endswith(".bin") and "pytorch" in file:
                    return os.path.join(model_path, file)
        except OSError:
            return None
        return None

    def _materialize_model_dir(self, model_name: str) -> tuple[str, str]:
        """
        Return ``(local_dir, mode)`` for integrity-checked loading.

        - Local directories: verify-before-load.
        - Hub IDs: snapshot download, then verify-after-download-before-use
          (HuggingFace does not expose a stable verify-before-download API for
          arbitrary weight blobs). Fail closed if download is impossible.
        """
        if os.path.isdir(model_name):
            return model_name, "verify_before_load"

        try:
            from huggingface_hub import snapshot_download
        except ImportError as e:
            raise GuarddError(
                "Remote Hub load requires huggingface_hub for "
                "verify-after-download-before-use; install huggingface_hub or "
                "pass a local model directory"
            ) from e

        try:
            local_dir = snapshot_download(
                repo_id=model_name,
                allow_patterns=[
                    "*.bin",
                    "*.safetensors",
                    "*.json",
                    "tokenizer*",
                    "vocab*",
                    "merges.txt",
                    "config.json",
                ],
            )
        except Exception as e:
            raise GuarddError(
                f"Failed to download Hub snapshot for '{model_name}' "
                f"(fail-closed): {e}"
            ) from e

        if not os.path.isdir(local_dir):
            raise GuarddError(
                f"Hub snapshot for '{model_name}' did not yield a local directory"
            )
        return local_dir, "verify_after_download_before_use"

    def checked_load_pretrained(
        self,
        model_name: str,
        expected_digest: bytes | None = None,
        verify_quantization: bool = True,
        **kwargs,
    ) -> tuple[PreTrainedModel, PreTrainedTokenizer, dict[str, Any]]:
        """
        Load a pretrained model with Model Asset Guard verification.

        Local directories: verify weight file **before** ``from_pretrained``.
        Remote Hub IDs: download a snapshot, verify the weight-file digest,
        then load from the local snapshot (verify-after-download-before-use).
        There is no reliable verify-before-download for Hub blobs.

        Args:
            model_name: HuggingFace model name or path
            expected_digest: Expected SHA-256 digest of the primary weight file
            verify_quantization: Whether to verify quantization bounds
            **kwargs: Additional arguments for from_pretrained

        Returns:
            Tuple of (model, tokenizer, verification_results)

        Raises:
            GuarddError: If verification or download fails (fail-closed)
        """
        print(f"Loading model {model_name} with integrity verification...")
        verification_results: dict[str, Any] = {}

        local_dir, mode = self._materialize_model_dir(model_name)
        verification_results["mode"] = mode
        verification_results["local_dir"] = local_dir

        pytorch_model_path = self._resolve_local_weight_file(local_dir)
        if pytorch_model_path is None:
            raise GuarddError(
                f"Could not find weight file under '{local_dir}' "
                "(expected pytorch_model.bin or model.safetensors)"
            )

        try:
            file_info = self.guard.checked_load(pytorch_model_path, expected_digest)
            verification_results["file_integrity"] = file_info
            if expected_digest is not None and not file_info.get("valid", False):
                raise GuarddError(
                    f"File integrity verification failed for '{pytorch_model_path}'"
                )
            print(f"File integrity digest ({mode}): {file_info['digest']}")
        except GuarddError:
            raise
        except Exception as e:
            raise GuarddError(f"File integrity verification failed: {e}") from e

        # Load only from the verified local tree (never from an unverified remote id).
        model = AutoModel.from_pretrained(local_dir, **kwargs)
        tokenizer = AutoTokenizer.from_pretrained(local_dir, **kwargs)

        if verify_quantization and hasattr(model, "state_dict"):
            print("Verifying quantization bounds...")
            try:
                quant_results = self._verify_model_quantization(model)
            except Exception as e:
                raise GuarddError(f"Quantization verification failed: {e}") from e
            verification_results["quantization"] = quant_results
            if isinstance(quant_results, dict) and "error" in quant_results:
                raise GuarddError(
                    f"Quantization verification failed: {quant_results['error']}"
                )
            if isinstance(quant_results, dict) and quant_results.get(
                "overall_pass_rate", 100.0
            ) < 100.0:
                raise GuarddError(
                    "Quantization verification failed: not all layers passed"
                )
            print("Quantization verification completed")

        verification_results["verified"] = True
        return model, tokenizer, verification_results

    def _verify_model_quantization(self, model: PreTrainedModel) -> dict[str, Any]:
        """
        Verify quantization bounds for all layers in a model.

        Args:
            model: PyTorch model to verify

        Returns:
            Dictionary with verification results
        """
        layers = []
        state_dict = model.state_dict()

        for name, param in state_dict.items():
            if len(param.shape) == 2:  # Only check 2D weight matrices
                weights = param.detach().numpy().flatten().tolist()
                fan_in, fan_out = param.shape

                layers.append(
                    {
                        "name": name,
                        "weights": weights,
                        "fan_in": fan_in,
                        "fan_out": fan_out,
                        "quant_type": "int8",  # Default quantization type
                    }
                )

        if not layers:
            return {"error": "no_2d_layers_found"}

        return self.guard.verify_model_quantization(layers)

    def verify_tokenizer_determinism(
        self,
        tokenizer: PreTrainedTokenizer,
        test_strings: list[str] | None = None,
        num_tests: int = 1000,
    ) -> dict[str, Any]:
        """
        Verify tokenizer determinism property.

        Args:
            tokenizer: HuggingFace tokenizer to test
            test_strings: List of test strings (if None, generates random strings)
            num_tests: Number of random tests to run

        Returns:
            Dictionary with test results
        """
        if test_strings is None:
            test_strings = self._generate_test_strings(num_tests)

        results = {
            "total_tests": len(test_strings),
            "passed": 0,
            "failed": 0,
            "errors": [],
        }

        for i, test_str in enumerate(test_strings):
            try:
                # Encode and decode
                tokens = tokenizer.encode(test_str)
                decoded = tokenizer.decode(tokens)

                # Check determinism: decode(encode(x)) == x
                if decoded == test_str:
                    results["passed"] += 1
                else:
                    results["failed"] += 1
                    results["errors"].append(
                        {
                            "index": i,
                            "original": test_str,
                            "decoded": decoded,
                            "tokens": tokens,
                        }
                    )
            except Exception as e:
                results["failed"] += 1
                results["errors"].append(
                    {"index": i, "original": test_str, "error": str(e)}
                )

        return results

    def _generate_test_strings(self, num_tests: int) -> list[str]:
        """Generate random test strings for tokenizer testing"""
        import random
        import string

        strings = []
        for _ in range(num_tests):
            # Generate strings of varying lengths
            length = random.randint(1, 100)
            test_str = "".join(
                random.choices(string.ascii_letters + string.digits + " ", k=length)
            )
            strings.append(test_str)

        return strings


# Convenience functions for easy usage
def create_guard(lib_path: str | None = None) -> ModelAssetGuard:
    """Create a ModelAssetGuard instance"""
    return ModelAssetGuard(lib_path)


def create_hf_guard(lib_path: str | None = None) -> HuggingFaceGuard:
    """Create a HuggingFaceGuard instance"""
    guard = ModelAssetGuard(lib_path)
    return HuggingFaceGuard(guard)


def checked_load_pretrained(
    model_name: str,
    expected_digest: bytes | None = None,
    verify_quantization: bool = True,
    lib_path: str | None = None,
    **kwargs,
) -> tuple[PreTrainedModel, PreTrainedTokenizer, dict[str, Any]]:
    """
    Convenience function to load a pretrained model with verification.

    Args:
        model_name: HuggingFace model name or path
        expected_digest: Expected SHA-256 digest of the model file
        verify_quantization: Whether to verify quantization bounds
        lib_path: Path to libguardd library
        **kwargs: Additional arguments for from_pretrained

    Returns:
        Tuple of (model, tokenizer, verification_results)
    """
    hf_guard = create_hf_guard(lib_path)
    return hf_guard.checked_load_pretrained(
        model_name, expected_digest, verify_quantization, **kwargs
    )


# Example usage
if __name__ == "__main__":
    # Example: Load GPT-2 with verification
    try:
        model, tokenizer, results = checked_load_pretrained(
            "gpt2", verify_quantization=True
        )
        print("Model loaded successfully!")
        print(f"Verification results: {json.dumps(results, indent=2)}")
    except Exception as e:
        print(f"Failed to load model: {e}")
