use crate::{CASError, CASResult};
use std::io::{Read, Write};

/// Compression utilities for Content-Addressable Storage
/// 
/// Supports multiple compression algorithms optimized for different use cases:
/// - Zstd: Balanced compression ratio and speed (default)
/// - Lz4: Maximum speed with moderate compression
/// - Snappy: Very fast compression/decompression

/// Compress data using Zstandard compression
pub fn compress_zstd(data: &[u8], level: i32) -> CASResult<Vec<u8>> {
    zstd::bulk::compress(data, level)
        .map_err(|e| CASError::Compression(format!("Zstd compression failed: {}", e)))
}

/// Decompress Zstandard compressed data
pub fn decompress_zstd(data: &[u8], expected_size: usize) -> CASResult<Vec<u8>> {
    zstd::bulk::decompress(data, expected_size)
        .map_err(|e| CASError::Compression(format!("Zstd decompression failed: {}", e)))
}

/// Compress data using LZ4 compression
#[cfg(feature = "lz4")]
pub fn compress_lz4(data: &[u8]) -> CASResult<Vec<u8>> {
    lz4_flex::compress_prepend_size(data)
        .map_err(|e| CASError::Compression(format!("LZ4 compression failed: {}", e)))
}

/// Decompress LZ4 compressed data
#[cfg(feature = "lz4")]
pub fn decompress_lz4(data: &[u8]) -> CASResult<Vec<u8>> {
    lz4_flex::decompress_size_prepended(data)
        .map_err(|e| CASError::Compression(format!("LZ4 decompression failed: {}", e)))
}

/// Compress data using Snappy compression
#[cfg(feature = "snap")]
pub fn compress_snappy(data: &[u8]) -> CASResult<Vec<u8>> {
    let mut encoder = snap::raw::Encoder::new();
    encoder.compress_vec(data)
        .map_err(|e| CASError::Compression(format!("Snappy compression failed: {}", e)))
}

/// Decompress Snappy compressed data
#[cfg(feature = "snap")]
pub fn decompress_snappy(data: &[u8]) -> CASResult<Vec<u8>> {
    let mut decoder = snap::raw::Decoder::new();
    decoder.decompress_vec(data)
        .map_err(|e| CASError::Compression(format!("Snappy decompression failed: {}", e)))
}

/// Calculate compression ratio
pub fn compression_ratio(original_size: usize, compressed_size: usize) -> f64 {
    if original_size == 0 {
        return 0.0;
    }
    1.0 - (compressed_size as f64 / original_size as f64)
}

/// Estimate optimal compression level for data
/// Returns a recommendation based on content analysis
pub fn estimate_optimal_level(data: &[u8]) -> i32 {
    // Simple heuristics for compression level selection
    let size = data.len();
    
    // Sample entropy calculation for randomness estimation
    let mut byte_counts = [0u32; 256];
    for &byte in data.iter().take(1024.min(size)) {
        byte_counts[byte as usize] += 1;
    }
    
    let sample_size = 1024.min(size) as f64;
    let mut entropy = 0.0;
    
    for &count in &byte_counts {
        if count > 0 {
            let probability = count as f64 / sample_size;
            entropy -= probability * probability.log2();
        }
    }
    
    // Entropy-based level selection:
    // - High entropy (>7): Low compression level (data is random)
    // - Medium entropy (4-7): Medium compression level
    // - Low entropy (<4): High compression level (data is structured)
    match entropy {
        e if e > 7.0 => 1,   // Fast compression for random data
        e if e > 4.0 => 6,   // Balanced for mixed content
        _ => 19,             // Aggressive compression for structured data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zstd_compression() {
        let data = b"Hello, this is test data for compression!".repeat(100);
        let compressed = compress_zstd(&data, 6).unwrap();
        
        assert!(compressed.len() < data.len());
        
        let decompressed = decompress_zstd(&compressed, data.len()).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_compression_ratio() {
        let ratio = compression_ratio(1000, 400);
        assert!((ratio - 0.6).abs() < f64::EPSILON);
        
        let no_compression = compression_ratio(1000, 1000);
        assert_eq!(no_compression, 0.0);
    }

    #[test]
    fn test_optimal_level_estimation() {
        // Random data should get low compression level
        let random_data = (0..1000).map(|i| (i * 17) as u8).collect::<Vec<_>>();
        let level = estimate_optimal_level(&random_data);
        assert!(level <= 6);
        
        // Repeated data should get high compression level
        let repeated_data = vec![b'A'; 1000];
        let level = estimate_optimal_level(&repeated_data);
        assert!(level >= 10);
    }

    #[cfg(feature = "lz4")]
    #[test]
    fn test_lz4_compression() {
        let data = b"Test data for LZ4 compression algorithm".repeat(50);
        let compressed = compress_lz4(&data).unwrap();
        
        assert!(compressed.len() < data.len());
        
        let decompressed = decompress_lz4(&compressed).unwrap();
        assert_eq!(decompressed, data);
    }
}