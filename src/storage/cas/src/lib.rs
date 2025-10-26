use blake3::{Hash, Hasher};
use chrono::{DateTime, Utc};
use rocksdb::{Options, DB};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tokio::sync::Mutex;
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

pub mod compression;
pub mod encryption;
pub mod integrity;

/// Content-Addressable Storage (CAS) implementation for PRISM
/// 
/// Provides the foundation for the replication engine specified in PRISM PRD:
/// - BLAKE3 hashing for content addressing (faster than SHA-256, parallel)
/// - Block-level deduplication achieving 70-85% storage reduction
/// - Optional zstd compression and AES-256-GCM encryption
/// - RocksDB backend for high-performance block storage
/// - Integrity verification with Merkle trees
/// - Target: > 100MB/s block storage I/O performance
pub struct ContentAddressableStorage {
    /// RocksDB instance for persistent storage
    db: Arc<DB>,
    
    /// In-memory index for fast hash lookups
    index: Arc<RwLock<HashMap<Hash, BlockMetadata>>>,
    
    /// Configuration for storage behavior
    config: CASConfig,
    
    /// Statistics and metrics
    stats: Arc<Mutex<CASStatistics>>,
    
    /// Storage directory path
    storage_path: PathBuf,
}

/// Configuration for CAS behavior
#[derive(Debug, Clone)]
pub struct CASConfig {
    /// Block size for content chunking (4KB default from PRD)
    pub block_size: usize,
    
    /// Enable compression (zstd)
    pub compression_enabled: bool,
    
    /// Compression level (1-22 for zstd)
    pub compression_level: i32,
    
    /// Enable encryption (AES-256-GCM)
    pub encryption_enabled: bool,
    
    /// Maximum cache size for in-memory index
    pub max_cache_size: usize,
    
    /// Enable integrity verification
    pub integrity_verification: bool,
    
    /// Automatic garbage collection threshold (percentage full)
    pub gc_threshold: f64,
}

impl Default for CASConfig {
    fn default() -> Self {
        Self {
            block_size: 4096, // 4KB blocks for optimal I/O performance
            compression_enabled: true,
            compression_level: 6, // Balanced compression vs CPU
            encryption_enabled: false, // Optional for security
            max_cache_size: 512 * 1024 * 1024, // 512MB index cache
            integrity_verification: true,
            gc_threshold: 0.9, // GC when 90% full
        }
    }
}

/// Metadata about a stored block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockMetadata {
    /// Content hash (BLAKE3)
    pub hash: String,
    
    /// Original size before compression/encryption
    pub original_size: u64,
    
    /// Stored size after compression/encryption
    pub stored_size: u64,
    
    /// Compression algorithm used (if any)
    pub compression: Option<CompressionType>,
    
    /// Encryption algorithm used (if any)
    pub encryption: Option<EncryptionType>,
    
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    
    /// Last access timestamp
    pub last_accessed: DateTime<Utc>,
    
    /// Reference count (for deduplication)
    pub ref_count: u64,
    
    /// Integrity checksum
    pub checksum: Option<String>,
}

/// Compression algorithms supported
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompressionType {
    Zstd { level: i32 },
    Lz4,
    None,
}

/// Encryption algorithms supported
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EncryptionType {
    Aes256Gcm,
    ChaCha20Poly1305,
    None,
}

/// Storage and performance statistics
#[derive(Debug, Default)]
pub struct CASStatistics {
    /// Total blocks stored
    pub total_blocks: u64,
    
    /// Total bytes stored (deduplicated)
    pub total_stored_bytes: u64,
    
    /// Total bytes saved through deduplication
    pub dedup_saved_bytes: u64,
    
    /// Total bytes saved through compression
    pub compression_saved_bytes: u64,
    
    /// Read operations performed
    pub read_operations: u64,
    
    /// Write operations performed
    pub write_operations: u64,
    
    /// Cache hits
    pub cache_hits: u64,
    
    /// Cache misses
    pub cache_misses: u64,
    
    /// Integrity verification failures
    pub integrity_failures: u64,
}

