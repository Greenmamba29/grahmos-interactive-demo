use crate::{CASError, CASResult};
use blake3::{Hash, Hasher};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Data integrity verification utilities for Content-Addressable Storage
/// 
/// Implements multiple integrity verification mechanisms:
/// - BLAKE3 checksums for fast content verification
/// - Merkle trees for efficient integrity proofs
/// - Content verification and repair capabilities
/// - Batch integrity checking for storage optimization

/// Merkle tree node containing hash and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleNode {
    /// Hash of the node content
    pub hash: Hash,
    /// Level in the tree (0 = leaf)
    pub level: u8,
    /// Index at this level
    pub index: u64,
    /// Child hashes (empty for leaves)
    pub children: Vec<Hash>,
}

/// Merkle tree for integrity verification
#[derive(Debug, Clone)]
pub struct MerkleTree {
    /// All nodes in the tree, indexed by hash
    nodes: HashMap<Hash, MerkleNode>,
    /// Root hash of the tree
    root_hash: Option<Hash>,
    /// Leaf hashes (data blocks)
    leaves: Vec<Hash>,
}

/// Integrity proof for a specific data block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityProof {
    /// Hash of the data being verified
    pub data_hash: Hash,
    /// Merkle path from leaf to root
    pub merkle_path: Vec<Hash>,
    /// Sibling hashes for verification
    pub siblings: Vec<Hash>,
    /// Root hash this proof validates against
    pub root_hash: Hash,
}

/// Integrity verification result
#[derive(Debug, Clone, PartialEq)]
pub enum VerificationResult {
    /// Data is valid and matches expected hash
    Valid,
    /// Data is corrupted (hash mismatch)
    Corrupted { expected: Hash, actual: Hash },
    /// Data is missing
    Missing,
    /// Verification failed due to error
    Error(String),
}

/// Batch integrity check result
#[derive(Debug)]
pub struct BatchVerificationResult {
    /// Total blocks checked
    pub total_checked: u64,
    /// Number of valid blocks
    pub valid_blocks: u64,
    /// Number of corrupted blocks
    pub corrupted_blocks: u64,
    /// Number of missing blocks
    pub missing_blocks: u64,
    /// Details of failed blocks
    pub failures: Vec<(Hash, VerificationResult)>,
    /// Time taken for verification
    pub duration: std::time::Duration,
}

