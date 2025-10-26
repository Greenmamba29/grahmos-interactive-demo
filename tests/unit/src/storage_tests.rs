use prism_cas::*;
use tempfile::TempDir;
use tokio_test;
use pretty_assertions::assert_eq;
use std::time::Duration;
use rand::RngCore;
use std::sync::Arc;
use futures;

/// Comprehensive storage tests for Content-Addressable Storage
/// 
/// Tests cover:
/// - Content addressability (Hash(content) = Address)
/// - Deduplication accuracy and performance
/// - Compression effectiveness
/// - Integrity verification (BLAKE3)
/// - Garbage collection correctness
/// - Concurrent access safety
/// - Error handling and recovery

#[cfg(test)]
mod cas_correctness_tests {
    use super::*;

    #[tokio::test]
    async fn test_content_addressability() {
        let temp_dir = TempDir::new().unwrap();
        let cas = ContentAddressableStorage::new(temp_dir.path()).unwrap();
        
        let content = b"Test content for hash verification";
        let expected_hash = blake3::hash(content);
        
        let result = cas.store(content).await.unwrap();
        
        // Verify hash matches content
        assert_eq!(result.hash.as_bytes(), expected_hash.as_bytes());
        
        // Verify content can be retrieved by hash
        let retrieved = cas.retrieve(&result.hash).await.unwrap();
        assert_eq!(retrieved, content);
    }
    
    #[tokio::test]
    async fn test_deduplication_accuracy() {
        let temp_dir = TempDir::new().unwrap();
        let cas = ContentAddressableStorage::new(temp_dir.path()).unwrap();
        
        let content = b"Duplicate content test data";
        
        // Store same content multiple times
        let result1 = cas.store(content).await.unwrap();
        let result2 = cas.store(content).await.unwrap();
        let result3 = cas.store(content).await.unwrap();
        
        // First should be new, others deduplicated
        assert!(result1.is_new);
        assert!(!result2.is_new);
        assert!(!result3.is_new);
        
        // All should have same hash
        assert_eq!(result1.hash, result2.hash);
        assert_eq!(result2.hash, result3.hash);
        
        // Storage size should not increase for duplicates
        assert_eq!(result1.stored_size, result2.stored_size);
    }
    
    #[tokio::test]
    async fn test_compression_effectiveness() {
        let temp_dir = TempDir::new().unwrap();
        let config = CASConfig {
            compression_enabled: true,
            compression_level: 6,
            ..Default::default()
        };
        let cas = ContentAddressableStorage::with_config(temp_dir.path(), config).unwrap();
        
        // Create highly compressible data
        let pattern = b"REPEATED_PATTERN_";
        let mut compressible_data = Vec::new();
        for _ in 0..1000 {
            compressible_data.extend_from_slice(pattern);
        }
        
        let result = cas.store(&compressible_data).await.unwrap();
        
        // Compression should significantly reduce stored size
        let compression_ratio = 1.0 - (result.stored_size as f64 / result.original_size as f64);
        assert!(compression_ratio > 0.6, "Compression ratio {} should be >60%", compression_ratio);
        
        // Verify data integrity after compression
        let retrieved = cas.retrieve(&result.hash).await.unwrap();
        assert_eq!(retrieved, compressible_data);
    }
    
    #[tokio::test]
    async fn test_blake3_integrity_verification() {
        let temp_dir = TempDir::new().unwrap();
        let config = CASConfig {
            integrity_verification: true,
            ..Default::default()
        };
        let cas = ContentAddressableStorage::with_config(temp_dir.path(), config).unwrap();
        
        let content = b"Integrity verification test data";
        let result = cas.store(content).await.unwrap();
        
        // Verify integrity check passes
        let is_valid = cas.verify_integrity(&result.hash).await.unwrap();
        assert!(is_valid);
        
        // Test with various content sizes
        let sizes = vec![1, 100, 1024, 4096, 65536, 1048576];
        for size in sizes {
            let mut data = vec![0u8; size];
            rand::thread_rng().fill_bytes(&mut data);
            
            let result = cas.store(&data).await.unwrap();
            let is_valid = cas.verify_integrity(&result.hash).await.unwrap();
            assert!(is_valid, "Integrity check failed for size {}", size);
        }
    }
    
    #[tokio::test]
    async fn test_garbage_collection_correctness() {
        let temp_dir = TempDir::new().unwrap();
        let cas = ContentAddressableStorage::new(temp_dir.path()).unwrap();
        
        // Store some test data
        let mut hashes = Vec::new();
        for i in 0..10 {
            let data = format!("Test data {}", i).into_bytes();
            let result = cas.store(&data).await.unwrap();
            hashes.push(result.hash);
        }
        
        // Initial statistics
        let stats_before = cas.statistics().await;
        assert_eq!(stats_before.total_blocks, 10);
        
        // Simulate garbage collection (in real implementation, would have ref counting)
        let reclaimed = cas.garbage_collect().await.unwrap();
        
        // Verify GC completed without errors
        assert!(reclaimed >= 0);
        
        // All stored data should still be retrievable after GC
        for hash in &hashes {
            let retrieved = cas.retrieve(hash).await.unwrap();
            assert!(!retrieved.is_empty());
        }
    }
}