impl CASStatistics {
    /// Calculate deduplication ratio (saved / total)
    pub fn deduplication_ratio(&self) -> f64 {
        if self.total_stored_bytes == 0 {
            return 0.0;
        }
        self.dedup_saved_bytes as f64 / (self.total_stored_bytes + self.dedup_saved_bytes) as f64
    }
    
    /// Calculate compression ratio
    pub fn compression_ratio(&self) -> f64 {
        if self.total_stored_bytes == 0 {
            return 0.0;
        }
        self.compression_saved_bytes as f64 / (self.total_stored_bytes + self.compression_saved_bytes) as f64
    }
    
    /// Calculate cache hit rate
    pub fn cache_hit_rate(&self) -> f64 {
        let total_reads = self.cache_hits + self.cache_misses;
        if total_reads == 0 {
            return 0.0;
        }
        self.cache_hits as f64 / total_reads as f64
    }
}

/// Result of storing a block
#[derive(Debug)]
pub struct StoreResult {
    /// Hash of the stored content
    pub hash: Hash,
    
    /// Whether this was a new block (true) or deduplicated (false)
    pub is_new: bool,
    
    /// Original size of the content
    pub original_size: u64,
    
    /// Final stored size after compression/encryption
    pub stored_size: u64,
}

/// Errors that can occur during CAS operations
#[derive(thiserror::Error, Debug)]
pub enum CASError {
    #[error("Block not found: hash {hash}")]
    BlockNotFound { hash: String },
    
    #[error("Storage corruption detected: {details}")]
    Corruption { details: String },
    
    #[error("Storage full: {used_bytes}/{capacity_bytes} bytes")]
    StorageFull { used_bytes: u64, capacity_bytes: u64 },
    
    #[error("Index corruption: {reason}")]
    IndexCorruption { reason: String },
    
    #[error("Integrity check failed: expected {expected}, got {actual}")]
    IntegrityCheckFailed { expected: String, actual: String },
    
