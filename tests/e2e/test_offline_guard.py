"""
Offline e2e tests: require only a built libguardd (+ optional cargo for gen_perfect_hash).

No HuggingFace downloads, Node, or Lean toolchain.
"""

from __future__ import annotations

import hashlib
import json
import os
import subprocess
import tempfile
from pathlib import Path

import pytest
from pytorch_guard import GuarddError, ModelAssetGuard


def test_verify_digest_match(guard: ModelAssetGuard) -> None:
    content = b"Hello, Model Asset Guard!"
    with tempfile.NamedTemporaryFile(delete=False) as tmp:
        tmp.write(content)
        path = tmp.name
    try:
        expected = hashlib.sha256(content).digest()
        assert guard.verify_digest(path, expected) is True
    finally:
        os.remove(path)


def test_verify_digest_mismatch_returns_false(guard: ModelAssetGuard) -> None:
    """F026: mismatch must return False, not raise."""
    content = b"test content"
    with tempfile.NamedTemporaryFile(delete=False) as tmp:
        tmp.write(content)
        path = tmp.name
    try:
        wrong = b"\x00" * 32
        assert guard.verify_digest(path, wrong) is False
    finally:
        os.remove(path)


def test_verify_digest_missing_file_raises(guard: ModelAssetGuard) -> None:
    with pytest.raises(GuarddError):
        guard.verify_digest("/non/existent/file/model-asset-guard", b"\x00" * 32)


def test_verify_digest_short_digest_raises(guard: ModelAssetGuard) -> None:
    with tempfile.NamedTemporaryFile(delete=False) as tmp:
        tmp.write(b"x")
        path = tmp.name
    try:
        with pytest.raises(ValueError):
            guard.verify_digest(path, b"short")
    finally:
        os.remove(path)


def test_checked_load_valid_flag(guard: ModelAssetGuard) -> None:
    content = b"checked-load-payload"
    with tempfile.NamedTemporaryFile(delete=False) as tmp:
        tmp.write(content)
        path = tmp.name
    try:
        good = hashlib.sha256(content).digest()
        info = guard.checked_load(path, good)
        assert info["valid"] is True
        assert info["digest"] == good.hex()

        bad = guard.checked_load(path, b"\xff" * 32)
        assert bad["valid"] is False
    finally:
        os.remove(path)


def test_model_quantization_object_json_contract(guard: ModelAssetGuard) -> None:
    """F004: Python object-form layers must be accepted by Rust."""
    layers = [
        {
            "name": "linear.weight",
            "weights": [0.1, 0.2, 0.3, 0.4, 0.5, 0.6],
            "fan_in": 3,
            "fan_out": 2,
            "quant_type": "int8",
        }
    ]
    result = guard.verify_model_quantization(layers)
    assert isinstance(result, dict)
    assert result.get("total_layers") == 1
    assert result["layers"][0]["layer_name"] == "linear.weight"
    assert len(result["layers"][0]["error_distribution"]) == 128


def _generate_mph_vocab(repo_root: Path, words: list[str]) -> dict:
    with tempfile.NamedTemporaryFile(
        mode="w", suffix=".txt", delete=False, encoding="utf-8"
    ) as vocab_in:
        vocab_in.write("\n".join(words) + "\n")
        vocab_path = vocab_in.name
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
                vocab_path,
                out_path,
            ],
            capture_output=True,
            text=True,
            check=False,
            cwd=repo_root,
        )
        assert result.returncode == 0, result.stderr
        return json.loads(Path(out_path).read_text(encoding="utf-8"))
    finally:
        if os.path.exists(vocab_path):
            os.remove(vocab_path)
        if os.path.exists(out_path):
            os.remove(out_path)