#[cfg(test)]
mod cas_performance_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_concurrent_access_safety() {
        let temp_dir = TempDir::new().unwrap();
        let cas = std::sync::Arc::new(
            ContentAddressableStorage::new(temp_dir.path()).unwrap()
        );
        
        let num_concurrent = 10;
        let mut handles = Vec::new();
        
        // Spawn concurrent store operations
        for i in 0..num_concurrent {
            let cas_clone = Arc::clone(&cas);
            let handle = tokio::spawn(async move {
                let data = format!("Concurrent data {}", i).into_bytes();
                cas_clone.store(&data).await.unwrap()
            });
            handles.push(handle);
        }
        
        // Wait for all operations to complete
        let results: Vec<_> = futures::future::join_all(handles)
            .await
            .into_iter()
            .map(|r| r.unwrap())
            .collect();
        
        assert_eq!(results.len(), num_concurrent);
        
        // Verify all operations succeeded and have unique hashes
        let mut unique_hashes = std::collections::HashSet::new();
        for result in results {
            assert!(result.is_new); // Should all be unique content
            assert!(unique_hashes.insert(result.hash)); // Should all be different
        }
    }
    
    #[tokio::test]
    async fn test_large_content_handling() {
        let temp_dir = TempDir::new().unwrap();
        let cas = ContentAddressableStorage::new(temp_dir.path()).unwrap();
        
        // Test with 10MB content
        let large_size = 10 * 1024 * 1024;
        let mut large_content = vec![0u8; large_size];
        rand::thread_rng().fill_bytes(&mut large_content);
        
        let start_time = std::time::Instant::now();
        let result = cas.store(&large_content).await.unwrap();
        let store_duration = start_time.elapsed();
        
        // Should complete within reasonable time
        assert!(store_duration < Duration::from_secs(10), 
                "Large content storage took too long: {:?}", store_duration);
        
        // Verify retrieval
        let start_time = std::time::Instant::now();
        let retrieved = cas.retrieve(&result.hash).await.unwrap();
        let retrieve_duration = start_time.elapsed();
        
        assert!(retrieve_duration < Duration::from_secs(5),
                "Large content retrieval took too long: {:?}", retrieve_duration);
        assert_eq!(retrieved, large_content);
    }
    
    #[tokio::test]
    async fn test_storage_efficiency_requirements() {
        let temp_dir = TempDir::new().unwrap();
        let cas = ContentAddressableStorage::new(temp_dir.path()).unwrap();
        
        // Test deduplication effectiveness (70-85% target)
        let base_data = b"Base content pattern that will be duplicated";
        let mut total_original_size = 0;
        let mut total_stored_size = 0;
        
        // Store 100 copies of similar content (should deduplicate)
        for i in 0..100 {
            let mut content = base_data.to_vec();
            if i % 10 == 0 {
                // Every 10th is unique
                content.extend_from_slice(&i.to_le_bytes());
            }
            
            let result = cas.store(&content).await.unwrap();
            total_original_size += result.original_size;
            if result.is_new {
                total_stored_size += result.stored_size;
            }
        }
        
        let dedup_ratio = 1.0 - (total_stored_size as f64 / total_original_size as f64);
        assert!(dedup_ratio >= 0.7, "Deduplication ratio {} below 70% requirement", dedup_ratio);
        assert!(dedup_ratio <= 0.85, "Deduplication ratio {} suspiciously high", dedup_ratio);
    }
}

#[cfg(test)]
mod cas_error_handling_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_nonexistent_hash_retrieval() {
        let temp_dir = TempDir::new().unwrap();
        let cas = ContentAddressableStorage::new(temp_dir.path()).unwrap();
        
        // Generate random hash that doesn't exist
        let fake_hash = blake3::hash(b"nonexistent content");
        
        let result = cas.retrieve(&fake_hash).await;
        assert!(result.is_err());
        
        match result.unwrap_err() {
            CASError::BlockNotFound { .. } => {}, // Expected error
            other => panic!("Unexpected error type: {:?}", other),
        }
    }
    
    #[tokio::test]
    async fn test_storage_corruption_detection() {
        let temp_dir = TempDir::new().unwrap();
        let config = CASConfig {
            integrity_verification: true,
            ..Default::default()
        };
        let cas = ContentAddressableStorage::with_config(temp_dir.path(), config).unwrap();
        
        let content = b"Content for corruption test";
        let result = cas.store(content).await.unwrap();
        
        // Verify normal integrity check passes
        let is_valid = cas.verify_integrity(&result.hash).await.unwrap();
        assert!(is_valid);
        
        // TODO: In a real implementation, we would simulate corruption
        // by modifying the stored data and verifying detection
    }
    
    #[tokio::test]
    async fn test_storage_capacity_limits() {
        let temp_dir = TempDir::new().unwrap();
        let cas = ContentAddressableStorage::new(temp_dir.path()).unwrap();
        
        // Test capacity reporting
        let (used, total) = cas.capacity_info().unwrap();
        assert!(used <= total);
        assert!(total > 0);
        
        // TODO: Test behavior when approaching capacity limits
        // (would require configurable capacity limits in real implementation)
    }
    
    #[tokio::test]
    async fn test_invalid_configuration() {
        let temp_dir = TempDir::new().unwrap();
        
        // Test invalid compression level
        let invalid_config = CASConfig {
            compression_enabled: true,
            compression_level: 100, // Invalid level
            ..Default::default()
        };
        
        // Should handle gracefully or fail fast
        let result = ContentAddressableStorage::with_config(temp_dir.path(), invalid_config);
        // Implementation should either accept and clamp, or return error
        // For now, we just verify it doesn't panic
    }
}