impl MerkleTree {
    /// Create a new empty Merkle tree
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            root_hash: None,
            leaves: Vec::new(),
        }
    }
    
    /// Build Merkle tree from leaf hashes
    pub fn from_leaves(leaves: Vec<Hash>) -> CASResult<Self> {
        if leaves.is_empty() {
            return Err(CASError::Corruption {
                details: "Cannot create Merkle tree from empty leaves".to_string(),
            });
        }
        
        let mut tree = Self::new();
        tree.leaves = leaves.clone();
        
        // Create leaf nodes
        let mut current_level: Vec<Hash> = Vec::new();
        for (index, &leaf_hash) in leaves.iter().enumerate() {
            let node = MerkleNode {
                hash: leaf_hash,
                level: 0,
                index: index as u64,
                children: vec![],
            };
            tree.nodes.insert(leaf_hash, node);
            current_level.push(leaf_hash);
        }
        
        // Build tree bottom-up
        let mut level = 1;
        while current_level.len() > 1 {
            let mut next_level = Vec::new();
            
            // Process pairs of nodes
            for chunk in current_level.chunks(2) {
                let combined_hash = if chunk.len() == 2 {
                    // Hash pair of children
                    let mut hasher = Hasher::new();
                    hasher.update(chunk[0].as_bytes());
                    hasher.update(chunk[1].as_bytes());
                    hasher.finalize()
                } else {
                    // Odd number - promote single node
                    chunk[0]
                };
                
                let node = MerkleNode {
                    hash: combined_hash,
                    level,
                    index: next_level.len() as u64,
                    children: chunk.to_vec(),
                };
                
                tree.nodes.insert(combined_hash, node);
                next_level.push(combined_hash);
            }
            
            current_level = next_level;
            level += 1;
        }
        
        // Set root hash
        tree.root_hash = current_level.get(0).copied();
        
        Ok(tree)
    }
    
    /// Get the root hash of the tree
    pub fn root_hash(&self) -> Option<Hash> {
        self.root_hash
    }
    
    /// Generate integrity proof for a leaf hash
    pub fn generate_proof(&self, leaf_hash: Hash) -> CASResult<IntegrityProof> {
        let root_hash = self.root_hash
            .ok_or_else(|| CASError::Corruption {
                details: "Merkle tree has no root".to_string(),
            })?;
        
        // Find leaf index
        let leaf_index = self.leaves.iter()
            .position(|&h| h == leaf_hash)
            .ok_or_else(|| CASError::Corruption {
                details: format!("Leaf hash not found in tree: {}", leaf_hash.to_hex()),
            })?;
        
        let mut merkle_path = Vec::new();
        let mut siblings = Vec::new();
        let mut current_hash = leaf_hash;
        let mut current_index = leaf_index;
        
        // Traverse from leaf to root
        for level in 0..=20 { // Reasonable max depth
            if current_hash == root_hash {
                break;
            }
            
            // Find sibling
            let sibling_index = if current_index % 2 == 0 {
                current_index + 1
            } else {
                current_index - 1
            };
            
            // Find parent node that contains current hash as child
            let parent_hash = self.nodes.iter()
                .find(|(_, node)| {
                    node.level == level + 1 && node.children.contains(&current_hash)
                })
                .map(|(hash, _)| *hash)
                .ok_or_else(|| CASError::Corruption {
                    details: format!("Parent node not found for hash: {}", current_hash.to_hex()),
                })?;
            
            // Find sibling hash if it exists
            if let Some(&sibling_hash) = self.leaves.get(sibling_index) {
                if sibling_hash != current_hash {
                    siblings.push(sibling_hash);
                }
            }
            
            merkle_path.push(current_hash);
            current_hash = parent_hash;
            current_index /= 2;
        }
        
        Ok(IntegrityProof {
            data_hash: leaf_hash,
            merkle_path,
            siblings,
            root_hash,
        })
    }
    
    /// Verify an integrity proof
    pub fn verify_proof(&self, proof: &IntegrityProof) -> bool {
        if proof.root_hash != self.root_hash.unwrap_or_default() {
            return false;
        }
        
        let mut current_hash = proof.data_hash;
        
        // Reconstruct path to root
        for &sibling in &proof.siblings {
            let mut hasher = Hasher::new();
            // Order siblings correctly (smaller hash first)
            if current_hash.as_bytes() < sibling.as_bytes() {
                hasher.update(current_hash.as_bytes());
                hasher.update(sibling.as_bytes());
            } else {
                hasher.update(sibling.as_bytes());
                hasher.update(current_hash.as_bytes());
            }
            current_hash = hasher.finalize();
        }
        
        current_hash == proof.root_hash
    }
}

/// Compute BLAKE3 checksum of data
pub fn compute_checksum(data: &[u8]) -> Hash {
    blake3::hash(data)
}

/// Verify data integrity against expected hash
pub fn verify_data_integrity(data: &[u8], expected_hash: Hash) -> VerificationResult {
    let actual_hash = compute_checksum(data);
    
    if actual_hash == expected_hash {
        VerificationResult::Valid
    } else {
        VerificationResult::Corrupted {
            expected: expected_hash,
            actual: actual_hash,
        }
    }
}

