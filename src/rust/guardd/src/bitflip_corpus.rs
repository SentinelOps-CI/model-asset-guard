use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::Instant;

/// Default maximum corpus size without explicit `--allow-large` / opt-in.
pub const MAX_DEFAULT_CORPUS_BYTES: u64 = 64 * 1024 * 1024;

/// Platform temp dir + `bitflip_corpus` subdirectory (Windows-safe; not `/tmp`).
pub fn default_bitflip_temp_dir() -> PathBuf {
    std::env::temp_dir().join("bitflip_corpus")
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BitFlipTestResult {
    pub file_size_bytes: u64,
    pub original_hash: String,
    pub corruption_count: usize,
    pub rejection_count: usize,
    pub rejection_rate: f64,
    pub test_passed: bool,
    pub duration_ms: u64,
    pub throughput_mbps: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CorpusTestSummary {
    pub total_tests: usize,
    pub passed_tests: usize,
    pub failed_tests: usize,
    pub overall_success_rate: f64,
    pub total_duration_ms: u64,
    pub average_throughput_mbps: f64,
    pub test_results: Vec<BitFlipTestResult>,
}

pub struct BitFlipCorpusTester {
    chunk_size_mb: usize,
    temp_dir: PathBuf,
}

impl BitFlipCorpusTester {
    pub fn new(chunk_size_mb: usize, temp_dir: Option<String>) -> Self {
        let temp_dir = temp_dir
            .map(PathBuf::from)
            .unwrap_or_else(default_bitflip_temp_dir);
        Self {
            chunk_size_mb,
            temp_dir,
        }
    }

    /// Ensure the corpus working directory exists (idempotent).
    pub fn ensure_temp_dir(&self) -> Result<(), Box<dyn std::error::Error>> {
        fs::create_dir_all(&self.temp_dir)?;
        Ok(())
    }

    fn path_in_temp(&self, name: &str) -> PathBuf {
        self.temp_dir.join(name)
    }

    /// Generate deterministic random data for reproducible testing
    pub fn generate_deterministic_data(&self, size_bytes: usize, seed: u64) -> Vec<u8> {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut data = Vec::with_capacity(size_bytes);

        for _ in 0..size_bytes {
            data.push(rng.gen::<u8>());
        }

        data
    }

    /// Apply random bit flips to data
    pub fn apply_bit_flips(
        &self,
        data: &[u8],
        flip_probability: f64,
        seed: u64,
    ) -> (Vec<u8>, Vec<usize>) {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut corrupted_data = data.to_vec();
        let mut flipped_positions = Vec::new();

        for (byte_pos, byte) in corrupted_data.iter_mut().enumerate() {
            for bit_pos in 0..8 {
                if rng.gen::<f64>() < flip_probability {
                    *byte ^= 1 << bit_pos;
                    flipped_positions.push(byte_pos * 8 + bit_pos);
                }
            }
        }

        (corrupted_data, flipped_positions)
    }

    /// Create a large test file with specified size
    pub fn create_test_file(
        &self,
        file_path: &str,
        size_bytes: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        println!(
            "Creating test file: {} ({:.2} GB)",
            file_path,
            size_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
        );

        let chunk_size_bytes = self.chunk_size_mb * 1024 * 1024;
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(file_path)?;

        let mut remaining_bytes = size_bytes;
        let mut chunk_num = 0u64;

        while remaining_bytes > 0 {
            let chunk_size = std::cmp::min(chunk_size_bytes, remaining_bytes);
            let chunk_data = self.generate_deterministic_data(chunk_size, chunk_num);

            file.write_all(&chunk_data)?;
            remaining_bytes -= chunk_size;
            chunk_num += 1;

            if chunk_num % 10 == 0 {
                let progress = (size_bytes - remaining_bytes) as f64 / size_bytes as f64 * 100.0;
                println!("  Progress: {:.1}%", progress);
            }
        }

        file.flush()?;
        Ok(())
    }

    /// Compute SHA-256 hash of file
    pub fn compute_file_hash(&self, file_path: &str) -> Result<String, Box<dyn std::error::Error>> {
        let mut file = File::open(file_path)?;
        let mut hasher = Sha256::new();
        let mut buffer = [0; 4096];

        loop {
            let n = file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
        }

        Ok(format!("{:x}", hasher.finalize()))
    }

    /// Test file integrity using our Rust verification function
    pub fn test_file_integrity(
        &self,
        file_path: &str,
        expected_hash: Option<&str>,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let computed_hash = self.compute_file_hash(file_path)?;

        match expected_hash {
            Some(expected) => Ok(computed_hash == expected),
            None => Ok(true), // No expected hash provided, assume valid
        }
    }

    /// Run bit-flip test with an explicit byte size.
    ///
    /// Sizes above [`MAX_DEFAULT_CORPUS_BYTES`] require `allow_large = true`.
    pub fn run_bitflip_test_bytes(
        &self,
        file_size_bytes: u64,
        num_corruptions: usize,
        allow_large: bool,
    ) -> Result<BitFlipTestResult, Box<dyn std::error::Error>> {
        if file_size_bytes == 0 {
            return Err("file size must be > 0".into());
        }
        if !allow_large && file_size_bytes > MAX_DEFAULT_CORPUS_BYTES {
            return Err(format!(
                "refusing corpus size {} bytes (cap {} without allow_large)",
                file_size_bytes, MAX_DEFAULT_CORPUS_BYTES
            )
            .into());
        }
        if num_corruptions == 0 {
            return Err("corruptions must be > 0".into());
        }

        println!(
            "\n=== Running Bit-Flip Test ({:.2} MB) ===",
            file_size_bytes as f64 / (1024.0 * 1024.0)
        );

        let start_time = Instant::now();
        let size_usize = file_size_bytes as usize;

        self.ensure_temp_dir()?;

        // Create test file (PathBuf: works on Windows and Unix)
        let test_file = self.path_in_temp(&format!("test_{file_size_bytes}b.bin"));
        self.create_test_file(test_file.to_str().ok_or("non-UTF-8 temp path")?, size_usize)?;

        // Compute original hash
        println!("Computing original hash...");
        let original_hash =
            self.compute_file_hash(test_file.to_str().ok_or("non-UTF-8 temp path")?)?;

        // Test original file (should pass)
        println!("Testing original file integrity...");
        let original_valid = self.test_file_integrity(
            test_file.to_str().ok_or("non-UTF-8 temp path")?,
            Some(&original_hash),
        )?;

        if !original_valid {
            return Err("Original file failed integrity check".into());
        }

        // Apply bit flips and test
        let mut rejection_count = 0;
        let mut corruption_results = Vec::new();

        for i in 0..num_corruptions {
            println!("Applying corruption {}/{}...", i + 1, num_corruptions);

            // Create corrupted copy
            let corrupted_file =
                self.path_in_temp(&format!("corrupted_{i}_{file_size_bytes}b.bin"));

            // Copy and corrupt
            {
                let mut src = File::open(&test_file)?;
                let mut dst = OpenOptions::new()
                    .create(true)
                    .write(true)
                    .truncate(true)
                    .open(&corrupted_file)?;

                let mut buffer = Vec::new();
                src.read_to_end(&mut buffer)?;

                let (corrupted_data, _flipped_positions) =
                    self.apply_bit_flips(&buffer, 0.0001, i as u64);
                dst.write_all(&corrupted_data)?;
                dst.flush()?;
            }

            // Test corrupted file (should fail)
            let corrupted_valid = self.test_file_integrity(
                corrupted_file.to_str().ok_or("non-UTF-8 temp path")?,
                Some(&original_hash),
            )?;

            if !corrupted_valid {
                rejection_count += 1;
            }

            corruption_results.push((i, !corrupted_valid));

            // Clean up corrupted file
            let _ = fs::remove_file(&corrupted_file);

            println!("  Corruption {}: rejected: {}", i + 1, !corrupted_valid);
        }

        // Clean up test file
        let _ = fs::remove_file(&test_file);

        let duration = start_time.elapsed();
        let duration_ms = duration.as_millis() as u64;
        let secs = duration.as_secs_f64().max(1e-9);
        let throughput_mbps = (file_size_bytes as f64 / (1024.0 * 1024.0)) / secs;
        let rejection_rate = (rejection_count as f64 / num_corruptions as f64) * 100.0;
        let test_passed = rejection_rate == 100.0;

        let result = BitFlipTestResult {
            file_size_bytes,
            original_hash,
            corruption_count: num_corruptions,
            rejection_count,
            rejection_rate,
            test_passed,
            duration_ms,
            throughput_mbps,
        };

        println!("\nTest Results:");
        println!(
            "  File size: {:.2} MB",
            file_size_bytes as f64 / (1024.0 * 1024.0)
        );
        println!("  Duration: {:.2}s", duration.as_secs_f64());
        println!("  Corruptions tested: {}", num_corruptions);
        println!("  Rejections: {}/{}", rejection_count, num_corruptions);
        println!("  Rejection rate: {:.1}%", rejection_rate);
        println!("  Throughput: {:.2} MB/s", throughput_mbps);
        println!(
            "  Test passed: {}",
            if test_passed { "PASS" } else { "FAIL" }
        );

        Ok(result)
    }

    /// Run bit-flip test on a file of specified size in GB.
    ///
    /// `file_size_gb == 0` means 1 MiB (unit-test / CI default).
    /// Sizes above the default cap require `allow_large`.
    pub fn run_bitflip_test(
        &self,
        file_size_gb: usize,
        num_corruptions: usize,
    ) -> Result<BitFlipTestResult, Box<dyn std::error::Error>> {
        let file_size_bytes = if file_size_gb == 0 {
            1024 * 1024
        } else {
            (file_size_gb as u64).saturating_mul(1024 * 1024 * 1024)
        };
        let allow_large = file_size_bytes <= MAX_DEFAULT_CORPUS_BYTES
            || std::env::var("GUARDD_BITFLIP_ALLOW_LARGE").ok().as_deref() == Some("1");
        self.run_bitflip_test_bytes(file_size_bytes, num_corruptions, allow_large)
    }

    /// Run scalability test with small default sizes (CI-safe).
    ///
    /// Large GB suites require `GUARDD_BITFLIP_ALLOW_LARGE=1`.
    pub fn run_scalability_test(&self) -> Result<CorpusTestSummary, Box<dyn std::error::Error>> {
        println!("\n=== Running Scalability Test (capped defaults) ===");

        let allow_large = std::env::var("GUARDD_BITFLIP_ALLOW_LARGE").ok().as_deref() == Some("1");
        // Default: 1 MiB only. Opt-in: 1 GiB + 1 MiB.
        let test_sizes_bytes: Vec<u64> = if allow_large {
            vec![1024 * 1024, 1024 * 1024 * 1024]
        } else {
            vec![1024 * 1024]
        };

        let mut test_results = Vec::new();
        let mut total_duration_ms = 0u64;
        let mut total_throughput = 0.0;
        let mut test_count = 0;

        for &size_bytes in &test_sizes_bytes {
            println!(
                "\nTesting {:.2} MB file...",
                size_bytes as f64 / (1024.0 * 1024.0)
            );
            match self.run_bitflip_test_bytes(size_bytes, 5, allow_large) {
                Ok(result) => {
                    test_results.push(result.clone());
                    total_duration_ms += result.duration_ms;
                    total_throughput += result.throughput_mbps;
                    test_count += 1;
                }
                Err(e) => {
                    println!("Error testing {} bytes: {}", size_bytes, e);
                    test_results.push(BitFlipTestResult {
                        file_size_bytes: size_bytes,
                        original_hash: "".to_string(),
                        corruption_count: 0,
                        rejection_count: 0,
                        rejection_rate: 0.0,
                        test_passed: false,
                        duration_ms: 0,
                        throughput_mbps: 0.0,
                    });
                }
            }
        }

        let passed_tests = test_results.iter().filter(|r| r.test_passed).count();
        let failed_tests = test_results.len() - passed_tests;
        let overall_success_rate = (passed_tests as f64 / test_results.len() as f64) * 100.0;
        let average_throughput = if test_count > 0 {
            total_throughput / test_count as f64
        } else {
            0.0
        };

        Ok(CorpusTestSummary {
            total_tests: test_results.len(),
            passed_tests,
            failed_tests,
            overall_success_rate,
            total_duration_ms,
            average_throughput_mbps: average_throughput,
            test_results,
        })
    }

    /// Run comprehensive bit-flip corpus test (defaults stay under the size cap).
    ///
    /// Set `GUARDD_BITFLIP_ALLOW_LARGE=1` to include a 1 GiB case. There is no
    /// automatic 100 GiB path anymore.
    pub fn run_comprehensive_test(&self) -> Result<CorpusTestSummary, Box<dyn std::error::Error>> {
        println!("{}", "=".repeat(80));
        println!("Bit-Flip Corpus Test for Model Asset Guard (Rust)");
        println!("{}", "=".repeat(80));

        let start_time = Instant::now();
        let summary = self.run_scalability_test()?;
        let total_time = start_time.elapsed();

        let mut summary = summary;
        summary.total_duration_ms = total_time.as_millis() as u64;
        summary.overall_success_rate = if summary.total_tests > 0 {
            (summary.passed_tests as f64 / summary.total_tests as f64) * 100.0
        } else {
            0.0
        };

        let json_result = serde_json::to_string_pretty(&summary)?;
        std::fs::write("bitflip_corpus_rust_results.json", json_result)?;

        println!("\n{}", "=".repeat(80));
        println!("Test Suite Summary (Rust)");
        println!("{}", "=".repeat(80));
        println!("Total time: {:.2}s", total_time.as_secs_f64());
        println!("Total tests: {}", summary.total_tests);
        println!("Passed tests: {}", summary.passed_tests);
        println!("Failed tests: {}", summary.failed_tests);
        println!("Success rate: {:.1}%", summary.overall_success_rate);
        println!(
            "Average throughput: {:.2} MB/s",
            summary.average_throughput_mbps
        );
        println!(
            "All tests passed: {}",
            if summary.failed_tests == 0 {
                "PASS"
            } else {
                "FAIL"
            }
        );
        println!("Results saved to: bitflip_corpus_rust_results.json");

        Ok(summary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_generate_deterministic_data() {
        let tester = BitFlipCorpusTester::new(1, None);
        let data1 = tester.generate_deterministic_data(100, 42);
        let data2 = tester.generate_deterministic_data(100, 42);
        assert_eq!(data1, data2);
    }

    #[test]
    fn test_apply_bit_flips() {
        let tester = BitFlipCorpusTester::new(1, None);
        let original_data = vec![0u8; 100];
        let (corrupted_data, flipped_positions) = tester.apply_bit_flips(&original_data, 0.1, 42);

        assert_ne!(original_data, corrupted_data);
        assert!(!flipped_positions.is_empty());
    }

    #[test]
    fn default_temp_dir_is_platform_temp() {
        let tester = BitFlipCorpusTester::new(1, None);
        let expected = default_bitflip_temp_dir();
        assert_eq!(tester.temp_dir, expected);
        // Must track std::env::temp_dir() (Windows: %TEMP%; Unix: often /tmp).
        assert!(tester.temp_dir.starts_with(std::env::temp_dir()));
        assert_eq!(
            tester.temp_dir.file_name().and_then(|s| s.to_str()),
            Some("bitflip_corpus")
        );
        tester.ensure_temp_dir().unwrap();
        assert!(tester.temp_dir.is_dir());
    }

    #[test]
    fn test_file_creation_and_hash() {
        let temp_dir = tempdir().unwrap();
        let tester =
            BitFlipCorpusTester::new(1, Some(temp_dir.path().to_str().unwrap().to_string()));

        let test_file = temp_dir.path().join("test.bin");
        tester
            .create_test_file(test_file.to_str().unwrap(), 1024)
            .unwrap();

        let hash = tester
            .compute_file_hash(test_file.to_str().unwrap())
            .unwrap();
        assert_eq!(hash.len(), 64); // SHA-256 hex string length
    }

    #[test]
    fn test_small_bitflip_test() {
        let temp_dir = tempdir().unwrap();
        let tester =
            BitFlipCorpusTester::new(1, Some(temp_dir.path().to_str().unwrap().to_string()));

        // Test with 1MB file (much smaller for unit test)
        let result = tester.run_bitflip_test(0, 5).unwrap(); // 0GB = 1MB for testing
        assert!(result.test_passed);
    }

    #[test]
    fn test_size_cap_without_allow_large() {
        let temp_dir = tempdir().unwrap();
        let tester =
            BitFlipCorpusTester::new(1, Some(temp_dir.path().to_str().unwrap().to_string()));
        let oversized = MAX_DEFAULT_CORPUS_BYTES + 1;
        let err = tester
            .run_bitflip_test_bytes(oversized, 1, false)
            .unwrap_err();
        assert!(err.to_string().contains("allow_large"));
    }
}