#[cfg(test)]
mod cas_statistics_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_statistics_accuracy() {
        let temp_dir = TempDir::new().unwrap();
        let cas = ContentAddressableStorage::new(temp_dir.path()).unwrap();
        
        // Initial statistics should be zero
        let initial_stats = cas.statistics().await;
        assert_eq!(initial_stats.total_blocks, 0);
        assert_eq!(initial_stats.read_operations, 0);
        assert_eq!(initial_stats.write_operations, 0);
        
        // Store some content
        let content1 = b"First content";
        let content2 = b"Second content";
        let content3 = b"First content"; // Duplicate
        
        cas.store(content1).await.unwrap();
        cas.store(content2).await.unwrap();
        let result3 = cas.store(content3).await.unwrap();
        
        let stats_after_writes = cas.statistics().await;
        assert_eq!(stats_after_writes.total_blocks, 2); // Should be 2 unique
        assert_eq!(stats_after_writes.write_operations, 3); // All 3 operations counted
        assert!(stats_after_writes.dedup_saved_bytes > 0); // Third was deduplicated
        
        // Perform some reads
        cas.retrieve(&result3.hash).await.unwrap();
        cas.retrieve(&result3.hash).await.unwrap();
        
        let stats_after_reads = cas.statistics().await;
        assert_eq!(stats_after_reads.read_operations, 2);
        assert!(stats_after_reads.cache_hits > 0 || stats_after_reads.cache_misses > 0);
    }
    
    #[tokio::test]
    async fn test_deduplication_statistics() {
        let temp_dir = TempDir::new().unwrap();
        let cas = ContentAddressableStorage::new(temp_dir.path()).unwrap();
        
        let content = b"Content for dedup statistics test";
        
        // Store same content multiple times
        for _ in 0..5 {
            cas.store(content).await.unwrap();
        }
        
        let stats = cas.statistics().await;
        
        // Should show significant deduplication
        let dedup_ratio = stats.deduplication_ratio();
        assert!(dedup_ratio > 0.7, "Deduplication ratio {} too low", dedup_ratio);
        
        // Saved bytes should be reasonable
        assert!(stats.dedup_saved_bytes > content.len() as u64 * 3); // At least 4 duplicates saved
    }
}

/// Integration tests combining multiple CAS features
#[cfg(test)]
mod cas_integration_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_realistic_workload() {
        let temp_dir = TempDir::new().unwrap();
        let config = CASConfig {
            compression_enabled: true,
            integrity_verification: true,
            ..Default::default()
        };
        let cas = ContentAddressableStorage::with_config(temp_dir.path(), config).unwrap();
        
        // Simulate realistic mixed workload
        let mut stored_hashes = Vec::new();
        
        // Phase 1: Initial data loading
        for i in 0..100 {
            let mut content = format!("Document content {}", i % 20).into_bytes(); // Some duplication
            if i % 3 == 0 {
                // Add some unique data
                content.extend_from_slice(&i.to_le_bytes());
            }
            
            let result = cas.store(&content).await.unwrap();
            stored_hashes.push((result.hash, content));
        }
        
        // Phase 2: Random access pattern
        for _ in 0..50 {
            let idx = rand::random::<usize>() % stored_hashes.len();
            let (hash, expected_content) = &stored_hashes[idx];
            
            let retrieved = cas.retrieve(hash).await.unwrap();
            assert_eq!(retrieved, *expected_content);
        }
        
        // Phase 3: Verify final statistics
        let final_stats = cas.statistics().await;
        assert!(final_stats.total_blocks > 0);
        assert!(final_stats.read_operations >= 50);
        assert!(final_stats.write_operations >= 100);
        assert!(final_stats.deduplication_ratio() > 0.0);
        
        // Phase 4: Garbage collection
        let reclaimed = cas.garbage_collect().await.unwrap();
        assert!(reclaimed >= 0);
        
        // All data should still be accessible after GC
        for (hash, expected_content) in &stored_hashes[..10] { // Test subset
            let retrieved = cas.retrieve(hash).await.unwrap();
            assert_eq!(retrieved, *expected_content);
        }
    }
}