"""Optional cross-check against the official `sentencepiece` Python package.

Marked `integration` — skipped from default offline preflight.
Requires: `pip install sentencepiece` (no network needed for crafted / fixture models).
"""

from __future__ import annotations

import json
import struct
from pathlib import Path

import pytest
from pytorch_guard import GuarddError

pytestmark = pytest.mark.integration

sentencepiece = pytest.importorskip("sentencepiece")

FIXTURES = Path(__file__).resolve().parents[1] / "fixtures"


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


def _piece(text: str, score: float, typ: int = 1) -> bytes:
    body = _pb_varint((1 << 3) | 2) + _pb_varint(len(text.encode())) + text.encode()
    body += _pb_varint((2 << 3) | 5) + struct.pack("<f", score)
    body += _pb_varint((3 << 3) | 0) + _pb_varint(typ)
    return _pb_len_delim(1, body)


def _norm_spec() -> bytes:
    return (
        _pb_varint((3 << 3) | 0)
        + _pb_varint(1)
        + _pb_varint((4 << 3) | 0)
        + _pb_varint(1)
        + _pb_varint((5 << 3) | 0)
        + _pb_varint(1)
    )


def craft_unigram_unique_optimum() -> bytes:
    out = bytearray()
    out += _piece("<unk>", 0.0, 2)
    out += _piece("\u2581", -10.0)
    out += _piece("a", -5.0)
    out += _piece("b", -5.0)
    out += _piece("ab", -3.0)
    out += _piece("\u2581a", -2.0)
    out += _piece("\u2581ab", -0.5)
    out += _pb_len_delim(2, _pb_varint((3 << 3) | 0) + _pb_varint(1))  # UNIGRAM
    out += _pb_len_delim(3, _norm_spec())
    return bytes(out)


def craft_bpe_unique_optimum() -> bytes:
    out = bytearray()
    out += _piece("<unk>", 0.0, 2)
    out += _piece("\u2581", -5.0)
    out += _piece("a", -4.0)
    out += _piece("b", -4.0)
    out += _piece("ab", -1.0)
    out += _piece("\u2581a", -0.5)
    out += _piece("\u2581ab", 0.0)
    out += _pb_len_delim(2, _pb_varint((3 << 3) | 0) + _pb_varint(2))  # BPE
    out += _pb_len_delim(3, _norm_spec())
    return bytes(out)


def craft_char_limited_vocab() -> bytes:
    out = bytearray()
    out += _piece("<unk>", 0.0, 2)
    out += _piece("\u2581", 0.0)
    out += _piece("a", 0.0)
    out += _pb_len_delim(2, _pb_varint((3 << 3) | 0) + _pb_varint(4))  # CHAR
    out += _pb_len_delim(3, _norm_spec())
    return bytes(out)


def craft_word_model() -> bytes:
    out = bytearray()
    out += _piece("<unk>", 0.0, 2)
    out += _piece("\u2581hello", 0.0)
    out += _piece("\u2581world", 0.0)
    out += _pb_len_delim(2, _pb_varint((3 << 3) | 0) + _pb_varint(3))  # WORD
    out += _pb_len_delim(3, _norm_spec())
    return bytes(out)


def craft_word_suffix_ws() -> bytes:
    out = bytearray()
    out += _piece("<unk>", 0.0, 2)
    out += _piece("hello\u2581", 0.0)
    out += _piece("world\u2581", 0.0)
    trainer = (
        _pb_varint((3 << 3) | 0)
        + _pb_varint(3)  # WORD
        + _pb_varint((24 << 3) | 0)
        + _pb_varint(1)  # treat_whitespace_as_suffix
    )
    out += _pb_len_delim(2, trainer)
    out += _pb_len_delim(3, _norm_spec())
    return bytes(out)


def craft_control_symbols() -> bytes:
    out = bytearray()
    out += _piece("<unk>", 0.0, 2)
    out += _piece("<s>", 0.0, 3)
    out += _piece("</s>", 0.0, 3)
    out += _piece("\u2581a", -1.0)
    out += _pb_len_delim(2, _pb_varint((3 << 3) | 0) + _pb_varint(1))
    out += _pb_len_delim(3, _norm_spec())
    return bytes(out)


