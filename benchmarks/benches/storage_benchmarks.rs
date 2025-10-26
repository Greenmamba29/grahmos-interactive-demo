use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use prism_cas::*;
use rand::RngCore;
use std::time::Duration;
use tempfile::TempDir;
use tokio::runtime::Runtime;

/// Storage I/O benchmarks for PRISM Content-Addressable Storage
/// 
/// Performance targets from requirements:
/// - Storage I/O: Must exceed 100MB/s for block operations
/// - Memory Usage: Must stay <512MB baseline per agent
/// - Deduplication: 70-85% storage reduction target

fn create_test_data(size: usize) -> Vec<u8> {
    let mut data = vec![0u8; size];
    rand::thread_rng().fill_bytes(&mut data);
    data
}

fn create_compressible_data(size: usize) -> Vec<u8> {
    // Create data that compresses well (repeated patterns)
    let pattern = b"PRISM_TEST_DATA_PATTERN_";
    let mut data = Vec::with_capacity(size);
    while data.len() < size {
        data.extend_from_slice(pattern);
    }
    data.truncate(size);
    data
}

/// Benchmark CAS storage operations at various block sizes
fn bench_cas_store_performance(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("cas_store");
    
    // Test different block sizes
    let block_sizes = vec![1024, 4096, 16384, 65536, 262144, 1048576]; // 1KB to 1MB
    
    for size in block_sizes {
        group.throughput(Throughput::Bytes(size as u64));
        
        group.bench_with_input(
            BenchmarkId::new("store", size), 
            &size, 
            |b, &size| {
                let temp_dir = TempDir::new().unwrap();
                let cas = rt.block_on(async {
                    ContentAddressableStorage::new(temp_dir.path()).unwrap()
                });
                
                let data = create_test_data(size);
                
                b.to_async(&rt).iter(|| async {
                    let result = cas.store(&data).await.unwrap();
                    black_box(result);
                });
            }
        );
    }
    
    group.finish();
}

/// Benchmark CAS retrieval operations
fn bench_cas_retrieve_performance(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("cas_retrieve");
    
    let block_sizes = vec![1024, 4096, 16384, 65536, 262144, 1048576];
    
    for size in block_sizes {
        group.throughput(Throughput::Bytes(size as u64));
        
        group.bench_with_input(
            BenchmarkId::new("retrieve", size), 
            &size, 
            |b, &size| {
                let temp_dir = TempDir::new().unwrap();
                let cas = rt.block_on(async {
                    ContentAddressableStorage::new(temp_dir.path()).unwrap()
                });
                
                let data = create_test_data(size);
                
                // Pre-store the data
                let hash = rt.block_on(async {
                    cas.store(&data).await.unwrap().hash
                });
                
                b.to_async(&rt).iter(|| async {
                    let retrieved = cas.retrieve(&hash).await.unwrap();
                    black_box(retrieved);
                });
            }
        );
    }
    
    group.finish();
}

/// Benchmark compression effectiveness and performance
fn bench_cas_compression(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("cas_compression");
    
    // Test with compressible data
    let sizes = vec![4096, 65536, 1048576]; // 4KB, 64KB, 1MB
    
    for size in sizes {
        group.throughput(Throughput::Bytes(size as u64));
        
        // Test with compression enabled
        group.bench_with_input(
            BenchmarkId::new("compressed", size), 
            &size, 
            |b, &size| {
                let temp_dir = TempDir::new().unwrap();
                let config = CASConfig {
                    compression_enabled: true,
                    compression_level: 6,
                    ..Default::default()
                };
                
                let cas = rt.block_on(async {
                    ContentAddressableStorage::with_config(temp_dir.path(), config).unwrap()
                });
                
                let data = create_compressible_data(size);
                
                b.to_async(&rt).iter(|| async {
                    let result = cas.store(&data).await.unwrap();
                    black_box(result);
                });
            }
        );
        
        // Test without compression for comparison
        group.bench_with_input(
            BenchmarkId::new("uncompressed", size), 
            &size, 
            |b, &size| {
                let temp_dir = TempDir::new().unwrap();
                let config = CASConfig {
                    compression_enabled: false,
                    ..Default::default()
                };
                
                let cas = rt.block_on(async {
                    ContentAddressableStorage::with_config(temp_dir.path(), config).unwrap()
                });
                
                let data = create_compressible_data(size);
                
                b.to_async(&rt).iter(|| async {
                    let result = cas.store(&data).await.unwrap();
                    black_box(result);
                });
            }
        );
    }
    
    group.finish();
}