def test_perfect_hash_encode_decode_roundtrip(
    guard: ModelAssetGuard, repo_root: Path
) -> None:
    """F005: runtime schema round-trips via FFI."""
    vocab = _generate_mph_vocab(
        repo_root, ["hello", "world", "test", "perfect", "hash"]
    )
    vocab_json = json.dumps(vocab)
    text = "hello world test"
    tokens = guard.perfect_hash_encode(vocab_json, text)
    assert tokens == [1, 2, 3]
    assert guard.perfect_hash_decode(vocab_json, tokens[0]) == "hello"
    assert guard.perfect_hash_decode_sequence(vocab_json, tokens) == text


def test_gen_perfect_hash_cli_positional_and_output_flag(repo_root: Path) -> None:
    words_path = repo_root / "tmp_vocab_words.txt"
    out_a = repo_root / "tmp_mph_a.json"
    out_b = repo_root / "tmp_mph_b.json"
    words_path.write_text("alpha\nbeta\ngamma\n", encoding="utf-8")
    try:
        base = [
            "cargo",
            "run",
            "--quiet",
            "--manifest-path",
            str(repo_root / "src/rust/guardd/Cargo.toml"),
            "--bin",
            "gen_perfect_hash",
            "--",
            str(words_path),
        ]
        r1 = subprocess.run(
            [*base, str(out_a)],
            capture_output=True,
            text=True,
            check=False,
            cwd=repo_root,
        )
        assert r1.returncode == 0, r1.stderr
        assert out_a.is_file()

        r2 = subprocess.run(
            [*base, "--output", str(out_b)],
            capture_output=True,
            text=True,
            check=False,
            cwd=repo_root,
        )
        assert r2.returncode == 0, r2.stderr
        assert out_b.is_file()

        a = json.loads(out_a.read_text(encoding="utf-8"))
        b = json.loads(out_b.read_text(encoding="utf-8"))
        assert a["vocab"] == b["vocab"]
        assert a.get("kind") == "chd_mph"
        assert b.get("kind") == "chd_mph"
        assert a["pilots"] == b["pilots"]
        assert a["indices"] == b["indices"]
        assert a["n_buckets"] == b["n_buckets"]
        assert len(a["indices"]) == len(a["vocab"])
    finally:
        for p in (words_path, out_a, out_b):
            if p.exists():
                p.unlink()


def test_bitflip_corpus_small(guard: ModelAssetGuard) -> None:
    result = guard.run_bitflip_corpus_test(
        file_size_gb=0,
        num_corruptions=3,
        temp_dir=tempfile.gettempdir(),
    )
    assert isinstance(result, dict)
    assert "test_passed" in result


def test_hf_local_materialize_verify_before_load(guard: ModelAssetGuard, tmp_path: Path) -> None:
    """Local dirs use verify-before-load; no Hub download."""
    pytest.importorskip("transformers")
    from pytorch_guard import HuggingFaceGuard

    weight = tmp_path / "pytorch_model.bin"
    weight.write_bytes(b"fake-weights-for-materialize")
    (tmp_path / "config.json").write_text("{}", encoding="utf-8")

    hf = HuggingFaceGuard(guard)
    local_dir, mode = hf._materialize_model_dir(str(tmp_path))
    assert mode == "verify_before_load"
    assert local_dir == str(tmp_path)
    resolved = hf._resolve_local_weight_file(local_dir)
    assert resolved == str(weight)


def test_shared_epsilon_fixture_int8(guard: ModelAssetGuard) -> None:
    """Align Python/Rust with tests/fixtures/quant_epsilon_int8.json (Lean uses same formula)."""
    fixture = (
        Path(__file__).resolve().parents[2]
        / "tests"
        / "fixtures"
        / "quant_epsilon_int8.json"
    )
    data = json.loads(fixture.read_text(encoding="utf-8"))
    model = guard.verify_model_quantization(
        [
            {
                "name": data["layer_name"],
                "weights": data["weights"],
                "fan_in": data["fan_in"],
                "fan_out": data["fan_out"],
                "quant_type": data["quant_type"],
            }
        ]
    )
    assert model["total_layers"] == 1
    layer = model["layers"][0]
    assert layer["epsilon_bound"] == pytest.approx(data["epsilon_bound"], abs=1e-9)
    assert layer["epsilon_bound"] == pytest.approx(
        0.5 * (data["fan_in"] ** 0.5), abs=1e-12
    )
    assert layer["passed_128_vectors"] is True
    assert len(layer["error_distribution"]) == 128


