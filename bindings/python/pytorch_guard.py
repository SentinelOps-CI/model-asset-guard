#!/usr/bin/env python3
"""
Model Asset Guard Python Bindings

This module provides Python bindings for the Model Asset Guard Rust sidecar,
enabling integration with HuggingFace transformers and PyTorch models.

Requirements:
- transformers >= 4.20.0
- torch >= 1.12.0
- numpy >= 1.21.0
- ctypes (built-in)
"""

import os
import json
import tempfile
import ctypes
import ctypes.util
from typing import Dict, List, Optional, Union, Any, Tuple
import warnings

# Try to import optional dependencies
try:
    import numpy as np
    from transformers import (
        PreTrainedModel,
        PreTrainedTokenizer,
        AutoModel,
        AutoTokenizer,
    )

    TRANSFORMERS_AVAILABLE = True
except ImportError:
    TRANSFORMERS_AVAILABLE = False
    warnings.warn(
        "transformers or torch not available. HuggingFace integration disabled."
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

    def __init__(self, lib_path: Optional[str] = None):
        """
        Initialize Model Asset Guard bindings.

        Args:
            lib_path: Path to libguardd.so/dll. If None, will search in common locations.
        """
        self.lib = self._load_library(lib_path)
        self._setup_function_signatures()

    def _load_library(self, lib_path: Optional[str]) -> ctypes.CDLL:
        """Load the Rust sidecar library"""
        if lib_path is None:
            # Search for library in common locations
            search_paths = [
                "./src/rust/guardd/target/release/libguardd.so",
                "./src/rust/guardd/target/debug/libguardd.so",
                "./target/release/libguardd.so",
                "./target/debug/libguardd.so",
                "./src/rust/guardd/target/release/guardd.dll",
                "./src/rust/guardd/target/debug/guardd.dll",
                "./target/release/guardd.dll",
                "./target/debug/guardd.dll",
                ctypes.util.find_library("guardd"),
            ]

            for path in search_paths:
                if path and os.path.exists(path):
                    lib_path = path
                    break
            else:
                raise FileNotFoundError(
                    "Could not find libguardd library. Please build the Rust sidecar first:\n"
                    "cargo build --release --manifest-path src/rust/guardd/Cargo.toml"
                )

        try:
            lib = ctypes.CDLL(lib_path)
            return lib
        except Exception as e:
            raise RuntimeError(f"Failed to load library {lib_path}: {e}")

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

        self.lib.guardd_free_json_result.argtypes = [ctypes.c_char_p]
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
        self.lib.guardd_verify_quant_128_vectors.restype = ctypes.c_char_p

        # Model verification
        self.lib.guardd_verify_model_128_vectors.argtypes = [
            ctypes.c_char_p,  # layers_json
        ]
        self.lib.guardd_verify_model_128_vectors.restype = ctypes.c_char_p

        # Bit-flip corpus testing
        self.lib.guardd_run_bitflip_corpus_test.argtypes = [
            ctypes.c_size_t,  # file_size_gb
            ctypes.c_size_t,  # num_corruptions
            ctypes.c_char_p,  # temp_dir
        ]
        self.lib.guardd_run_bitflip_corpus_test.restype = ctypes.c_char_p

        # Perfect hash tokenizer functions
        self.lib.guardd_perfect_hash_encode.argtypes = [
            ctypes.c_char_p,  # vocab_json
            ctypes.c_char_p,  # text
        ]
        self.lib.guardd_perfect_hash_encode.restype = ctypes.c_char_p

        self.lib.guardd_perfect_hash_decode.argtypes = [
            ctypes.c_char_p,  # vocab_json
            ctypes.c_uint32,  # token
        ]
        self.lib.guardd_perfect_hash_decode.restype = ctypes.c_char_p

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
        self, file_path: str, expected_digest: Optional[bytes] = None
    ) -> Dict[str, Any]:
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
        weights: Union[List[float], np.ndarray],
        fan_in: int,
        fan_out: int,
        quant_type: str = "int8",
    ) -> Dict[str, Any]:
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

        if not result_ptr:
            raise GuarddError("Quantization verification failed")

        try:
            result_casted = ctypes.cast(result_ptr, ctypes.c_char_p)
            if result_casted.value is None:
                raise GuarddError("Quantization verification returned null result")
            result_json = result_casted.value.decode("utf-8")
            return json.loads(result_json)
        finally:
            self.lib.guardd_free_json_result(result_ptr)

    def verify_model_quantization(self, layers: List[Dict[str, Any]]) -> Dict[str, Any]:
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

        if not result_ptr:
            raise GuarddError("Model quantization verification failed")

        try:
            result_casted = ctypes.cast(result_ptr, ctypes.c_char_p)
            if result_casted.value is None:
                raise GuarddError(
                    "Model quantization verification returned null result"
                )
            result_json = result_casted.value.decode("utf-8")
            return json.loads(result_json)
        finally:
            self.lib.guardd_free_json_result(result_ptr)

    def run_bitflip_corpus_test(
        self,
        file_size_gb: int = 1,
        num_corruptions: int = 10,
        temp_dir: Optional[str] = None,
    ) -> Dict[str, Any]:
        """
        Run bit-flip corpus test for weight integrity validation.

        Args:
            file_size_gb: Size of test files in GB
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

        if not result_ptr:
            raise GuarddError("Bit-flip corpus test failed")

        try:
            result_casted = ctypes.cast(result_ptr, ctypes.c_char_p)
            if result_casted.value is None:
                raise GuarddError("Bit-flip corpus test returned null result")
            result_json = result_casted.value.decode("utf-8")
            return json.loads(result_json)
        finally:
            self.lib.guardd_free_json_result(result_ptr)

    def perfect_hash_encode(self, vocab_json: str, text: str) -> List[int]:
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

        if not result_ptr:
            raise GuarddError("Perfect hash encoding failed")

        try:
            result_casted = ctypes.cast(result_ptr, ctypes.c_char_p)
            if result_casted.value is None:
                raise GuarddError("Perfect hash encoding returned null result")
            result_json = result_casted.value.decode("utf-8")
            return json.loads(result_json)
        finally:
            self.lib.guardd_free_json_result(result_ptr)

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

        if not result_ptr:
            raise GuarddError("Perfect hash decoding failed")

        try:
            result_casted = ctypes.cast(result_ptr, ctypes.c_char_p)
            if result_casted.value is None:
                raise GuarddError("Perfect hash decoding returned null result")
            result_str = result_casted.value.decode("utf-8")
            return result_str
        finally:
            self.lib.guardd_free_json_result(result_ptr)

    def perfect_hash_decode_sequence(self, vocab_json: str, tokens: List[int]) -> str:
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

    def checked_load_pretrained(
        self,
        model_name: str,
        expected_digest: Optional[bytes] = None,
        verify_quantization: bool = True,
        **kwargs,
    ) -> Tuple[PreTrainedModel, PreTrainedTokenizer, Dict[str, Any]]:
        """
        Load a pretrained model with Model Asset Guard verification.

        Args:
            model_name: HuggingFace model name or path
            expected_digest: Expected SHA-256 digest of the model file
            verify_quantization: Whether to verify quantization bounds
            **kwargs: Additional arguments for from_pretrained

        Returns:
            Tuple of (model, tokenizer, verification_results)

        Raises:
            GuarddError: If verification fails
        """
        print(f"Loading model {model_name} with integrity verification...")

        # Load model and tokenizer normally first
        model = AutoModel.from_pretrained(model_name, **kwargs)
        tokenizer = AutoTokenizer.from_pretrained(model_name, **kwargs)

        # Get model file path
        model_path = model.config._name_or_path
        if os.path.isdir(model_path):
            # Local directory
            pytorch_model_path = os.path.join(model_path, "pytorch_model.bin")
            if not os.path.exists(pytorch_model_path):
                # Try to find the actual model file
                for file in os.listdir(model_path):
                    if file.endswith(".bin") and "pytorch" in file:
                        pytorch_model_path = os.path.join(model_path, file)
                        break
        else:
            # Remote model, we can't verify the file directly
            print("Warning: Remote model, skipping file integrity verification")
            return model, tokenizer, {"verified": False, "reason": "remote_model"}

        # Verify model file integrity
        verification_results = {}

        if os.path.exists(pytorch_model_path):
            try:
                file_info = self.guard.checked_load(pytorch_model_path, expected_digest)
                verification_results["file_integrity"] = file_info
                print(f"✓ File integrity verified: {file_info['digest']}")
            except Exception as e:
                print(f"✗ File integrity verification failed: {e}")
                verification_results["file_integrity"] = {"error": str(e)}
        else:
            print("Warning: Could not find pytorch_model.bin file")
            verification_results["file_integrity"] = {"error": "file_not_found"}

        # Verify quantization bounds if requested
        if verify_quantization and hasattr(model, "state_dict"):
            try:
                print("Verifying quantization bounds...")
                quant_results = self._verify_model_quantization(model)
                verification_results["quantization"] = quant_results
                print("✓ Quantization verification completed")
            except Exception as e:
                print(f"✗ Quantization verification failed: {e}")
                verification_results["quantization"] = {"error": str(e)}

        verification_results["verified"] = True
        return model, tokenizer, verification_results

    def _verify_model_quantization(self, model: PreTrainedModel) -> Dict[str, Any]:
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
        test_strings: Optional[List[str]] = None,
        num_tests: int = 1000,
    ) -> Dict[str, Any]:
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

    def _generate_test_strings(self, num_tests: int) -> List[str]:
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
def create_guard(lib_path: Optional[str] = None) -> ModelAssetGuard:
    """Create a ModelAssetGuard instance"""
    return ModelAssetGuard(lib_path)


def create_hf_guard(lib_path: Optional[str] = None) -> HuggingFaceGuard:
    """Create a HuggingFaceGuard instance"""
    guard = ModelAssetGuard(lib_path)
    return HuggingFaceGuard(guard)


def checked_load_pretrained(
    model_name: str,
    expected_digest: Optional[bytes] = None,
    verify_quantization: bool = True,
    lib_path: Optional[str] = None,
    **kwargs,
) -> Tuple[PreTrainedModel, PreTrainedTokenizer, Dict[str, Any]]:
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