/// Benchmark deduplication effectiveness
fn bench_cas_deduplication(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("cas_deduplication");
    group.sample_size(10); // Fewer samples for this test
    
    group.bench_function("duplicate_detection", |b| {
        let temp_dir = TempDir::new().unwrap();
        let cas = rt.block_on(async {
            ContentAddressableStorage::new(temp_dir.path()).unwrap()
        });
        
        let data = create_test_data(65536); // 64KB
        
        // Store initial copy
        rt.block_on(async {
            cas.store(&data).await.unwrap();
        });
        
        b.to_async(&rt).iter(|| async {
            // This should be deduplicated (very fast)
            let result = cas.store(&data).await.unwrap();
            assert!(!result.is_new); // Should be deduplicated
            black_box(result);
        });
    });
    
    group.finish();
}

/// Benchmark concurrent operations
fn bench_cas_concurrent_ops(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("cas_concurrent");
    group.measurement_time(Duration::from_secs(10)); // Longer measurement for concurrent tests
    
    let concurrency_levels = vec![1, 4, 8, 16];
    
    for concurrency in concurrency_levels {
        group.bench_with_input(
            BenchmarkId::new("concurrent_stores", concurrency), 
            &concurrency, 
            |b, &concurrency| {
                let temp_dir = TempDir::new().unwrap();
                let cas = rt.block_on(async {
                    ContentAddressableStorage::new(temp_dir.path()).unwrap()
                });
                
                b.to_async(&rt).iter(|| async {
                    let mut handles = Vec::new();
                    
                    for i in 0..concurrency {
                        let data = create_test_data(4096 + i * 100); // Slightly different sizes
                        let cas_clone = &cas;
                        
                        handles.push(tokio::spawn(async move {
                            cas_clone.store(&data).await.unwrap()
                        }));
                    }
                    
                    let results: Vec<_> = futures::future::join_all(handles)
                        .await
                        .into_iter()
                        .map(|r| r.unwrap())
                        .collect();
                    
                    black_box(results);
                });
            }
        );
    }
    
    group.finish();
}

/// Measure throughput in MB/s and verify it meets requirements
fn bench_throughput_validation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("throughput_validation");
    group.measurement_time(Duration::from_secs(10));
    
    // Test with optimal block size (4KB as per config)
    group.throughput(Throughput::Bytes(4096));
    
    group.bench_function("sustained_throughput", |b| {
        let temp_dir = TempDir::new().unwrap();
        let cas = rt.block_on(async {
            ContentAddressableStorage::new(temp_dir.path()).unwrap()
        });
        
        b.to_async(&rt).iter_custom(|iters| async move {
            let start = std::time::Instant::now();
            
            for i in 0..iters {
                let data = create_test_data(4096);
                // Modify data slightly to avoid deduplication
                let mut modified_data = data;
                modified_data.extend_from_slice(&i.to_le_bytes());
                
                cas.store(&modified_data).await.unwrap();
            }
            
            start.elapsed()
        });
    });
    
    // Custom callback to measure and validate throughput
    group.bench_function("throughput_measurement", |b| {
        let temp_dir = TempDir::new().unwrap();
        let cas = rt.block_on(async {
            ContentAddressableStorage::new(temp_dir.path()).unwrap()
        });
        
        const TOTAL_DATA_MB: usize = 100; // Test with 100MB
        const BLOCK_SIZE: usize = 4096;
        const NUM_BLOCKS: usize = TOTAL_DATA_MB * 1024 * 1024 / BLOCK_SIZE;
        
        b.iter_custom(|_| {
            rt.block_on(async {
                let start = std::time::Instant::now();
                
                for i in 0..NUM_BLOCKS {
                    let mut data = create_test_data(BLOCK_SIZE);
                    // Ensure uniqueness to avoid deduplication
                    data.extend_from_slice(&i.to_le_bytes());
                    
                    cas.store(&data).await.unwrap();
                }
                
                let elapsed = start.elapsed();
                let throughput_mb_per_sec = TOTAL_DATA_MB as f64 / elapsed.as_secs_f64();
                
                println!("Storage throughput: {:.2} MB/s", throughput_mb_per_sec);
                
                // Validate requirement: Must exceed 100MB/s
                assert!(throughput_mb_per_sec >= 100.0, 
                        "Storage throughput {} MB/s does not meet 100MB/s requirement", 
                        throughput_mb_per_sec);
                
                elapsed
            })
        });
    });
    
    group.finish();
}