def _pb_varint(v: int) -> bytes:
    out = bytearray()
    while True:
        b = v & 0x7F
        v >>= 7
        if v:
            b |= 0x80
        out.append(b)
        if not v:
            return bytes(out)


def _pb_len_delim(field: int, payload: bytes) -> bytes:
    return _pb_varint((field << 3) | 2) + _pb_varint(len(payload)) + payload


def _craft_sp_bpe_model() -> bytes:
    """Minimal BPE ModelProto: ▁/a/b/ab/▁a/▁ab with scores (matches Rust unit fixture)."""
    import struct

    def piece(text: str, score: float, typ: int = 1) -> bytes:
        body = _pb_varint((1 << 3) | 2) + _pb_varint(len(text.encode())) + text.encode()
        body += _pb_varint((2 << 3) | 5) + struct.pack("<f", score)
        body += _pb_varint((3 << 3) | 0) + _pb_varint(typ)
        return _pb_len_delim(1, body)

    out = bytearray()
    out += piece("<unk>", 0.0, 2)
    out += piece("\u2581", -5.0)
    out += piece("a", -4.0)
    out += piece("b", -4.0)
    out += piece("ab", -1.0)
    out += piece("\u2581a", -0.5)
    out += piece("\u2581ab", 0.0)
    trainer = _pb_varint((3 << 3) | 0) + _pb_varint(2)  # model_type=BPE
    out += _pb_len_delim(2, trainer)
    norm = (
        _pb_varint((3 << 3) | 0)
        + _pb_varint(1)  # add_dummy_prefix
        + _pb_varint((4 << 3) | 0)
        + _pb_varint(1)  # remove_extra_whitespaces
        + _pb_varint((5 << 3) | 0)
        + _pb_varint(1)  # escape_whitespaces
    )
    out += _pb_len_delim(3, norm)
    return bytes(out)


def _craft_sp_unigram_model() -> bytes:
    """Minimal UNIGRAM ModelProto with unique Viterbi optimum ▁ab."""
    import struct

    def piece(text: str, score: float, typ: int = 1) -> bytes:
        body = _pb_varint((1 << 3) | 2) + _pb_varint(len(text.encode())) + text.encode()
        body += _pb_varint((2 << 3) | 5) + struct.pack("<f", score)
        body += _pb_varint((3 << 3) | 0) + _pb_varint(typ)
        return _pb_len_delim(1, body)

    out = bytearray()
    out += piece("<unk>", 0.0, 2)
    out += piece("\u2581", -10.0)
    out += piece("a", -5.0)
    out += piece("b", -5.0)
    out += piece("ab", -3.0)
    out += piece("\u2581a", -2.0)
    out += piece("\u2581ab", -0.5)
    trainer = _pb_varint((3 << 3) | 0) + _pb_varint(1)  # model_type=UNIGRAM
    out += _pb_len_delim(2, trainer)
    norm = (
        _pb_varint((3 << 3) | 0)
        + _pb_varint(1)
        + _pb_varint((4 << 3) | 0)
        + _pb_varint(1)
        + _pb_varint((5 << 3) | 0)
        + _pb_varint(1)
    )
    out += _pb_len_delim(3, norm)
    return bytes(out)