def test_unigram_matches_sentencepiece_package(guard, tmp_path: Path) -> None:
    model_path = tmp_path / "ug.model"
    model_path.write_bytes(craft_unigram_unique_optimum())

    sp = sentencepiece.SentencePieceProcessor(model_file=str(model_path))
    official = list(sp.encode("ab", out_type=int))
    ours = guard.sp_encode(str(model_path), "ab")
    assert ours == official == [6], (ours, official)
    assert guard.sp_decode(str(model_path), ours) == sp.decode(official) == "ab"


def test_bpe_matches_sentencepiece_package(guard, tmp_path: Path) -> None:
    model_path = tmp_path / "bpe.model"
    model_path.write_bytes(craft_bpe_unique_optimum())

    sp = sentencepiece.SentencePieceProcessor(model_file=str(model_path))
    official = list(sp.encode("ab", out_type=int))
    ours = guard.sp_encode(str(model_path), "ab")
    assert ours == official == [6], (ours, official)


def test_identity_fixture_matches_sentencepiece(guard) -> None:
    model_path = FIXTURES / "sp_tiny_identity_unigram.model"
    assert model_path.is_file()
    sp = sentencepiece.SentencePieceProcessor(model_file=str(model_path))
    for text in ("hello", "  foo  bar  "):
        official = list(sp.encode(text, out_type=int))
        ours = guard.sp_encode(str(model_path), text)
        assert ours == official, (text, ours, official)
        assert guard.sp_decode(str(model_path), ours) == sp.decode(official)


def test_custom_charsmap_fixture_matches_on_covered_text(guard) -> None:
    """Charsmap is applied; covered ASCII still matches official SP."""
    model_path = FIXTURES / "sp_tiny_custom_norm_unigram.model"
    assert model_path.is_file()
    sp = sentencepiece.SentencePieceProcessor(model_file=str(model_path))
    for text in ("hello", "  foo  bar  "):
        official = list(sp.encode(text, out_type=int))
        ours = guard.sp_encode(str(model_path), text)
        assert ours == official, (text, ours, official)

    # Fullwidth A is rewritten by charsmap to ASCII A (OOV). Stock SP emits unk;
    # we remain fail-closed without byte_fallback / silent_unk.
    with pytest.raises(GuarddError):
        guard.sp_encode(str(model_path), "\uff21bc")


def test_byte_fallback_fixture_matches_sentencepiece(guard) -> None:
    model_path = FIXTURES / "sp_tiny_identity_bpe_bytefallback.model"
    assert model_path.is_file()
    sp = sentencepiece.SentencePieceProcessor(model_file=str(model_path))
    for text in ("hello", "Z", "ABC", "  foo  bar  ", "\U0001f600"):
        official = list(sp.encode(text, out_type=int))
        ours = guard.sp_encode(str(model_path), text)
        assert ours == official, (text, ours, official)
        assert guard.sp_decode(str(model_path), ours) == sp.decode(official)


def test_denorm_fixture_roundtrip_matches_sentencepiece(guard) -> None:
    model_path = FIXTURES / "sp_tiny_identity_denorm_unigram.model"
    assert model_path.is_file()
    sp = sentencepiece.SentencePieceProcessor(model_file=str(model_path))
    for text in ("hello", "TM", "xTMy", "ATM"):
        official = list(sp.encode(text, out_type=int))
        ours = guard.sp_encode(str(model_path), text)
        assert ours == official, (text, ours, official)
        assert guard.sp_decode(str(model_path), ours) == sp.decode(official)


def test_silent_unk_opt_in_matches_sentencepiece(guard, tmp_path: Path) -> None:
    model_path = tmp_path / "char_lim.model"
    model_path.write_bytes(craft_char_limited_vocab())
    sp = sentencepiece.SentencePieceProcessor(model_file=str(model_path))
    official = list(sp.encode("z", out_type=int))
    with pytest.raises(GuarddError):
        guard.sp_encode(str(model_path), "z")
    ours = guard.sp_encode(str(model_path), "z", allow_silent_unk=True)
    assert ours == official == [1, 0], (ours, official)
    assert guard.sp_decode(str(model_path), ours) == sp.decode(official)
    # Consecutive OOV chars merge to one unk (stock processor).
    official_xyz = list(sp.encode("xyz", out_type=int))
    ours_xyz = guard.sp_encode(str(model_path), "xyz", allow_silent_unk=True)
    assert ours_xyz == official_xyz == [1, 0], (ours_xyz, official_xyz)
    piece = guard.sp_encode(
        str(model_path), "xyz", allow_silent_unk=True, merge_consecutive_unk=False
    )
    assert piece == [1, 0, 0, 0], piece