/// Batch verification of multiple data blocks
pub fn batch_verify_integrity(
    data_blocks: &[(Hash, &[u8])],
) -> BatchVerificationResult {
    let start_time = std::time::Instant::now();
    let mut result = BatchVerificationResult {
        total_checked: data_blocks.len() as u64,
        valid_blocks: 0,
        corrupted_blocks: 0,
        missing_blocks: 0,
        failures: Vec::new(),
        duration: std::time::Duration::default(),
    };
    
    for &(expected_hash, data) in data_blocks {
        let verification = verify_data_integrity(data, expected_hash);
        
        match verification {
            VerificationResult::Valid => {
                result.valid_blocks += 1;
            }
            VerificationResult::Corrupted { .. } => {
                result.corrupted_blocks += 1;
                result.failures.push((expected_hash, verification));
            }
            VerificationResult::Missing => {
                result.missing_blocks += 1;
                result.failures.push((expected_hash, verification));
            }
            VerificationResult::Error(_) => {
                result.failures.push((expected_hash, verification));
            }
        }
    }
    
    result.duration = start_time.elapsed();
    result
}

/// Repair corrupted data using redundancy information
pub fn repair_corrupted_block(
    corrupted_data: &[u8],
    redundant_copies: &[&[u8]],
    expected_hash: Hash,
) -> CASResult<Vec<u8>> {
    // First, check if any redundant copy is valid
    for &copy in redundant_copies {
        if verify_data_integrity(copy, expected_hash) == VerificationResult::Valid {
            return Ok(copy.to_vec());
        }
    }
    
    // If no complete valid copy exists, attempt bit-level repair
    // This is a simplified implementation - in production, you'd use
    // more sophisticated error correction codes
    
    if redundant_copies.len() >= 2 {
        // Use majority voting for each byte position
        let mut repaired = corrupted_data.to_vec();
        
        for i in 0..corrupted_data.len() {
            let mut byte_candidates = vec![corrupted_data[i]];
            
            // Collect bytes from redundant copies
            for &copy in redundant_copies {
                if i < copy.len() {
                    byte_candidates.push(copy[i]);
                }
            }
            
            // Find most common byte value
            let mut byte_counts = HashMap::new();
            for &byte in &byte_candidates {
                *byte_counts.entry(byte).or_insert(0) += 1;
            }
            
            if let Some((&most_common_byte, _)) = byte_counts.iter()
                .max_by_key(|(_, &count)| count) {
                repaired[i] = most_common_byte;
            }
        }
        
        // Verify repair was successful
        if verify_data_integrity(&repaired, expected_hash) == VerificationResult::Valid {
            return Ok(repaired);
        }
    }
    
    Err(CASError::Corruption {
        details: "Unable to repair corrupted data".to_string(),
    })
}

/// Calculate integrity statistics for a dataset
pub fn calculate_integrity_statistics(
    verification_results: &[VerificationResult],
) -> IntegrityStatistics {
    let total = verification_results.len();
    let valid = verification_results.iter()
        .filter(|&r| matches!(r, VerificationResult::Valid))
        .count();
    let corrupted = verification_results.iter()
        .filter(|&r| matches!(r, VerificationResult::Corrupted { .. }))
        .count();
    let missing = verification_results.iter()
        .filter(|&r| matches!(r, VerificationResult::Missing))
        .count();
    let errors = verification_results.iter()
        .filter(|&r| matches!(r, VerificationResult::Error(_)))
        .count();
    
    IntegrityStatistics {
        total_blocks: total as u64,
        valid_blocks: valid as u64,
        corrupted_blocks: corrupted as u64,
        missing_blocks: missing as u64,
        error_blocks: errors as u64,
        integrity_rate: if total > 0 { valid as f64 / total as f64 } else { 0.0 },
    }
}