def test_sentencepiece_bpe_sp_encode_cli(repo_root: Path) -> None:
    """Wave 7/8: crafted BPE .model encodes via guardd_perfect_hash sp-encode."""
    model_path = repo_root / "tmp_sp_bpe_wave8.model"
    model_path.write_bytes(_craft_sp_bpe_model())
    try:
        result = subprocess.run(
            [
                "cargo",
                "run",
                "--quiet",
                "--manifest-path",
                str(repo_root / "src/rust/guardd/Cargo.toml"),
                "--bin",
                "guardd_perfect_hash",
                "--",
                "sp-encode",
                str(model_path),
                "ab",
            ],
            capture_output=True,
            text=True,
            check=False,
            cwd=repo_root,
        )
        assert result.returncode == 0, result.stderr
        tokens = json.loads(result.stdout.strip())
        assert tokens == [6], tokens

        test = subprocess.run(
            [
                "cargo",
                "run",
                "--quiet",
                "--manifest-path",
                str(repo_root / "src/rust/guardd/Cargo.toml"),
                "--bin",
                "guardd_perfect_hash",
                "--",
                "test",
                str(model_path),
            ],
            capture_output=True,
            text=True,
            check=False,
            cwd=repo_root,
        )
        assert test.returncode == 0, test.stderr
        meta = json.loads(test.stdout.strip())
        assert meta["ok"] is True
        assert meta["kind"] == "sentencepiece_model_proto"
        assert meta["model_type"] == "BPE"
        assert meta["encode"] == "supported"
    finally:
        if model_path.exists():
            model_path.unlink()


def test_sentencepiece_unigram_sp_encode_ffi(guard: ModelAssetGuard, repo_root: Path) -> None:
    """Wave 8: UNIGRAM Viterbi via Python FFI + CLI golden vector."""
    model_path = repo_root / "tmp_sp_unigram_wave8.model"
    model_path.write_bytes(_craft_sp_unigram_model())
    try:
        tokens = guard.sp_encode(str(model_path), "ab")
        assert tokens == [6], tokens
        assert guard.sp_decode(str(model_path), tokens) == "ab"

        cli = subprocess.run(
            [
                "cargo",
                "run",
                "--quiet",
                "--manifest-path",
                str(repo_root / "src/rust/guardd/Cargo.toml"),
                "--bin",
                "guardd_perfect_hash",
                "--",
                "sp-encode",
                str(model_path),
                "ab",
            ],
            capture_output=True,
            text=True,
            check=False,
            cwd=repo_root,
        )
        assert cli.returncode == 0, cli.stderr
        assert json.loads(cli.stdout.strip()) == [6]

        with pytest.raises(GuarddError, match="no covering segmentation|not in vocab|fail"):
            # Missing character coverage must fail closed (no silent unk).
            guard.sp_encode(str(model_path), "xyz")
    finally:
        if model_path.exists():
            model_path.unlink()


def test_sp_nfkc_normalize_goldens_offline_hygiene(repo_root: Path) -> None:
    """Wave 27: curated NFKC golden shape without live SP / nfkc blobs.

    Identity rows must be pure passthrough under charsmap-only knobs (in == out).
    Full nfkc / nfkc_cf charsmap cross-check remains integration-only.
    """
    gold_path = repo_root / "tests" / "fixtures" / "sp_nfkc_normalize_goldens.json"
    gold = json.loads(gold_path.read_text(encoding="utf-8"))
    rules = gold["rule"]
    assert set(rules) == {"identity", "nfkc", "nfkc_cf"}
    counts = {name: len(rows) for name, rows in rules.items()}
    assert len(set(counts.values())) == 1
    assert counts["identity"] > 100
    assert "permanent non-goal" in gold.get("note", "").lower() or "MT19937" in gold.get(
        "note", ""
    )
    for row in rules["identity"]:
        assert row["in"] == row["out"], row
    # Spot-check a few known NFKC transforms stay present (fixture integrity).
    nfkc = {row["in"]: row["out"] for row in rules["nfkc"]}
    assert nfkc["\uff21"] == "A"
    assert nfkc["\ufb01"] == "fi"
    assert nfkc["\u00b2"] == "2"