def craft_unigram_with_user_defined() -> bytes:
    out = bytearray()
    out += _piece("<unk>", 0.0, 2)
    out += _piece("\u2581", -10.0)
    out += _piece("a", -5.0)
    out += _piece("b", -5.0)
    out += _piece("ab", -0.1)
    out += _piece("\u2581a", -2.0)
    out += _piece("<foo>", 0.0, 4)  # USER_DEFINED
    out += _piece("f", -5.0)
    out += _piece("o", -5.0)
    out += _piece("<", -5.0)
    out += _piece(">", -5.0)
    out += _pb_len_delim(2, _pb_varint((3 << 3) | 0) + _pb_varint(1))  # UNIGRAM
    out += _pb_len_delim(3, _norm_spec())
    return bytes(out)


def test_user_defined_matches_sentencepiece(guard, tmp_path: Path) -> None:
    model_path = tmp_path / "ud.model"
    model_path.write_bytes(craft_unigram_with_user_defined())
    sp = sentencepiece.SentencePieceProcessor(model_file=str(model_path))
    for text in ("<foo>", "<foo>ab", "a<foo>b"):
        official = list(sp.encode(text, out_type=int))
        ours = guard.sp_encode(str(model_path), text)
        assert ours == official, (text, ours, official)


def test_nbest_matches_sentencepiece_order(guard, tmp_path: Path) -> None:
    model_path = tmp_path / "ug.model"
    model_path.write_bytes(craft_unigram_unique_optimum())
    sp = sentencepiece.SentencePieceProcessor(model_file=str(model_path))
    official = [list(row) for row in sp.nbest_encode("ab", nbest_size=5, out_type=int)]
    ours = [row["tokens"] for row in guard.sp_nbest_encode(str(model_path), "ab", 5)]
    assert ours == official
    # 1-best equals encode
    assert ours[0] == guard.sp_encode(str(model_path), "ab")


def test_sample_encode_from_nbest_is_member(guard, tmp_path: Path) -> None:
    model_path = tmp_path / "ug.model"
    model_path.write_bytes(craft_unigram_unique_optimum())
    nbest = {tuple(r["tokens"]) for r in guard.sp_nbest_encode(str(model_path), "ab", 5)}
    sampled = tuple(
        guard.sp_sample_encode(str(model_path), "ab", nbest_size=5, alpha=1.0, seed=99)
    )
    assert sampled in nbest


def test_word_matches_sentencepiece(guard, tmp_path: Path) -> None:
    model_path = tmp_path / "word.model"
    model_path.write_bytes(craft_word_model())
    sp = sentencepiece.SentencePieceProcessor(model_file=str(model_path))
    official = list(sp.encode("hello world", out_type=int))
    ours = guard.sp_encode(str(model_path), "hello world")
    assert ours == official
    assert guard.sp_decode(str(model_path), ours) == sp.decode(official)


def test_treat_whitespace_as_suffix_matches_sentencepiece(guard, tmp_path: Path) -> None:
    """Normalize + UNIGRAM encode with suffix ▁ matches official SP.

    Note: official ``word_model.cc`` calls ``SplitIntoWords(normalized)`` with
    default ``treat_ws_as_suffix=false``, so WORD *inference* ignores the trainer
    flag. We apply the flag in WORD split (trainer-compatible). Cross-check the
    flag via UNIGRAM, where segmentation does not depend on SplitIntoWords.
    """
    out = bytearray()
    out += _piece("<unk>", 0.0, 2)
    out += _piece("h", -2.0)
    out += _piece("e", -2.0)
    out += _piece("l", -2.0)
    out += _piece("o", -2.0)
    out += _piece("hello\u2581", -0.1)
    out += _piece("w", -2.0)
    out += _piece("r", -2.0)
    out += _piece("d", -2.0)
    out += _piece("world\u2581", -0.1)
    trainer = (
        _pb_varint((3 << 3) | 0)
        + _pb_varint(1)  # UNIGRAM
        + _pb_varint((24 << 3) | 0)
        + _pb_varint(1)  # treat_whitespace_as_suffix
    )
    out += _pb_len_delim(2, trainer)
    out += _pb_len_delim(3, _norm_spec())
    model_path = tmp_path / "ug_suffix.model"
    model_path.write_bytes(bytes(out))

    sp = sentencepiece.SentencePieceProcessor(model_file=str(model_path))
    for text in ("hello", "hello world"):
        official = list(sp.encode(text, out_type=int))
        ours = guard.sp_encode(str(model_path), text)
        assert ours == official, (text, ours, official)

    # WORD + suffix: our SplitIntoWords honors the trainer flag (official WORD
    # inference does not). Sanity-check encode succeeds for suffix vocab.
    word_path = tmp_path / "word_suffix.model"
    word_path.write_bytes(craft_word_suffix_ws())
    ours_word = guard.sp_encode(str(word_path), "hello world")
    assert ours_word == [1, 2], ours_word


