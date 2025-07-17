use sha2::{Sha256, Digest};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write, Seek, SeekFrom};
use std::path::Path;
use std::time::{Duration, Instant};
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use serde::{Serialize, Deserialize};

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
    temp_dir: String,
}

impl BitFlipCorpusTester {
    pub fn new(chunk_size_mb: usize, temp_dir: Option<String>) -> Self {
        let temp_dir = temp_dir.unwrap_or_else(|| "/tmp/bitflip_corpus".to_string());
        Self {
            chunk_size_mb,
            temp_dir,
        }
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
    pub fn apply_bit_flips(&self, data: &[u8], flip_probability: f64, seed: u64) -> (Vec<u8>, Vec<usize>) {
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
    pub fn create_test_file(&self, file_path: &str, size_bytes: usize) -> Result<(), Box<dyn std::error::Error>> {
        println!("Creating test file: {} ({:.2} GB)", file_path, size_bytes as f64 / (1024.0 * 1024.0 * 1024.0));
        
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
    pub fn test_file_integrity(&self, file_path: &str, expected_hash: Option<&str>) -> Result<bool, Box<dyn std::error::Error>> {
        let computed_hash = self.compute_file_hash(file_path)?;
        
        match expected_hash {
            Some(expected) => Ok(computed_hash == expected),
            None => Ok(true), // No expected hash provided, assume valid
        }
    }

    /// Run bit-flip test on a file of specified size
    pub fn run_bitflip_test(&self, file_size_gb: usize, num_corruptions: usize) -> Result<BitFlipTestResult, Box<dyn std::error::Error>> {
        println!("\n=== Running {}GB Bit-Flip Test ===", file_size_gb);
        
        let start_time = Instant::now();
        let file_size_bytes = file_size_gb * 1024 * 1024 * 1024;
        
        // Create test file
        let test_file = format!("{}/test_{}gb.bin", self.temp_dir, file_size_gb);
        self.create_test_file(&test_file, file_size_bytes)?;
        
        // Compute original hash
        println!("Computing original hash...");
        let original_hash = self.compute_file_hash(&test_file)?;
        
        // Test original file (should pass)
        println!("Testing original file integrity...");
        let original_valid = self.test_file_integrity(&test_file, Some(&original_hash))?;
        
        if !original_valid {
            return Err("Original file failed integrity check".into());
        }

        // Apply bit flips and test
        let mut rejection_count = 0;
        let mut corruption_results = Vec::new();

        for i in 0..num_corruptions {
            println!("Applying corruption {}/{}...", i + 1, num_corruptions);
            
            // Create corrupted copy
            let corrupted_file = format!("{}/corrupted_{}_{}gb.bin", self.temp_dir, i, file_size_gb);
            
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
                
                let (corrupted_data, flipped_positions) = self.apply_bit_flips(&buffer, 0.0001, i as u64);
                dst.write_all(&corrupted_data)?;
                dst.flush()?;
            }
            
            // Test corrupted file (should fail)
            let corrupted_valid = self.test_file_integrity(&corrupted_file, Some(&original_hash))?;
            
            if !corrupted_valid {
                rejection_count += 1;
            }
            
            corruption_results.push((i, !corrupted_valid));
            
            // Clean up corrupted file
            std::fs::remove_file(&corrupted_file)?;
            
            println!("  Corruption {}: rejected: {}", 
                    i + 1, !corrupted_valid);
        }
        
        // Clean up test file
        std::fs::remove_file(&test_file)?;
        
        let duration = start_time.elapsed();
        let duration_ms = duration.as_millis() as u64;
        let throughput_mbps = (file_size_bytes as f64 / (1024.0 * 1024.0)) / (duration.as_secs_f64());
        let rejection_rate = (rejection_count as f64 / num_corruptions as f64) * 100.0;
        let test_passed = rejection_rate == 100.0;
        
        let result = BitFlipTestResult {
            file_size_bytes: file_size_bytes as u64,
            original_hash,
            corruption_count: num_corruptions,
            rejection_count,
            rejection_rate,
            test_passed,
            duration_ms,
            throughput_mbps,
        };
        
        println!("\nTest Results:");
        println!("  File size: {}GB", file_size_gb);
        println!("  Duration: {:.2}s", duration.as_secs_f64());
        println!("  Corruptions tested: {}", num_corruptions);
        println!("  Rejections: {}/{}", rejection_count, num_corruptions);
        println!("  Rejection rate: {:.1}%", rejection_rate);
        println!("  Throughput: {:.2} MB/s", throughput_mbps);
        println!("  Test passed: {}", if test_passed { "✓" } else { "✗" });
        
        Ok(result)
    }

    /// Run scalability test with different file sizes
    pub fn run_scalability_test(&self) -> Result<CorpusTestSummary, Box<dyn std::error::Error>> {
        println!("\n=== Running Scalability Test ===");
        
        // Test different file sizes: 1GB, 10GB, 50GB, 100GB
        let test_sizes = vec![1, 10, 50, 100];
        let mut test_results = Vec::new();
        let mut total_duration_ms = 0u64;
        let mut total_throughput = 0.0;
        let mut test_count = 0;
        
        for &size_gb in &test_sizes {
            println!("\nTesting {}GB file...", size_gb);
            match self.run_bitflip_test(size_gb, 5) {
                Ok(result) => {
                    test_results.push(result.clone());
                    total_duration_ms += result.duration_ms;
                    total_throughput += result.throughput_mbps;
                    test_count += 1;
                }
                Err(e) => {
                    println!("Error testing {}GB: {}", size_gb, e);
                    test_results.push(BitFlipTestResult {
                        file_size_bytes: (size_gb * 1024 * 1024 * 1024) as u64,
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
        let average_throughput = if test_count > 0 { total_throughput / test_count as f64 } else { 0.0 };
        
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

    /// Run comprehensive 100GB bit-flip corpus test
    pub fn run_comprehensive_test(&self) -> Result<CorpusTestSummary, Box<dyn std::error::Error>> {
        println!("{}", "=".repeat(80));
        println!("100GB Bit-Flip Corpus Test for Model Asset Guard (Rust)");
        println!("{}", "=".repeat(80));
        
        let start_time = Instant::now();
        
        // Run scalability test
        let mut summary = self.run_scalability_test()?;
        
        // Run full 100GB test if system has enough resources
        println!("\nAttempting full 100GB test...");
        match self.run_bitflip_test(100, 20) {
            Ok(full_test_result) => {
                summary.test_results.push(full_test_result.clone());
                summary.total_tests += 1;
                if full_test_result.test_passed {
                    summary.passed_tests += 1;
                } else {
                    summary.failed_tests += 1;
                }
            }
            Err(e) => {
                println!("Full 100GB test failed: {}", e);
                summary.test_results.push(BitFlipTestResult {
                    file_size_bytes: 100 * 1024 * 1024 * 1024,
                    original_hash: "".to_string(),
                    corruption_count: 0,
                    rejection_count: 0,
                    rejection_rate: 0.0,
                    test_passed: false,
                    duration_ms: 0,
                    throughput_mbps: 0.0,
                });
                summary.total_tests += 1;
                summary.failed_tests += 1;
            }
        }
        
        let total_time = start_time.elapsed();
        summary.total_duration_ms = total_time.as_millis() as u64;
        summary.overall_success_rate = (summary.passed_tests as f64 / summary.total_tests as f64) * 100.0;
        
        // Save results
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
        println!("Average throughput: {:.2} MB/s", summary.average_throughput_mbps);
        println!("All tests passed: {}", if summary.failed_tests == 0 { "✓" } else { "✗" });
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
    fn test_file_creation_and_hash() {
        let temp_dir = tempdir().unwrap();
        let tester = BitFlipCorpusTester::new(1, Some(temp_dir.path().to_str().unwrap().to_string()));
        
        let test_file = temp_dir.path().join("test.bin");
        tester.create_test_file(test_file.to_str().unwrap(), 1024).unwrap();
        
        let hash = tester.compute_file_hash(test_file.to_str().unwrap()).unwrap();
        assert_eq!(hash.len(), 64); // SHA-256 hex string length
    }

    #[test]
    fn test_small_bitflip_test() {
        let temp_dir = tempdir().unwrap();
        let tester = BitFlipCorpusTester::new(1, Some(temp_dir.path().to_str().unwrap().to_string()));
        
        // Test with 1MB file (much smaller for unit test)
        let result = tester.run_bitflip_test(0, 5).unwrap(); // 0GB = 1MB for testing
        assert!(result.test_passed);
    }
} 