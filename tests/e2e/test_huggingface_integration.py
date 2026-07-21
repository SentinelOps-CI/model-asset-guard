"""
HuggingFace / network-facing integration tests.

Marked `integration` — excluded from default preflight (no HF downloads in CI gate).
"""

from __future__ import annotations

import hashlib
import os
import tempfile
import time

import pytest

pytest.importorskip("numpy")
np = pytest.importorskip("numpy")
pytest.importorskip("transformers")

from pytorch_guard import (  # noqa: E402
    HuggingFaceGuard,
    ModelAssetGuard,
    checked_load_pretrained,
)

pytestmark = pytest.mark.integration


@pytest.fixture
def hf_guard(guard: ModelAssetGuard) -> HuggingFaceGuard:
    return HuggingFaceGuard(guard)


def test_quantization_single_layer(guard: ModelAssetGuard) -> None:
    fan_in, fan_out = 64, 32
    weights = np.random.randn(fan_in, fan_out).astype(np.float32)
    layer_result = guard.verify_quantization_128_vectors(
        layer_name="test_layer",
        weights=weights,
        fan_in=fan_in,
        fan_out=fan_out,
        quant_type="int8",
    )
    assert isinstance(layer_result, dict)
    assert layer_result["layer_name"] == "test_layer"
    assert len(layer_result["error_distribution"]) == 128


# Hub LFS oid for sshleifer/tiny-gpt2 `pytorch_model.bin` (commit 0c95bc9).
# Integration-only: skip/fail if Hub replaces the blob.
TINY_GPT2_PYTORCH_BIN_SHA256 = bytes.fromhex(
    "b706b24034032bdfe765ded5ab6403d201d295a995b790cb24c74becca5c04e6"
)


def test_huggingface_checked_load_pretrained(hf_guard: HuggingFaceGuard) -> None:
    model, tokenizer, verification_results = hf_guard.checked_load_pretrained(
        "sshleifer/tiny-gpt2",
        verify_quantization=False,
    )
    assert model is not None
    assert tokenizer is not None
    assert isinstance(verification_results, dict)
    # Remote Hub: verify-after-download-before-use (no verify-before-download API).
    assert verification_results.get("verified") is True
    assert (
        verification_results.get("mode") == "verify_after_download_before_use"
    )
    assert "file_integrity" in verification_results
    assert verification_results["file_integrity"].get("digest")


def test_huggingface_pinned_weight_digest(hf_guard: HuggingFaceGuard) -> None:
    """Pin Hub weight digest: verify-after-download must match the known LFS oid."""
    model, tokenizer, verification_results = hf_guard.checked_load_pretrained(
        "sshleifer/tiny-gpt2",
        expected_digest=TINY_GPT2_PYTORCH_BIN_SHA256,
        verify_quantization=False,
    )
    assert model is not None
    assert tokenizer is not None
    info = verification_results["file_integrity"]
    assert info.get("valid") is True
    assert info.get("digest") == TINY_GPT2_PYTORCH_BIN_SHA256.hex()


def test_convenience_checked_load_pretrained() -> None:
    model, tokenizer, results = checked_load_pretrained(
        "sshleifer/tiny-gpt2",
        verify_quantization=False,
    )
    assert model is not None
    assert tokenizer is not None
    assert results.get("verified") is True
    assert results.get("mode") == "verify_after_download_before_use"


def test_tokenizer_determinism_sample(hf_guard: HuggingFaceGuard) -> None:
    from transformers import AutoTokenizer

    tokenizer = AutoTokenizer.from_pretrained("sshleifer/tiny-gpt2")
    # HF decode(encode(x)) is not always identity; we only assert the API returns structure.
    results = hf_guard.verify_tokenizer_determinism(
        tokenizer,
        test_strings=["hello", "world"],
        num_tests=2,
    )
    assert results["total_tests"] == 2
    assert "passed" in results and "failed" in results


def test_digest_throughput_smoke(guard: ModelAssetGuard) -> None:
    content = b"x" * (256 * 1024)
    fd, path = tempfile.mkstemp()
    try:
        os.write(fd, content)
        os.close(fd)
        expected = hashlib.sha256(content).digest()
        start = time.time()
        for _ in range(5):
            assert guard.verify_digest(path, expected)
        elapsed = time.time() - start
        assert elapsed < 5.0
    finally:
        if os.path.exists(path):
            os.remove(path)