/// Benchmark memory usage during operations
fn bench_memory_usage(c: &mut Criterion) {
    use sysinfo::{System, SystemExt, ProcessExt, PidExt};
    
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("memory_usage");
    group.sample_size(10);
    
    group.bench_function("memory_baseline", |b| {
        b.iter_custom(|_| {
            rt.block_on(async {
                let start = std::time::Instant::now();
                
                let temp_dir = TempDir::new().unwrap();
                let cas = ContentAddressableStorage::new(temp_dir.path()).unwrap();
                
                // Measure memory before operations
                let mut system = System::new();
                system.refresh_processes();
                let pid = sysinfo::get_current_pid().unwrap();
                let process = system.process(pid).unwrap();
                let memory_before = process.memory();
                
                // Perform operations
                for i in 0..1000 {
                    let data = create_test_data(4096 + i); // Variable size to avoid dedup
                    cas.store(&data).await.unwrap();
                }
                
                // Measure memory after operations
                system.refresh_processes();
                let process = system.process(pid).unwrap();
                let memory_after = process.memory();
                
                let memory_used_mb = (memory_after - memory_before) as f64 / 1024.0 / 1024.0;
                println!("Memory used: {:.2} MB", memory_used_mb);
                
                // Validate requirement: Must stay <512MB baseline per agent
                assert!(memory_used_mb < 512.0, 
                        "Memory usage {} MB exceeds 512MB limit", memory_used_mb);
                
                start.elapsed()
            })
        });
    });
    
    group.finish();
}

/// Benchmark garbage collection performance
fn bench_garbage_collection(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("garbage_collection");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(15));
    
    group.bench_function("gc_performance", |b| {
        let temp_dir = TempDir::new().unwrap();
        let cas = rt.block_on(async {
            ContentAddressableStorage::new(temp_dir.path()).unwrap()
        });
        
        b.to_async(&rt).iter(|| async {
            // Create blocks for garbage collection
            let mut hashes = Vec::new();
            for i in 0..100 {
                let data = create_test_data(4096 + i);
                let result = cas.store(&data).await.unwrap();
                hashes.push(result.hash);
            }
            
            // Simulate dereferencing blocks (would need actual ref counting in real implementation)
            // For now, just measure GC execution time
            let gc_start = std::time::Instant::now();
            let reclaimed = cas.garbage_collect().await.unwrap();
            let gc_duration = gc_start.elapsed();
            
            println!("GC reclaimed {} bytes in {:?}", reclaimed, gc_duration);
            black_box(reclaimed);
        });
    });
    
    group.finish();
}

criterion_group!(
    storage_benches,
    bench_cas_store_performance,
    bench_cas_retrieve_performance, 
    bench_cas_compression,
    bench_cas_deduplication,
    bench_cas_concurrent_ops,
    bench_throughput_validation,
    bench_memory_usage,
    bench_garbage_collection
);

criterion_main!(storage_benches);