def test_control_decode_matches_sentencepiece(guard, tmp_path: Path) -> None:
    model_path = tmp_path / "ctrl.model"
    model_path.write_bytes(craft_control_symbols())
    sp = sentencepiece.SentencePieceProcessor(model_file=str(model_path))
    assert sp.bos_id() == 1
    assert sp.eos_id() == 2
    ids = [1] + list(sp.encode("a", out_type=int)) + [2]
    official = sp.decode(ids)
    ours = guard.sp_decode(str(model_path), ids)
    assert ours == official == "a"


def test_sample_encode_and_score_with_replacement(guard, tmp_path: Path) -> None:
    model_path = tmp_path / "ug.model"
    model_path.write_bytes(craft_unigram_unique_optimum())
    rows = guard.sp_sample_encode_and_score(
        str(model_path), "ab", num_samples=4, alpha=0.5, seed=3
    )
    assert len(rows) == 4
    for row in rows:
        assert row["tokens"]
        assert isinstance(row["score"], float)


def test_sample_encode_and_score_wor(guard, tmp_path: Path) -> None:
    """WOR: unique samples; include_best puts Viterbi first with score 0."""
    model_path = tmp_path / "ug.model"
    model_path.write_bytes(craft_unigram_unique_optimum())
    best = guard.sp_encode(str(model_path), "ab")
    rows = guard.sp_sample_encode_and_score(
        str(model_path),
        "ab",
        num_samples=3,
        alpha=0.5,
        wor=True,
        include_best=True,
        seed=11,
    )
    assert rows[0]["tokens"] == best
    assert rows[0]["score"] == 0.0
    seen = set()
    for row in rows:
        key = tuple(row["tokens"])
        assert key not in seen
        seen.add(key)
        assert isinstance(row["score"], float)

    # Cross-check vs official SP: same token sets appear in n-best; scores finite.
    sp = sentencepiece.SentencePieceProcessor(model_file=str(model_path))
    official = sp.sample_encode_and_score(
        "ab", num_samples=3, alpha=0.5, wor=True, include_best=True, out_type=int
    )
    assert list(official[0][0]) == best
    assert float(official[0][1]) == 0.0
    # Our seeded draw need not match SP's global MT; membership in n-best holds.
    nbest = {tuple(r) for r in sp.nbest_encode("ab", nbest_size=8, out_type=int)}
    for row in rows:
        assert tuple(row["tokens"]) in nbest


def test_bpe_dropout_variants(guard, tmp_path: Path) -> None:
    model_path = tmp_path / "bpe.model"
    model_path.write_bytes(craft_bpe_unique_optimum())
    det = guard.sp_encode(str(model_path), "ab")
    sp = sentencepiece.SentencePieceProcessor(model_file=str(model_path))
    assert det == list(sp.encode("ab", out_type=int))
    seen: set[tuple[int, ...]] = set()
    for seed in range(40):
        ids = tuple(
            guard.sp_sample_encode(
                str(model_path), "ab", alpha=0.5, seed=seed, nbest_size=-1
            )
        )
        seen.add(ids)
    assert len(seen) >= 2, f"expected dropout diversity, got {seen}"
    off_seen = set()
    for _ in range(50):
        off_seen.add(tuple(sp.encode("ab", out_type=int, enable_sampling=True, alpha=0.5)))
    assert seen & off_seen, f"no overlap with official dropout variants: {seen} vs {off_seen}"