/// Integrity statistics for a dataset
#[derive(Debug, Clone)]
pub struct IntegrityStatistics {
    pub total_blocks: u64,
    pub valid_blocks: u64,
    pub corrupted_blocks: u64,
    pub missing_blocks: u64,
    pub error_blocks: u64,
    pub integrity_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checksum_computation() {
        let data = b"Hello, PRISM integrity verification!";
        let hash1 = compute_checksum(data);
        let hash2 = compute_checksum(data);
        
        // Same data should produce same hash
        assert_eq!(hash1, hash2);
        
        // Different data should produce different hash
        let different_data = b"Different data";
        let hash3 = compute_checksum(different_data);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_data_integrity_verification() {
        let data = b"Test data for integrity verification";
        let expected_hash = compute_checksum(data);
        
        // Valid data should pass verification
        let result = verify_data_integrity(data, expected_hash);
        assert_eq!(result, VerificationResult::Valid);
        
        // Corrupted data should fail verification
        let corrupted_data = b"Test data for integrity corruption";
        let result = verify_data_integrity(corrupted_data, expected_hash);
        assert!(matches!(result, VerificationResult::Corrupted { .. }));
    }

    #[test]
    fn test_merkle_tree_creation() {
        let data_blocks = vec![
            b"block 1".as_slice(),
            b"block 2".as_slice(),
            b"block 3".as_slice(),
            b"block 4".as_slice(),
        ];
        
        let leaf_hashes: Vec<Hash> = data_blocks.iter()
            .map(|data| compute_checksum(data))
            .collect();
        
        let tree = MerkleTree::from_leaves(leaf_hashes.clone()).unwrap();
        assert!(tree.root_hash().is_some());
        
        // Verify we can generate proofs for all leaves
        for &leaf_hash in &leaf_hashes {
            let proof = tree.generate_proof(leaf_hash).unwrap();
            assert_eq!(proof.data_hash, leaf_hash);
            assert_eq!(proof.root_hash, tree.root_hash().unwrap());
            
            // Verify the proof
            assert!(tree.verify_proof(&proof));
        }
    }

    #[test]
    fn test_batch_integrity_verification() {
        let test_data = vec![
            (b"data block 1".as_slice(), compute_checksum(b"data block 1")),
            (b"data block 2".as_slice(), compute_checksum(b"data block 2")),
            (b"data block 3".as_slice(), compute_checksum(b"data block 3")),
        ];
        
        let data_blocks: Vec<(Hash, &[u8])> = test_data.iter()
            .map(|(data, hash)| (*hash, *data))
            .collect();
        
        let result = batch_verify_integrity(&data_blocks);
        
        assert_eq!(result.total_checked, 3);
        assert_eq!(result.valid_blocks, 3);
        assert_eq!(result.corrupted_blocks, 0);
        assert!(result.failures.is_empty());
    }

    #[test]
    fn test_data_repair() {
        let original_data = b"This is the original correct data";
        let expected_hash = compute_checksum(original_data);
        
        // Create corrupted version
        let mut corrupted_data = original_data.to_vec();
        corrupted_data[10] = b'X'; // Corrupt one byte
        
        // Create redundant copies (one correct, one corrupted)
        let redundant_copies = vec![
            original_data.as_slice(),
            corrupted_data.as_slice(),
        ];
        
        // Attempt repair
        let repaired = repair_corrupted_block(
            &corrupted_data,
            &redundant_copies,
            expected_hash,
        ).unwrap();
        
        assert_eq!(repaired, original_data);
        assert_eq!(
            verify_data_integrity(&repaired, expected_hash),
            VerificationResult::Valid
        );
    }

    #[test]
    fn test_integrity_statistics() {
        let results = vec![
            VerificationResult::Valid,
            VerificationResult::Valid,
            VerificationResult::Corrupted {
                expected: compute_checksum(b"expected"),
                actual: compute_checksum(b"actual"),
            },
            VerificationResult::Missing,
            VerificationResult::Valid,
        ];
        
        let stats = calculate_integrity_statistics(&results);
        
        assert_eq!(stats.total_blocks, 5);
        assert_eq!(stats.valid_blocks, 3);
        assert_eq!(stats.corrupted_blocks, 1);
        assert_eq!(stats.missing_blocks, 1);
        assert_eq!(stats.integrity_rate, 0.6); // 3/5
    }
}