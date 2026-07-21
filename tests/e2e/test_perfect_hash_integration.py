"""
Perfect-hash integration tests.

Default (offline) coverage lives primarily in `test_offline_guard.py`.
This module covers optional Node / Lean surfaces, marked `integration`.
"""

from __future__ import annotations

import json
import os
import subprocess
import tempfile
from pathlib import Path

import pytest
from pytorch_guard import ModelAssetGuard


def test_generator_words_json_input(repo_root: Path, guard: ModelAssetGuard) -> None:
    """Words-object input must produce a runtime-loadable vocab."""
    words_obj = {
        "words": ["hello", "world", "test", "perfect", "hash", "tokenizer"],
    }
    with tempfile.NamedTemporaryFile(
        mode="w", suffix=".json", delete=False, encoding="utf-8"
    ) as f:
        json.dump(words_obj, f)
        vocab_in = f.name
    out_fd, out_path = tempfile.mkstemp(suffix=".json")
    os.close(out_fd)
    try:
        result = subprocess.run(
            [
                "cargo",
                "run",
                "--quiet",
                "--manifest-path",
                str(repo_root / "src/rust/guardd/Cargo.toml"),
                "--bin",
                "gen_perfect_hash",
                "--",
                vocab_in,
                "--output",
                out_path,
            ],
            capture_output=True,
            text=True,
            check=False,
            cwd=repo_root,
        )
        assert result.returncode == 0, result.stderr
        vocab = json.loads(Path(out_path).read_text(encoding="utf-8"))
        assert "vocab" in vocab
        assert vocab.get("kind") == "chd_mph"
        assert "pilots" in vocab and "indices" in vocab and "n_buckets" in vocab
        assert len(vocab["indices"]) == len(vocab["vocab"])
        vocab_json = json.dumps(vocab)
        tokens = guard.perfect_hash_encode(vocab_json, "hello world test")
        assert tokens == [1, 2, 3]
        assert guard.perfect_hash_decode_sequence(vocab_json, tokens) == "hello world test"
    finally:
        for p in (vocab_in, out_path):
            if os.path.exists(p):
                os.remove(p)


@pytest.mark.integration
def test_nodejs_bindings_roundtrip(repo_root: Path) -> None:
    node = subprocess.run(["node", "--version"], capture_output=True, text=True)
    if node.returncode != 0:
        pytest.skip("node not available")

    # Build a real vocab via Rust first
    words = {"words": ["hello", "world", "test"]}
    with tempfile.NamedTemporaryFile(
        mode="w", suffix=".json", delete=False, encoding="utf-8"
    ) as f:
        json.dump(words, f)
        vocab_in = f.name
    mph_fd, mph_path = tempfile.mkstemp(suffix=".json")
    os.close(mph_fd)
    try:
        gen = subprocess.run(
            [
                "cargo",
                "run",
                "--quiet",
                "--manifest-path",
                str(repo_root / "src/rust/guardd/Cargo.toml"),
                "--bin",
                "gen_perfect_hash",
                "--",
                vocab_in,
                mph_path,
            ],
            capture_output=True,
            text=True,
            check=False,
            cwd=repo_root,
        )
        assert gen.returncode == 0, gen.stderr
        vocab_json = Path(mph_path).read_text(encoding="utf-8")

        node_guard = (repo_root / "bindings/nodejs/node_guard.js").resolve().as_posix()
        script = f"""
const {{ ModelAssetGuard }} = require({json.dumps(node_guard)});
const guard = new ModelAssetGuard();
const vocabJson = {json.dumps(vocab_json)};
const tokens = guard.perfectHashEncode(vocabJson, "hello world test");
if (!Array.isArray(tokens) || tokens.length !== 3) {{
  console.error("ENCODE_FAIL", tokens);
  process.exit(2);
}}
const decoded = guard.perfectHashDecodeSequence(vocabJson, tokens);
if (decoded !== "hello world test") {{
  console.error("DECODE_FAIL", decoded);
  process.exit(3);
}}
console.log("OK");
"""
        with tempfile.NamedTemporaryFile(
            mode="w", suffix=".js", delete=False, encoding="utf-8"
        ) as sf:
            sf.write(script)
            script_path = sf.name
        try:
            run = subprocess.run(
                ["node", script_path],
                capture_output=True,
                text=True,
                check=False,
                cwd=repo_root,
            )
            err = (run.stderr or "") + (run.stdout or "")
            if (
                "Cannot find module" in err
                or "koffi" in err
                or "ffi-napi" in err
                or "Could not find libguardd" in err
            ):
                pytest.skip(f"nodejs native deps / libguardd unavailable: {err}")
            assert run.returncode == 0, run.stderr or run.stdout
            assert "OK" in run.stdout
        finally:
            os.remove(script_path)
    finally:
        for p in (vocab_in, mph_path):
            if os.path.exists(p):
                os.remove(p)


@pytest.mark.integration
def test_lean_perfecthash_cli_is_honest_stub(repo_root: Path) -> None:
    """Lean perfecthash must not claim simulated success (nonzero exit)."""
    lake = subprocess.run(["lake", "--version"], capture_output=True, text=True)
    if lake.returncode != 0:
        pytest.skip("lake not available")

    with tempfile.NamedTemporaryFile(
        mode="w", suffix=".json", delete=False, encoding="utf-8"
    ) as f:
        json.dump(
            {
                "words": ["hello"],
                "hash_table": [0],
                "table_size": 1,
                "vocab": [["hello", 1]],
            },
            f,
        )
        vocab_file = f.name
    try:
        result = subprocess.run(
            ["lake", "exe", "perfecthash", "encode", vocab_file, "hello"],
            capture_output=True,
            text=True,
            check=False,
            cwd=repo_root,
            timeout=120,
        )
        assert result.returncode != 0
        assert "not implemented" in (result.stdout + result.stderr).lower()
    finally:
        os.remove(vocab_file)