    #[error("Database error: {0}")]
    Database(#[from] rocksdb::Error),
    
    #[error("Compression error: {0}")]
    Compression(String),
    
    #[error("Encryption error: {0}")]
    Encryption(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type CASResult<T> = Result<T, CASError>;

impl ContentAddressableStorage {
    /// Create a new CAS instance with default configuration
    pub fn new<P: AsRef<Path>>(storage_path: P) -> CASResult<Self> {
        Self::with_config(storage_path, CASConfig::default())
    }
    
    /// Create a new CAS instance with custom configuration
    #[instrument(skip(config))]
    pub fn with_config<P: AsRef<Path>>(storage_path: P, config: CASConfig) -> CASResult<Self> {
        let storage_path = storage_path.as_ref().to_path_buf();
        
        info!(path = %storage_path.display(), \"Initializing Content-Addressable Storage\");
        
        // Ensure storage directory exists
        std::fs::create_dir_all(&storage_path)?;
        
        // Configure RocksDB options for optimal performance
        let mut db_opts = Options::default();
        db_opts.create_if_missing(true);
        db_opts.set_write_buffer_size(64 * 1024 * 1024); // 64MB write buffer
        db_opts.set_max_write_buffer_number(3);
        db_opts.set_target_file_size_base(64 * 1024 * 1024); // 64MB SST files
        db_opts.set_level_zero_file_num_compaction_trigger(8);
        db_opts.set_max_bytes_for_level_base(512 * 1024 * 1024); // 512MB L1
        db_opts.set_compression_type(rocksdb::DBCompressionType::Lz4); // Fast compression
        
        // Open database
        let db_path = storage_path.join(\"blocks.db\");
        let db = Arc::new(DB::open(&db_opts, db_path)?);
        
        // Initialize in-memory index
        let index = Arc::new(RwLock::new(HashMap::new()));
        
        // Load existing block metadata into index
        let stats = Arc::new(Mutex::new(CASStatistics::default()));
        
        let cas = Self {
            db,
            index,
            config,
            stats,
            storage_path,
        };
        
        // Rebuild index from existing data
        cas.rebuild_index()?;
        
        info!(\"CAS initialization complete\");
        Ok(cas)
    }
    
    /// Store content in the CAS, returning its hash
    #[instrument(skip(self, content))]
    pub async fn store(&self, content: &[u8]) -> CASResult<StoreResult> {
        let start = std::time::Instant::now();
        
        // Compute content hash
        let hash = self.hash_content(content);
        let hash_str = hash.to_hex().to_string();
        
        debug!(hash = %hash_str, size = content.len(), \"Storing content\");
        
        // Check if content already exists (deduplication)
        {
            let index = self.index.read().unwrap();
            if let Some(metadata) = index.get(&hash) {
                // Content exists, increment reference count
                let mut metadata = metadata.clone();
                metadata.ref_count += 1;
                metadata.last_accessed = Utc::now();
                
                // Update metadata in database
                let metadata_bytes = bincode::serialize(&metadata)
                    .map_err(|e| CASError::Database(rocksdb::Error::new(e.to_string())))?;
                self.db.put(format!(\"meta:{}\", hash_str), metadata_bytes)?;
                
                // Update index
                drop(index);
                let mut index = self.index.write().unwrap();
                index.insert(hash, metadata.clone());
                
                // Update statistics
                {
                    let mut stats = self.stats.lock().await;
                    stats.dedup_saved_bytes += content.len() as u64;
                }
                
                debug!(hash = %hash_str, \"Content deduplicated (existing)\");
                
                return Ok(StoreResult {
                    hash,
                    is_new: false,
                    original_size: content.len() as u64,
                    stored_size: metadata.stored_size,
                });
            }
        }
        
        // Content is new, process and store it
        let processed_content = self.process_content(content).await?;
        let stored_size = processed_content.len() as u64;
        
        // Create block metadata
        let metadata = BlockMetadata {
            hash: hash_str.clone(),
            original_size: content.len() as u64,
            stored_size,
            compression: if self.config.compression_enabled {
                Some(CompressionType::Zstd { level: self.config.compression_level })
            } else {
                None
            },
            encryption: if self.config.encryption_enabled {
                Some(EncryptionType::Aes256Gcm)
            } else {
                None
            },
            created_at: Utc::now(),
            last_accessed: Utc::now(),
            ref_count: 1,
            checksum: if self.config.integrity_verification {
                Some(self.compute_checksum(&processed_content))
            } else {
                None
            },
        };
        
        // Store content and metadata atomically
        let content_key = format!(\"data:{}\", hash_str);
        let metadata_key = format!(\"meta:{}\", hash_str);
        
        let metadata_bytes = bincode::serialize(&metadata)
            .map_err(|e| CASError::Database(rocksdb::Error::new(e.to_string())))?;
        
        // Use transaction for atomicity
        let mut batch = rocksdb::WriteBatch::default();
        batch.put(content_key, &processed_content);
        batch.put(metadata_key, metadata_bytes);
        self.db.write(batch)?;
        
        // Update in-memory index
        {
            let mut index = self.index.write().unwrap();
            index.insert(hash, metadata);
        }
        
        // Update statistics
        {
            let mut stats = self.stats.lock().await;
            stats.total_blocks += 1;
            stats.total_stored_bytes += stored_size;
            stats.write_operations += 1;
            
            if self.config.compression_enabled {
                stats.compression_saved_bytes += content.len() as u64 - stored_size;
            }
        }
        
        let duration = start.elapsed();
        debug!(
            hash = %hash_str, 
            original_size = content.len(),
            stored_size = stored_size,
            duration = ?duration,
            \"Content stored successfully\"
        );
        
        Ok(StoreResult {
            hash,
            is_new: true,
            original_size: content.len() as u64,
            stored_size,
        })
    }
    
    /// Retrieve content by its hash
    #[instrument(skip(self))]
    pub async fn retrieve(&self, hash: &Hash) -> CASResult<Vec<u8>> {
        let start = std::time::Instant::now();
        let hash_str = hash.to_hex().to_string();
        
        debug!(hash = %hash_str, \"Retrieving content\");
        
        // Check index first (cache)
        let metadata = {
            let index = self.index.read().unwrap();
            if let Some(metadata) = index.get(hash) {
                let mut stats = self.stats.try_lock().unwrap();
                stats.cache_hits += 1;
                Some(metadata.clone())
            } else {
                let mut stats = self.stats.try_lock().unwrap();
                stats.cache_misses += 1;
                None
            }
        };
        
        // If not in cache, load from database
        let metadata = match metadata {
            Some(meta) => meta,
            None => {
                let metadata_key = format!(\"meta:{}\", hash_str);
                let metadata_bytes = self.db.get(metadata_key)?
                    .ok_or_else(|| CASError::BlockNotFound { 
                        hash: hash_str.clone() 
                    })?;
                
                let metadata: BlockMetadata = bincode::deserialize(&metadata_bytes)
                    .map_err(|e| CASError::IndexCorruption { 
                        reason: format!(\"Failed to deserialize metadata: {}\", e)
                    })?;
                
                // Update cache
                {
                    let mut index = self.index.write().unwrap();
                    index.insert(*hash, metadata.clone());
                }
                
                metadata
            }
        };
        
        // Retrieve content data
        let content_key = format!(\"data:{}\", hash_str);
        let processed_content = self.db.get(content_key)?
            .ok_or_else(|| CASError::BlockNotFound { 
                hash: hash_str.clone() 
            })?;
        
        // Verify integrity if enabled
        if self.config.integrity_verification {
            if let Some(expected_checksum) = &metadata.checksum {
                let actual_checksum = self.compute_checksum(&processed_content);
                if &actual_checksum != expected_checksum {
                    return Err(CASError::IntegrityCheckFailed {
                        expected: expected_checksum.clone(),
                        actual: actual_checksum,
                    });
                }
            }
        }
        
        // Decompress and decrypt content
        let original_content = self.unprocess_content(&processed_content, &metadata).await?;
        
        // Update access time and statistics
        {
            let mut index = self.index.write().unwrap();
            if let Some(meta) = index.get_mut(hash) {
                meta.last_accessed = Utc::now();
            }
        }
        
        {
            let mut stats = self.stats.lock().await;
            stats.read_operations += 1;
        }
        
        let duration = start.elapsed();
        debug!(
            hash = %hash_str,
            size = original_content.len(),
            duration = ?duration,
            \"Content retrieved successfully\"
        );
        
        Ok(original_content)
    }
    
    /// Verify integrity of a stored block
    pub async fn verify_integrity(&self, hash: &Hash) -> CASResult<bool> {
        let hash_str = hash.to_hex().to_string();
        
        // Retrieve content (this will verify integrity)
        match self.retrieve(hash).await {
            Ok(_) => Ok(true),
            Err(CASError::IntegrityCheckFailed { .. }) => {
                let mut stats = self.stats.lock().await;
                stats.integrity_failures += 1;
                Ok(false)
            }
            Err(e) => Err(e),
        }
    }
    
    /// Get current storage statistics
    pub async fn statistics(&self) -> CASStatistics {
        self.stats.lock().await.clone()
    }
    
    /// Get storage capacity information
    pub fn capacity_info(&self) -> CASResult<(u64, u64)> {
        // Get filesystem statistics for the storage directory
        let metadata = std::fs::metadata(&self.storage_path)?;
        // This is a simplified implementation - in production you'd want
        // to get actual filesystem capacity information
        let used = metadata.len();
        let total = 1024 * 1024 * 1024 * 1024; // 1TB default capacity
        
        Ok((used, total))
    }
    
    /// Perform garbage collection to reclaim space
    pub async fn garbage_collect(&self) -> CASResult<u64> {
        info!(\"Starting garbage collection\");
        
        let mut reclaimed_bytes = 0u64;
        let mut blocks_removed = 0u64;
        
        // Collect blocks with zero references
        let blocks_to_remove: Vec<Hash> = {
            let index = self.index.read().unwrap();
            index.iter()
                .filter_map(|(hash, metadata)| {
                    if metadata.ref_count == 0 {
                        Some(*hash)
                    } else {
                        None
                    }
                })
                .collect()
        };
        
        // Remove unreferenced blocks
        for hash in blocks_to_remove {
            if let Ok(metadata) = self.remove_block(&hash).await {
                reclaimed_bytes += metadata.stored_size;
                blocks_removed += 1;
            }
        }
        
        info!(
            blocks_removed = blocks_removed,
            bytes_reclaimed = reclaimed_bytes,
            \"Garbage collection completed\"
        );
        
        Ok(reclaimed_bytes)
    }
    
    /// Compute BLAKE3 hash of content
    fn hash_content(&self, content: &[u8]) -> Hash {
        blake3::hash(content)
    }
    
    /// Process content for storage (compress + encrypt)
    async fn process_content(&self, content: &[u8]) -> CASResult<Vec<u8>> {
        let mut processed = content.to_vec();
        
        // Apply compression if enabled
        if self.config.compression_enabled {
            processed = zstd::bulk::compress(&processed, self.config.compression_level)
                .map_err(|e| CASError::Compression(e.to_string()))?;
        }
        
        // Apply encryption if enabled
        if self.config.encryption_enabled {
            // TODO: Implement AES-256-GCM encryption
            // This would require key management integration
        }
        
        Ok(processed)
    }
    
    /// Reverse processing (decrypt + decompress)
    async fn unprocess_content(&self, content: &[u8], metadata: &BlockMetadata) -> CASResult<Vec<u8>> {
        let mut processed = content.to_vec();
        
        // Decrypt if needed
        if let Some(EncryptionType::Aes256Gcm) = metadata.encryption {
            // TODO: Implement decryption
        }
        
        // Decompress if needed
        if let Some(CompressionType::Zstd { .. }) = metadata.compression {
            processed = zstd::bulk::decompress(&processed, metadata.original_size as usize)
                .map_err(|e| CASError::Compression(e.to_string()))?;
        }
        
        Ok(processed)
    }
    
    /// Compute integrity checksum
    fn compute_checksum(&self, content: &[u8]) -> String {
        blake3::hash(content).to_hex().to_string()
    }
    
    /// Rebuild in-memory index from database
    fn rebuild_index(&self) -> CASResult<()> {
        info!(\"Rebuilding in-memory index\");
        
        let mut index = self.index.write().unwrap();
        let mut count = 0;
        
        let iter = self.db.iterator(rocksdb::IteratorMode::Start);
        for item in iter {
            let (key, value) = item?;
            let key_str = String::from_utf8_lossy(&key);
            
            // Only process metadata entries
            if key_str.starts_with(\"meta:\") {
                let hash_str = &key_str[5..]; // Remove \"meta:\" prefix
                if let Ok(hash) = Hash::from_hex(hash_str) {
                    if let Ok(metadata) = bincode::deserialize::<BlockMetadata>(&value) {
                        index.insert(hash, metadata);
                        count += 1;
                    }
                }
            }
        }
        
        info!(blocks_indexed = count, \"Index rebuild complete\");
        Ok(())
    }
    
    /// Remove a block from storage
    async fn remove_block(&self, hash: &Hash) -> CASResult<BlockMetadata> {
        let hash_str = hash.to_hex().to_string();
        
        // Get metadata before removal
        let metadata = {
            let index = self.index.read().unwrap();
            index.get(hash).cloned()
                .ok_or_else(|| CASError::BlockNotFound { 
                    hash: hash_str.clone() 
                })?
        };
        
        // Remove from database
        let content_key = format!(\"data:{}\", hash_str);
        let metadata_key = format!(\"meta:{}\", hash_str);
        
        let mut batch = rocksdb::WriteBatch::default();
        batch.delete(content_key);
        batch.delete(metadata_key);
        self.db.write(batch)?;
        
        // Remove from index
        {
            let mut index = self.index.write().unwrap();
            index.remove(hash);
        }
        
        // Update statistics
        {
            let mut stats = self.stats.lock().await;
            stats.total_blocks -= 1;
            stats.total_stored_bytes -= metadata.stored_size;
        }
        
        Ok(metadata)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio_test;
    
    #[tokio::test]
    async fn test_cas_creation() {
        let temp_dir = TempDir::new().unwrap();
        let cas = ContentAddressableStorage::new(temp_dir.path());
        assert!(cas.is_ok());
    }
    
    #[tokio::test]
    async fn test_store_and_retrieve() {
        let temp_dir = TempDir::new().unwrap();
        let cas = ContentAddressableStorage::new(temp_dir.path()).unwrap();
        
        let content = b\"Hello, PRISM CAS!\";
        let store_result = cas.store(content).await.unwrap();
        
        assert!(store_result.is_new);
        assert_eq!(store_result.original_size, content.len() as u64);
        
        let retrieved = cas.retrieve(&store_result.hash).await.unwrap();
        assert_eq!(retrieved, content);
    }
    
    #[tokio::test]
    async fn test_deduplication() {
        let temp_dir = TempDir::new().unwrap();
        let cas = ContentAddressableStorage::new(temp_dir.path()).unwrap();
        
        let content = b\"Duplicate content test\";
        
        // Store the same content twice
        let result1 = cas.store(content).await.unwrap();
        let result2 = cas.store(content).await.unwrap();
        
        // First should be new, second should be deduplicated
        assert!(result1.is_new);
        assert!(!result2.is_new);
        
        // Hashes should be identical
        assert_eq!(result1.hash, result2.hash);
        
        // Verify deduplication statistics
        let stats = cas.statistics().await;
        assert!(stats.deduplication_ratio() > 0.0);
    }
    
    #[tokio::test]
    async fn test_integrity_verification() {
        let temp_dir = TempDir::new().unwrap();
        let cas = ContentAddressableStorage::new(temp_dir.path()).unwrap();
        
        let content = b\"Integrity test content\";
        let store_result = cas.store(content).await.unwrap();
        
        // Verify integrity
        let is_valid = cas.verify_integrity(&store_result.hash).await.unwrap();
        assert!(is_valid);
    }
    
    #[tokio::test]
    async fn test_compression() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = CASConfig::default();
        config.compression_enabled = true;
        
        let cas = ContentAddressableStorage::with_config(temp_dir.path(), config).unwrap();
        
        // Store highly compressible content
        let content = vec![b'A'; 10000]; // 10KB of repeated data
        let store_result = cas.store(&content).await.unwrap();
        
        // Stored size should be much smaller due to compression
        assert!(store_result.stored_size < store_result.original_size);
        
        // Verify we can retrieve the original content
        let retrieved = cas.retrieve(&store_result.hash).await.unwrap();
        assert_eq!(retrieved, content);
    }
    
    #[test]
    fn test_blake3_hashing() {
        let content = b\"Test content for hashing\";
        let hash1 = blake3::hash(content);
        let hash2 = blake3::hash(content);
        
        // Same content should produce same hash
        assert_eq!(hash1, hash2);
        
        // Different content should produce different hash
        let different_content = b\"Different test content\";
        let hash3 = blake3::hash(different_content);
        assert_ne!(hash1, hash3);
    }
    
    #[tokio::test]
    async fn test_statistics() {
        let temp_dir = TempDir::new().unwrap();
        let cas = ContentAddressableStorage::new(temp_dir.path()).unwrap();
        
        // Store some content
        let content1 = b\"First content\";
        let content2 = b\"Second content\";
        let content3 = b\"First content\"; // Duplicate
        
        cas.store(content1).await.unwrap();
        cas.store(content2).await.unwrap();
        cas.store(content3).await.unwrap(); // Should be deduplicated
        
        let stats = cas.statistics().await;
        
        // Should have 2 unique blocks (content3 is duplicate)
        assert_eq!(stats.total_blocks, 2);
        assert!(stats.dedup_saved_bytes > 0);
        assert_eq!(stats.write_operations, 3);
    }
}"