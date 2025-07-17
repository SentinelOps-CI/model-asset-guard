#!/usr/bin/env python3
"""
Test HuggingFace Transformers Integration (G-3)

This script tests the complete HuggingFace transformers integration
for Model Asset Guard, validating the G-3 requirement implementation.

Requirements:
- transformers >= 4.20.0
- torch >= 1.12.0
- numpy >= 1.21.0
- pytest (for testing)
"""

import os
import sys
import json
import tempfile
import hashlib
import subprocess
from pathlib import Path
from typing import Dict, Any, Optional

# Add the current directory to Python path
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

try:
    import pytest
    import torch
    import numpy as np
    from transformers import AutoModel, AutoTokenizer, GPT2LMHeadModel, GPT2Tokenizer
    from pytorch_guard import ModelAssetGuard, HuggingFaceGuard, checked_load_pretrained

    TRANSFORMERS_AVAILABLE = True
except ImportError as e:
    print(f"Warning: Some dependencies not available: {e}")
    TRANSFORMERS_AVAILABLE = False


class HuggingFaceIntegrationTester:
    """Comprehensive tester for HuggingFace transformers integration"""

    def __init__(self):
        self.test_results = []
        self.temp_files = []
        self.guard = None
        self.hf_guard = None

    def setup(self):
        """Setup test environment"""
        print("Setting up HuggingFace integration test environment...")

        try:
            # Initialize Model Asset Guard
            self.guard = ModelAssetGuard()
            self.hf_guard = HuggingFaceGuard(self.guard)
            print("✓ Model Asset Guard initialized successfully")
        except Exception as e:
            print(f"✗ Failed to initialize Model Asset Guard: {e}")
            return False

        return True

    def cleanup(self):
        """Clean up test resources"""
        for temp_file in self.temp_files:
            try:
                os.remove(temp_file)
            except:
                pass

    def create_test_file(self, content: str) -> str:
        """Create a temporary test file"""
        fd, path = tempfile.mkstemp()
        os.write(fd, content.encode())
        os.close(fd)
        self.temp_files.append(path)
        return path

    def test_basic_guard_functionality(self) -> Dict[str, Any]:
        """Test basic Model Asset Guard functionality"""
        print("\n=== Testing Basic Guard Functionality ===")

        results = {
            "test_name": "basic_guard_functionality",
            "passed": False,
            "errors": [],
        }

        try:
            # Create test file
            test_content = "Hello, Model Asset Guard!"
            test_file = self.create_test_file(test_content)

            # Compute expected digest
            expected_digest = hashlib.sha256(test_content.encode()).digest()

            # Test digest verification
            verified = self.guard.verify_digest(test_file, expected_digest)
            if not verified:
                results["errors"].append("Digest verification failed")

            # Test checked_load
            file_info = self.guard.checked_load(test_file, expected_digest)
            if not file_info["valid"]:
                results["errors"].append("Checked load validation failed")

            # Test bit-flip corpus
            corpus_result = self.guard.run_bitflip_corpus_test(
                file_size_gb=0.001,  # Small test
                num_corruptions=5,
                temp_dir=tempfile.gettempdir(),
            )

            if not isinstance(corpus_result, dict):
                results["errors"].append("Bit-flip corpus test failed")

            results["passed"] = len(results["errors"]) == 0
            print(
                f"✓ Basic functionality test: {'PASSED' if results['passed'] else 'FAILED'}"
            )

        except Exception as e:
            results["errors"].append(f"Exception: {str(e)}")
            print(f"✗ Basic functionality test failed: {e}")

        return results

    def test_quantization_verification(self) -> Dict[str, Any]:
        """Test quantization verification with 128 vectors"""
        print("\n=== Testing Quantization Verification ===")

        results = {
            "test_name": "quantization_verification",
            "passed": False,
            "errors": [],
        }

        try:
            # Create test weights
            fan_in, fan_out = 256, 512
            weights = np.random.randn(fan_in, fan_out).astype(np.float32)

            # Test single layer verification
            layer_result = self.guard.verify_quantization_128_vectors(
                layer_name="test_layer",
                weights=weights,
                fan_in=fan_in,
                fan_out=fan_out,
                quant_type="int8",
            )

            if not isinstance(layer_result, dict):
                results["errors"].append("Single layer verification failed")

            # Test model verification
            layers = [
                {
                    "name": "test_layer",
                    "weights": weights.flatten().tolist(),
                    "fan_in": fan_in,
                    "fan_out": fan_out,
                    "quant_type": "int8",
                }
            ]

            model_result = self.guard.verify_model_quantization(layers)

            if not isinstance(model_result, dict):
                results["errors"].append("Model verification failed")

            results["passed"] = len(results["errors"]) == 0
            print(
                f"✓ Quantization verification test: {'PASSED' if results['passed'] else 'FAILED'}"
            )

        except Exception as e:
            results["errors"].append(f"Exception: {str(e)}")
            print(f"✗ Quantization verification test failed: {e}")

        return results

    def test_huggingface_integration(self) -> Dict[str, Any]:
        """Test HuggingFace transformers integration"""
        print("\n=== Testing HuggingFace Integration ===")

        results = {
            "test_name": "huggingface_integration",
            "passed": False,
            "errors": [],
        }

        if not TRANSFORMERS_AVAILABLE:
            results["errors"].append("Transformers not available")
            print("✗ HuggingFace integration test skipped (transformers not available)")
            return results

        try:
            # Test with a small model (GPT-2)
            print("Loading GPT-2 model with verification...")

            model, tokenizer, verification_results = (
                self.hf_guard.checked_load_pretrained("gpt2", verify_quantization=True)
            )

            # Check verification results
            if not verification_results.get("verified", False):
                results["errors"].append("Model verification failed")

            # Test tokenizer determinism
            tokenizer_results = self.hf_guard.verify_tokenizer_determinism(
                tokenizer, num_tests=100  # Small test for speed
            )

            if tokenizer_results["failed"] > 0:
                results["errors"].append(
                    f"Tokenizer determinism test failed: {tokenizer_results['failed']} failures"
                )

            results["passed"] = len(results["errors"]) == 0
            print(
                f"✓ HuggingFace integration test: {'PASSED' if results['passed'] else 'FAILED'}"
            )

        except Exception as e:
            results["errors"].append(f"Exception: {str(e)}")
            print(f"✗ HuggingFace integration test failed: {e}")

        return results

    def test_convenience_functions(self) -> Dict[str, Any]:
        """Test convenience functions"""
        print("\n=== Testing Convenience Functions ===")

        results = {"test_name": "convenience_functions", "passed": False, "errors": []}

        if not TRANSFORMERS_AVAILABLE:
            results["errors"].append("Transformers not available")
            print("✗ Convenience functions test skipped (transformers not available)")
            return results

        try:
            # Test create_guard function
            guard = ModelAssetGuard()
            if not guard:
                results["errors"].append("create_guard failed")

            # Test create_hf_guard function
            hf_guard = HuggingFaceGuard(guard)
            if not hf_guard:
                results["errors"].append("create_hf_guard failed")

            # Test checked_load_pretrained convenience function
            model, tokenizer, verification_results = checked_load_pretrained(
                "gpt2", verify_quantization=False  # Skip for speed
            )

            if not model or not tokenizer:
                results["errors"].append("checked_load_pretrained failed")

            results["passed"] = len(results["errors"]) == 0
            print(
                f"✓ Convenience functions test: {'PASSED' if results['passed'] else 'FAILED'}"
            )

        except Exception as e:
            results["errors"].append(f"Exception: {str(e)}")
            print(f"✗ Convenience functions test failed: {e}")

        return results

    def test_error_handling(self) -> Dict[str, Any]:
        """Test error handling"""
        print("\n=== Testing Error Handling ===")

        results = {"test_name": "error_handling", "passed": False, "errors": []}

        try:
            # Test invalid digest
            test_file = self.create_test_file("test content")
            invalid_digest = b"invalid" * 4  # 32 bytes but wrong

            try:
                self.guard.verify_digest(test_file, invalid_digest)
                results["errors"].append("Should have failed with invalid digest")
            except:
                pass  # Expected to fail

            # Test non-existent file
            try:
                self.guard.verify_digest("/non/existent/file", b"0" * 32)
                results["errors"].append("Should have failed with non-existent file")
            except:
                pass  # Expected to fail

            # Test invalid digest length
            try:
                self.guard.verify_digest(test_file, b"short")
                results["errors"].append("Should have failed with short digest")
            except ValueError:
                pass  # Expected to fail

            results["passed"] = len(results["errors"]) == 0
            print(
                f"✓ Error handling test: {'PASSED' if results['passed'] else 'FAILED'}"
            )

        except Exception as e:
            results["errors"].append(f"Exception: {str(e)}")
            print(f"✗ Error handling test failed: {e}")

        return results

    def test_performance(self) -> Dict[str, Any]:
        """Test performance characteristics"""
        print("\n=== Testing Performance ===")

        results = {
            "test_name": "performance_test",
            "passed": False,
            "errors": [],
            "metrics": {},
        }

        try:
            import time

            # Test digest verification performance
            test_content = "x" * (1024 * 1024)  # 1MB
            test_file = self.create_test_file(test_content)
            expected_digest = hashlib.sha256(test_content.encode()).digest()

            start_time = time.time()
            for _ in range(10):
                self.guard.verify_digest(test_file, expected_digest)
            end_time = time.time()

            avg_time = (end_time - start_time) / 10
            throughput = (1024 * 1024) / avg_time  # MB/s

            results["metrics"]["digest_throughput_mbps"] = throughput

            if throughput < 100:  # Target: > 100 MB/s
                results["errors"].append(
                    f"Digest throughput too low: {throughput:.2f} MB/s"
                )

            # Test quantization verification performance
            fan_in, fan_out = 512, 512
            weights = np.random.randn(fan_in, fan_out).astype(np.float32)

            start_time = time.time()
            self.guard.verify_quantization_128_vectors(
                "test_layer", weights, fan_in, fan_out, "int8"
            )
            end_time = time.time()

            layer_time = end_time - start_time
            results["metrics"]["quantization_time_seconds"] = layer_time

            if layer_time > 1.0:  # Target: < 1 second
                results["errors"].append(
                    f"Quantization verification too slow: {layer_time:.2f}s"
                )

            results["passed"] = len(results["errors"]) == 0
            print(f"✓ Performance test: {'PASSED' if results['passed'] else 'FAILED'}")
            print(f"  Digest throughput: {throughput:.2f} MB/s")
            print(f"  Quantization time: {layer_time:.2f}s")

        except Exception as e:
            results["errors"].append(f"Exception: {str(e)}")
            print(f"✗ Performance test failed: {e}")

        return results

    def run_all_tests(self) -> Dict[str, Any]:
        """Run all tests and return comprehensive results"""
        print("Starting HuggingFace Transformers Integration Tests (G-3)")
        print("=" * 60)

        if not self.setup():
            return {"error": "Failed to setup test environment"}

        try:
            # Run all tests
            tests = [
                self.test_basic_guard_functionality,
                self.test_quantization_verification,
                self.test_huggingface_integration,
                self.test_convenience_functions,
                self.test_error_handling,
                self.test_performance,
            ]

            for test_func in tests:
                result = test_func()
                self.test_results.append(result)

            # Compile results
            total_tests = len(self.test_results)
            passed_tests = sum(1 for r in self.test_results if r.get("passed", False))

            summary = {
                "total_tests": total_tests,
                "passed_tests": passed_tests,
                "failed_tests": total_tests - passed_tests,
                "success_rate": passed_tests / total_tests if total_tests > 0 else 0,
                "test_results": self.test_results,
                "overall_status": "PASSED" if passed_tests == total_tests else "FAILED",
            }

            print("\n" + "=" * 60)
            print("TEST SUMMARY")
            print("=" * 60)
            print(f"Total tests: {total_tests}")
            print(f"Passed: {passed_tests}")
            print(f"Failed: {total_tests - passed_tests}")
            print(f"Success rate: {summary['success_rate']:.1%}")
            print(f"Overall status: {summary['overall_status']}")

            # Print detailed results
            for result in self.test_results:
                status = "PASSED" if result.get("passed", False) else "FAILED"
                print(f"\n{result['test_name']}: {status}")
                if result.get("errors"):
                    for error in result["errors"]:
                        print(f"  - {error}")
                if result.get("metrics"):
                    for key, value in result["metrics"].items():
                        print(f"  - {key}: {value}")

            return summary

        finally:
            self.cleanup()


def main():
    """Main test runner"""
    tester = HuggingFaceIntegrationTester()
    results = tester.run_all_tests()

    # Save results to file
    with open("huggingface_integration_test_results.json", "w") as f:
        json.dump(results, f, indent=2)

    print(f"\nTest results saved to: huggingface_integration_test_results.json")

    # Exit with appropriate code
    if results.get("overall_status") == "PASSED":
        print("\n🎉 All HuggingFace integration tests passed!")
        return 0
    else:
        print("\n❌ Some HuggingFace integration tests failed!")
        return 1


if __name__ == "__main__":
    exit(main())