def test_word_official_split_compat(guard, tmp_path: Path) -> None:
    """word_official_split uses flag-blind SplitIntoWords (stock word_model.cc).

    Default encode also merges consecutive ``<unk>`` like stock
    ``SentencePieceProcessor`` (disable with ``merge_consecutive_unk=False``).
    """
    model_path = tmp_path / "word_suffix.model"
    model_path.write_bytes(craft_word_suffix_ws())
    ours = guard.sp_encode(str(model_path), "hello world")
    assert ours == [1, 2]
    sp = sentencepiece.SentencePieceProcessor(model_file=str(model_path))
    official = list(sp.encode("hello world", out_type=int))
    compat = guard.sp_encode(
        str(model_path), "hello world", word_official_split=True, allow_silent_unk=True
    )
    assert compat == official
    piece = guard.sp_encode(
        str(model_path),
        "hello world",
        word_official_split=True,
        allow_silent_unk=True,
        merge_consecutive_unk=False,
    )
    assert piece == [0, 0, 0]


def test_consecutive_unk_merge_crosscheck(guard, tmp_path: Path) -> None:
    """Processor-level consecutive-unk merge vs official SentencePiece."""
    model_path = tmp_path / "char_lim.model"
    model_path.write_bytes(craft_char_limited_vocab())
    sp = sentencepiece.SentencePieceProcessor(model_file=str(model_path))
    for text in ("xyz", "xay", "zzazz", "a", "hello"):
        official = list(sp.encode(text, out_type=int))
        ours = guard.sp_encode(str(model_path), text, allow_silent_unk=True)
        assert ours == official, (text, ours, official)


def test_nfkc_normalize_goldens_vs_live_and_guardd(tmp_path: Path, repo_root: Path) -> None:
    """Curated corpus vs live SP Normalizer; nfkc/nfkc_cf charsmap through guardd.

    Not an exhaustive Unicode NFKC audit. Full nfkc blobs (~240KB) are loaded from
    the installed sentencepiece package at test time (size-aware: not checked in).
    Wave 27 expanded the curated set beyond 100/rule (compat forms, NFD/NFC pairs).

    Note: ``SentencePieceNormalizer`` goldens use charsmap-only flags
    (no dummy-prefix / ▁ escape). They are not compared to encode-path models.
    """
    gold_path = FIXTURES / "sp_nfkc_normalize_goldens.json"
    assert gold_path.is_file()
    gold = json.loads(gold_path.read_text(encoding="utf-8"))
    counts = {rule: len(rows) for rule, rows in gold["rule"].items()}
    assert set(counts) == {"identity", "nfkc", "nfkc_cf"}
    assert len(set(counts.values())) == 1
    assert next(iter(counts.values())) > 100

    for rule, rows in gold["rule"].items():
        live = sentencepiece.SentencePieceNormalizer(rule_name=rule)
        for row in rows:
            assert live.normalize(row["in"]) == row["out"], (rule, row)

    import subprocess

    cli = repo_root / "src" / "rust" / "guardd" / "target" / "debug" / "guardd_perfect_hash.exe"
    if not cli.is_file():
        cli = repo_root / "src" / "rust" / "guardd" / "target" / "debug" / "guardd_perfect_hash"
    if not cli.is_file():
        pytest.skip("guardd_perfect_hash binary not built")

    # Embed live nfkc / nfkc_cf NormalizerSpec (charsmap-only flags) into a tiny model.
    for rule in ("nfkc", "nfkc_cf"):
        live = sentencepiece.SentencePieceNormalizer(rule_name=rule)
        spec = live.serialized_normalizer_spec()
        pieces = [("<unk>", 0.0, 2), ("\u2581", 0.0, 1)]
        for ch in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789":
            pieces.append((ch, 0.0, 1))
        body = bytearray()
        for text, score, typ in pieces:
            body += _piece(text, score, typ)
        body += _pb_len_delim(2, _pb_varint((3 << 3) | 0) + _pb_varint(4))  # CHAR
        body += _pb_len_delim(3, spec)
        model_path = tmp_path / f"{rule}_norm.model"
        model_path.write_bytes(bytes(body))

        for row in gold["rule"][rule]:
            proc = subprocess.run(
                [str(cli), "sp-normalize", str(model_path), row["in"]],
                capture_output=True,
                text=True,
                encoding="utf-8",
                check=False,
            )
            assert proc.returncode == 0, (rule, row, proc.stderr)
            assert proc.stdout.rstrip("\n") == row["out"], (rule, row, proc.stdout)
