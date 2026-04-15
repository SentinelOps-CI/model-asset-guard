#!/usr/bin/env python3
"""
Perfect Hash Tokenizer Integration Test

This script tests the complete perfect hash tokenizer integration including:
- Python bindings
- Node.js bindings
- Rust CLI tools
- Lean CLI tools
- End-to-end functionality
"""

import os
import json
import tempfile
import subprocess
import sys
from pathlib import Path
from typing import Dict, Any


def create_test_vocab() -> Dict[str, Any]:
    """Create a test vocabulary for perfect hash tokenizer"""
    return {
        "words": [
            "hello",
            "world",
            "test",
            "perfect",
            "hash",
            "tokenizer",
            "model",
            "asset",
            "guard",
            "rust",
            "lean",
            "python",
        ],
        "hash_table": [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
    }


def test_rust_cli() -> Dict[str, Any]:
    """Test Rust CLI tool for perfect hash generation"""
    print("Testing Rust CLI tool...")

    # Create test vocab file
    test_vocab = create_test_vocab()
    with tempfile.NamedTemporaryFile(mode="w", suffix=".json", delete=False) as f:
        json.dump(test_vocab, f)
        vocab_file = f.name

    try:
        # Test perfect hash generation
        result = subprocess.run(
            [
                "cargo",
                "run",
                "--bin",
                "gen_perfect_hash",
                "--manifest-path",
                "src/rust/guardd/Cargo.toml",
                vocab_file,
                "--output",
                "test_perfect_hash.json",
            ],
            capture_output=True,
            text=True,
            timeout=30,
        )

        success = result.returncode == 0
        output_file_exists = os.path.exists("test_perfect_hash.json")

        if output_file_exists:
            with open("test_perfect_hash.json", "r") as f:
                generated_vocab = json.load(f)
            os.remove("test_perfect_hash.json")
        else:
            generated_vocab = None

        print(f"  Rust CLI test: {'✓' if success and output_file_exists else '✗'}")

        return {
            "test": "rust_cli",
            "success": success and output_file_exists,
            "output": result.stdout,
            "error": result.stderr,
            "generated_vocab": generated_vocab,
        }
    except subprocess.TimeoutExpired:
        return {"test": "rust_cli", "success": False, "error": "Timeout"}
    finally:
        if os.path.exists(vocab_file):
            os.remove(vocab_file)


def test_python_bindings() -> Dict[str, Any]:
    """Test Python bindings for perfect hash tokenizer"""
    print("Testing Python bindings...")

    try:
        # Import the Python bindings
        repo_root = Path(__file__).resolve().parents[2]
        sys.path.insert(0, str(repo_root / "bindings" / "python"))
        from pytorch_guard import ModelAssetGuard

        # Create guard instance
        guard = ModelAssetGuard()

        # Create test vocabulary
        test_vocab = create_test_vocab()
        vocab_json = json.dumps(test_vocab)

        # Test encoding
        test_text = "hello world test"
        try:
            tokens = guard.perfect_hash_encode(vocab_json, test_text)
            encode_success = isinstance(tokens, list) and len(tokens) > 0
            print(f"  Python encode test: {'✓' if encode_success else '✗'}")
        except Exception as e:
            encode_success = False
            print(f"  Python encode test: ✗ ({e})")

        # Test decoding
        try:
            if encode_success and tokens:
                decoded_word = guard.perfect_hash_decode(vocab_json, tokens[0])
                decode_success = isinstance(decoded_word, str) and len(decoded_word) > 0
                print(f"  Python decode test: {'✓' if decode_success else '✗'}")
            else:
                decode_success = False
                print("  Python decode test: ✗ (no tokens to decode)")
        except Exception as e:
            decode_success = False
            print(f"  Python decode test: ✗ ({e})")

        # Test sequence decoding
        try:
            if encode_success and tokens:
                decoded_text = guard.perfect_hash_decode_sequence(vocab_json, tokens)
                sequence_success = (
                    isinstance(decoded_text, str) and len(decoded_text) > 0
                )
                print(
                    f"  Python sequence decode test: {'✓' if sequence_success else '✗'}"
                )
            else:
                sequence_success = False
                print("  Python sequence decode test: ✗ (no tokens to decode)")
        except Exception as e:
            sequence_success = False
            print(f"  Python sequence decode test: ✗ ({e})")

        overall_success = encode_success and decode_success and sequence_success

        return {
            "test": "python_bindings",
            "success": overall_success,
            "encode_success": encode_success,
            "decode_success": decode_success,
            "sequence_success": sequence_success,
            "tokens": tokens if encode_success else None,
            "decoded_word": decoded_word if decode_success else None,
            "decoded_text": decoded_text if sequence_success else None,
        }
    except ImportError as e:
        print(f"  Python bindings test: ✗ (Import error: {e})")
        return {
            "test": "python_bindings",
            "success": False,
            "error": f"Import error: {e}",
        }
    except Exception as e:
        print(f"  Python bindings test: ✗ ({e})")
        return {"test": "python_bindings", "success": False, "error": str(e)}


def test_nodejs_bindings() -> Dict[str, Any]:
    """Test Node.js bindings for perfect hash tokenizer"""
    print("Testing Node.js bindings...")

    # Create test script
    repo_root = Path(__file__).resolve().parents[2]
    node_guard_path = str((repo_root / "bindings" / "nodejs" / "node_guard.js").resolve()).replace("\\", "/")
    test_script = (
        """
const { ModelAssetGuard } = require('"""
        + node_guard_path
        + """');

async function testPerfectHash() {
    try {
        const guard = new ModelAssetGuard();
        
        const testVocab = {
            words: ["hello", "world", "test", "perfect", "hash", "tokenizer"],
            hash_table: [0, 1, 2, 3, 4, 5]
        };
        
        const vocabJson = JSON.stringify(testVocab);
        const testText = "hello world test";
        
        // Test encoding
        const tokens = guard.perfectHashEncode(vocabJson, testText);
        console.log('ENCODE_SUCCESS:', Array.isArray(tokens) && tokens.length > 0);
        
        // Test decoding
        if (tokens && tokens.length > 0) {
            const decodedWord = guard.perfectHashDecode(vocabJson, tokens[0]);
            console.log('DECODE_SUCCESS:', typeof decodedWord === 'string' && decodedWord.length > 0);
            
            // Test sequence decoding
            const decodedText = guard.perfectHashDecodeSequence(vocabJson, tokens);
            console.log('SEQUENCE_SUCCESS:', typeof decodedText === 'string' && decodedText.length > 0);
        } else {
            console.log('DECODE_SUCCESS: false');
            console.log('SEQUENCE_SUCCESS: false');
        }
        
    } catch (error) {
        console.log('ERROR:', error.message);
    }
}

testPerfectHash();
"""
    )

    with tempfile.NamedTemporaryFile(mode="w", suffix=".js", delete=False) as f:
        f.write(test_script)
        test_file = f.name

    try:
        result = subprocess.run(
            ["node", test_file], capture_output=True, text=True, timeout=30
        )

        output = result.stdout.strip()
        encode_success = "ENCODE_SUCCESS: true" in output
        decode_success = "DECODE_SUCCESS: true" in output
        sequence_success = "SEQUENCE_SUCCESS: true" in output
        has_error = "ERROR:" in output

        overall_success = (
            encode_success and decode_success and sequence_success and not has_error
        )

        print(f"  Node.js encode test: {'✓' if encode_success else '✗'}")
        print(f"  Node.js decode test: {'✓' if decode_success else '✗'}")
        print(f"  Node.js sequence decode test: {'✓' if sequence_success else '✗'}")

        return {
            "test": "nodejs_bindings",
            "success": overall_success,
            "encode_success": encode_success,
            "decode_success": decode_success,
            "sequence_success": sequence_success,
            "output": output,
            "error": result.stderr,
        }
    except subprocess.TimeoutExpired:
        return {"test": "nodejs_bindings", "success": False, "error": "Timeout"}
    finally:
        if os.path.exists(test_file):
            os.remove(test_file)


def test_lean_cli() -> Dict[str, Any]:
    """Test Lean CLI tool for perfect hash tokenizer"""
    print("Testing Lean CLI tool...")

    # Create test vocabulary file
    test_vocab = create_test_vocab()
    with tempfile.NamedTemporaryFile(mode="w", suffix=".json", delete=False) as f:
        json.dump(test_vocab, f)
        vocab_file = f.name

    try:
        # Test encode command
        result = subprocess.run(
            ["lake", "exe", "perfecthash", "encode", vocab_file, "hello world"],
            capture_output=True,
            text=True,
            timeout=30,
        )

        encode_success = result.returncode == 0
        print(f"  Lean CLI encode test: {'✓' if encode_success else '✗'}")

        # Test decode command
        result = subprocess.run(
            ["lake", "exe", "perfecthash", "decode", vocab_file, "0"],
            capture_output=True,
            text=True,
            timeout=30,
        )

        decode_success = result.returncode == 0
        print(f"  Lean CLI decode test: {'✓' if decode_success else '✗'}")

        # Test test command
        result = subprocess.run(
            ["lake", "exe", "perfecthash", "test", vocab_file],
            capture_output=True,
            text=True,
            timeout=30,
        )

        test_success = result.returncode == 0
        print(f"  Lean CLI test command: {'✓' if test_success else '✗'}")

        overall_success = encode_success and decode_success and test_success

        return {
            "test": "lean_cli",
            "success": overall_success,
            "encode_success": encode_success,
            "decode_success": decode_success,
            "test_success": test_success,
            "output": result.stdout,
            "error": result.stderr,
        }
    except subprocess.TimeoutExpired:
        return {"test": "lean_cli", "success": False, "error": "Timeout"}
    finally:
        if os.path.exists(vocab_file):
            os.remove(vocab_file)


def test_end_to_end() -> Dict[str, Any]:
    """Test end-to-end perfect hash tokenizer workflow"""
    print("Testing end-to-end workflow...")

    try:
        # Step 1: Generate perfect hash vocabulary
        test_vocab = create_test_vocab()
        with tempfile.NamedTemporaryFile(mode="w", suffix=".json", delete=False) as f:
            json.dump(test_vocab, f)
            input_vocab_file = f.name

        output_vocab_file = "e2e_perfect_hash.json"

        # Generate perfect hash
        result = subprocess.run(
            [
                "cargo",
                "run",
                "--bin",
                "gen_perfect_hash",
                "--manifest-path",
                "src/rust/guardd/Cargo.toml",
                input_vocab_file,
                "--output",
                output_vocab_file,
            ],
            capture_output=True,
            text=True,
            timeout=30,
        )

        if result.returncode != 0:
            return {
                "test": "end_to_end",
                "success": False,
                "error": "Failed to generate perfect hash",
            }

        # Step 2: Test with Python bindings
        from pytorch_guard import ModelAssetGuard

        guard = ModelAssetGuard()

        with open(output_vocab_file, "r") as f:
            generated_vocab = json.load(f)

        vocab_json = json.dumps(generated_vocab)
        test_text = "hello world perfect hash"

        # Encode
        tokens = guard.perfect_hash_encode(vocab_json, test_text)
        if not tokens:
            return {"test": "end_to_end", "success": False, "error": "Encoding failed"}

        # Decode
        decoded_text = guard.perfect_hash_decode_sequence(vocab_json, tokens)
        if not decoded_text:
            return {"test": "end_to_end", "success": False, "error": "Decoding failed"}

        # Verify determinism (simplified check)
        determinism_ok = len(decoded_text.split()) == len(test_text.split())

        print(f"  End-to-end workflow: {'✓' if determinism_ok else '✗'}")

        # Cleanup
        if os.path.exists(output_vocab_file):
            os.remove(output_vocab_file)
        if os.path.exists(input_vocab_file):
            os.remove(input_vocab_file)

        return {
            "test": "end_to_end",
            "success": determinism_ok,
            "original_text": test_text,
            "tokens": tokens,
            "decoded_text": decoded_text,
            "determinism_ok": determinism_ok,
        }
    except Exception as e:
        return {"test": "end_to_end", "success": False, "error": str(e)}


def main():
    """Run all perfect hash integration tests"""
    print("Perfect Hash Tokenizer Integration Test")
    print("=" * 50)

    tests = [
        test_rust_cli,
        test_python_bindings,
        test_nodejs_bindings,
        test_lean_cli,
        test_end_to_end,
    ]

    results = []
    for test_func in tests:
        result = test_func()
        results.append(result)
        print()

    # Summary
    passed = sum(1 for r in results if r["success"])
    total = len(results)

    print("=" * 50)
    print(f"Test Summary: {passed}/{total} tests passed")

    if passed == total:
        print("✓ All perfect hash integration tests passed!")
    else:
        print("✗ Some tests failed:")
        for result in results:
            if not result["success"]:
                print(f"  - {result['test']}: {result.get('error', 'Unknown error')}")

    # Save results
    with open("perfect_hash_test_results.json", "w") as f:
        json.dump(
            {"total_tests": total, "passed_tests": passed, "results": results},
            f,
            indent=2,
        )

    print("\nResults saved to perfect_hash_test_results.json")

    # Exit with appropriate code
    if passed == total:
        exit(0)
    else:
        exit(1)


if __name__ == "__main__":
    main()
