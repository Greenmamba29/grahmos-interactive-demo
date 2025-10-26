use prism_core_error::*;
use std::error::Error;
use pretty_assertions::assert_eq;

/// Comprehensive error handling tests
/// 
/// Tests cover:
/// - Error type conversion and chaining
/// - Error message formatting and clarity
/// - Serialization/deserialization of errors
/// - Error propagation through async boundaries
/// - Error recovery scenarios
/// - Custom error creation macros

#[cfg(test)]
mod error_type_tests {
    use super::*;

    #[test]
    fn test_network_error_creation() {
        let error = NetworkError::ConnectionFailed {
            peer_id: "node-123".to_string(),
            reason: "Timeout after 30s".to_string(),
        };
        
        assert!(error.to_string().contains("node-123"));
        assert!(error.to_string().contains("Timeout after 30s"));
        
        // Test error source chain
        let prism_error: PrismError = error.into();
        assert!(matches!(prism_error, PrismError::Network(_)));
    }
    
    #[test]
    fn test_storage_error_creation() {
        let error = StorageError::BlockNotFound {
            hash: "abc123def456".to_string(),
        };
        
        let formatted = error.to_string();
        assert!(formatted.contains("Block not found"));
        assert!(formatted.contains("abc123def456"));
        
        // Test conversion to PrismError
        let prism_error: PrismError = error.into();
        assert!(matches!(prism_error, PrismError::Storage(_)));
    }
    
    #[test]
    fn test_consensus_error_creation() {
        let error = ConsensusError::NoQuorum {
            required: 3,
            available: 2,
        };
        
        let formatted = error.to_string();
        assert!(formatted.contains("No quorum"));
        assert!(formatted.contains("need 3"));
        assert!(formatted.contains("have 2"));
        
        // Test error chain
        let prism_error: PrismError = error.into();
        match prism_error {
            PrismError::Consensus(ConsensusError::NoQuorum { required, available }) => {
                assert_eq!(required, 3);
                assert_eq!(available, 2);
            },
            _ => panic!("Unexpected error type"),
        }
    }
    
    #[test]
    fn test_crdt_error_creation() {
        let error = CrdtError::MergeConflict {
            details: "Vector clock inconsistency".to_string(),
        };
        
        let formatted = error.to_string();
        assert!(formatted.contains("Merge conflict"));
        assert!(formatted.contains("Vector clock inconsistency"));
        
        // Test conversion chain
        let prism_error: PrismError = error.into();
        assert!(matches!(prism_error, PrismError::Crdt(_)));
    }
    
    #[test]
    fn test_serialization_error_chain() {
        let json_error = serde_json::Error::syntax(
            serde_json::error::ErrorCode::InvalidNumber,
            0,
            0
        );
        
        let ser_error = SerializationError::Json(json_error);
        let prism_error: PrismError = ser_error.into();
        
        assert!(matches!(prism_error, PrismError::Serialization(_)));
        assert!(prism_error.to_string().contains("JSON error"));
    }
}

#[cfg(test)]
mod error_macro_tests {
    use super::*;
    
    #[test]
    fn test_config_error_macro() {
        let error = config_error!("Invalid port: {}", 65536);
        
        match error {
            PrismError::Configuration { message } => {
                assert_eq!(message, "Invalid port: 65536");
            },
            _ => panic!("Expected Configuration error"),
        }
    }
    
    #[test]
    fn test_validation_error_macro() {
        let error = validation_error!("Agent ID cannot be empty");
        
        match error {
            PrismError::Validation { message } => {
                assert_eq!(message, "Agent ID cannot be empty");
            },
            _ => panic!("Expected Validation error"),
        }
    }
    
    #[test]
    fn test_prism_error_macro() {
        let error = prism_error!(Timeout, timeout_ms: 5000);
        
        match error {
            PrismError::Timeout { timeout_ms } => {
                assert_eq!(timeout_ms, 5000);
            },
            _ => panic!("Expected Timeout error"),
        }
    }
}

#[cfg(test)]
mod error_conversion_tests {
    use super::*;
    
    #[test]
    fn test_rocksdb_error_conversion() {
        let rocks_error = rocksdb::Error::new("Database corruption detected".to_string());
        let storage_error = StorageError::Database(rocks_error);
        let prism_error: PrismError = storage_error.into();
        
        assert!(matches!(prism_error, PrismError::Storage(StorageError::Database(_))));
        assert!(prism_error.to_string().contains("Database corruption"));
    }
    
    #[test]
    fn test_arbitrary_error_conversion() {
        #[derive(Debug)]
        struct CustomError(&'static str);
        
        impl std::fmt::Display for CustomError {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "Custom error: {}", self.0)
            }
        }
        
        impl std::error::Error for CustomError {}
        
        let custom = CustomError("Something went wrong");
        let prism_error = custom.into_prism_error();
        
        match prism_error {
            PrismError::Agent { source } => {
                assert!(source.to_string().contains("Something went wrong"));
            },
            _ => panic!("Expected Agent error"),
        }
    }
    
    #[test]
    fn test_error_source_chain() {
        let network_error = NetworkError::ConnectionFailed {
            peer_id: "test-node".to_string(),
            reason: "Connection refused".to_string(),
        };
        
        let prism_error: PrismError = network_error.into();
        
        // Test source chain traversal
        let mut current: &dyn Error = &prism_error;
        let mut chain_length = 0;
        
        while let Some(source) = current.source() {
            current = source;
            chain_length += 1;
            if chain_length > 10 {
                panic!("Error chain too deep - possible cycle");
            }
        }
        
        assert!(chain_length > 0, "Error should have a source");
    }
}

#[cfg(test)]
mod error_formatting_tests {
    use super::*;
    
    #[test]
    fn test_network_error_formatting() {
        let error = NetworkError::BandwidthLimit {
            current_mbps: 150,
            limit_mbps: 100,
        };
        
        let formatted = format!("{}", error);
        assert!(formatted.contains("Bandwidth limit exceeded"));
        assert!(formatted.contains("150Mbps"));
        assert!(formatted.contains("100Mbps"));
        
        // Test debug formatting
        let debug_formatted = format!("{:?}", error);
        assert!(debug_formatted.contains("BandwidthLimit"));
    }
    
    #[test]
    fn test_storage_error_formatting() {
        let error = StorageError::StorageFull {
            used_bytes: 1024 * 1024 * 1024, // 1GB
            capacity_bytes: 1024 * 1024 * 1024, // 1GB
        };
        
        let formatted = format!("{}", error);
        assert!(formatted.contains("Storage full"));
        assert!(formatted.contains("1073741824")); // Bytes value
    }
    
    #[test]
    fn test_consensus_error_formatting() {
        let error = ConsensusError::ByzantineBehavior {
            node_id: "malicious-node-42".to_string(),
        };
        
        let formatted = format!("{}", error);
        assert!(formatted.contains("Byzantine behavior"));
        assert!(formatted.contains("malicious-node-42"));
    }
    
    #[test]
    fn test_error_message_clarity() {
        // Test that error messages are clear and actionable
        let errors = vec![
            PrismError::Network(NetworkError::ConnectionFailed {
                peer_id: "node-1".to_string(),
                reason: "Timeout".to_string(),
            }),
            PrismError::Storage(StorageError::IntegrityCheckFailed {
                expected: "abc123".to_string(),
                actual: "def456".to_string(),
            }),
            PrismError::Consensus(ConsensusError::InvalidProposal {
                reason: "Invalid signature".to_string(),
            }),
        ];
        
        for error in errors {
            let message = error.to_string();
            
            // Error messages should not be empty
            assert!(!message.is_empty());
            
            // Should contain meaningful context
            assert!(message.len() > 10);
            
            // Should not contain debug-only information in user message
            assert!(!message.contains("Debug"));
            assert!(!message.contains("debug"));
        }
    }
}

#[cfg(test)]
mod async_error_propagation_tests {
    use super::*;
    use tokio_test;
    
    async fn failing_network_operation() -> PrismResult<()> {
        Err(NetworkError::ConnectionFailed {
            peer_id: "test-peer".to_string(),
            reason: "Connection timeout".to_string(),
        }.into())
    }
    
    async fn failing_storage_operation() -> PrismResult<Vec<u8>> {
        Err(StorageError::BlockNotFound {
            hash: "missing-block".to_string(),
        }.into())
    }
    
    async fn operation_that_propagates_errors() -> PrismResult<Vec<u8>> {
        failing_network_operation().await?;
        failing_storage_operation().await
    }
    
    #[tokio::test]
    async fn test_error_propagation_through_async() {
        let result = operation_that_propagates_errors().await;
        
        assert!(result.is_err());
        
        match result.unwrap_err() {
            PrismError::Network(NetworkError::ConnectionFailed { peer_id, .. }) => {
                assert_eq!(peer_id, "test-peer");
            },
            other => panic!("Unexpected error type: {:?}", other),
        }
    }
    
    #[tokio::test]
    async fn test_timeout_error_with_actual_timeout() {
        use tokio::time::{timeout, Duration};
        
        async fn long_running_operation() -> PrismResult<()> {
            tokio::time::sleep(Duration::from_secs(2)).await;
            Ok(())
        }
        
        let result = timeout(Duration::from_millis(100), long_running_operation()).await;
        
        match result {
            Err(_timeout_error) => {
                // Convert timeout to our error type
                let prism_error = PrismError::Timeout { timeout_ms: 100 };
                assert!(prism_error.to_string().contains("timed out"));
                assert!(prism_error.to_string().contains("100ms"));
            },
            Ok(_) => panic!("Operation should have timed out"),
        }
    }
}

#[cfg(test)]
mod error_recovery_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_network_error_recovery() {
        async fn network_operation_with_retry(max_attempts: u32) -> PrismResult<String> {
            for attempt in 1..=max_attempts {
                let result = if attempt < max_attempts {
                    Err(NetworkError::ConnectionFailed {
                        peer_id: format!("peer-{}", attempt),
                        reason: "Temporary failure".to_string(),
                    }.into())
                } else {
                    Ok("Success".to_string())
                };
                
                match result {
                    Ok(value) => return Ok(value),
                    Err(PrismError::Network(NetworkError::ConnectionFailed { .. })) => {
                        // Retry on network errors
                        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                        continue;
                    },
                    Err(other) => return Err(other), // Don't retry other errors
                }
            }
            
            Err(PrismError::Network(NetworkError::ConnectionFailed {
                peer_id: "all-peers".to_string(),
                reason: "Max retries exceeded".to_string(),
            }))
        }
        
        let result = network_operation_with_retry(3).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Success");
    }
    
    #[test]
    fn test_error_context_preservation() {
        let original_error = StorageError::Corruption {
            details: "Checksum mismatch in block 42".to_string(),
        };
        
        let prism_error: PrismError = original_error.into();
        
        // Context should be preserved through conversion
        let error_string = prism_error.to_string();
        assert!(error_string.contains("Corruption"));
        assert!(error_string.contains("block 42"));
        assert!(error_string.contains("Checksum"));
    }
}

#[cfg(test)]
mod error_serialization_tests {
    use super::*;
    use serde_json;
    
    // Note: In a real implementation, errors would need to implement Serialize/Deserialize
    // This test shows the structure for when that's implemented
    
    #[test]
    fn test_error_metadata_extraction() {
        let error = PrismError::Consensus(ConsensusError::NoQuorum {
            required: 5,
            available: 3,
        });
        
        // Extract structured information from errors for logging/metrics
        let error_type = match &error {
            PrismError::Network(_) => "network",
            PrismError::Storage(_) => "storage", 
            PrismError::Consensus(_) => "consensus",
            PrismError::Crdt(_) => "crdt",
            PrismError::Agent { .. } => "agent",
            PrismError::Configuration { .. } => "configuration",
            PrismError::Authentication { .. } => "authentication",
            PrismError::Serialization(_) => "serialization",
            PrismError::Timeout { .. } => "timeout",
            PrismError::Validation { .. } => "validation",
        };
        
        assert_eq!(error_type, "consensus");
        
        // Extract severity level
        let is_recoverable = matches!(error, 
            PrismError::Network(NetworkError::ConnectionFailed { .. }) |
            PrismError::Timeout { .. }
        );
        
        let is_critical = matches!(error,
            PrismError::Storage(StorageError::Corruption { .. }) |
            PrismError::Consensus(ConsensusError::ByzantineBehavior { .. })
        );
        
        assert!(!is_recoverable); // NoQuorum is not easily recoverable
        assert!(!is_critical); // NoQuorum is not data corruption
    }
    
    #[test]
    fn test_error_aggregation() {
        // Test collecting multiple errors (useful for batch operations)
        let errors = vec![
            PrismError::Network(NetworkError::ConnectionFailed {
                peer_id: "node-1".to_string(),
                reason: "Timeout".to_string(),
            }),
            PrismError::Storage(StorageError::BlockNotFound {
                hash: "abc123".to_string(),
            }),
            PrismError::Validation {
                message: "Invalid input".to_string(),
            },
        ];
        
        // Count errors by type
        let mut error_counts = std::collections::HashMap::new();
        for error in &errors {
            let error_type = match error {
                PrismError::Network(_) => "network",
                PrismError::Storage(_) => "storage",
                PrismError::Validation { .. } => "validation",
                _ => "other",
            };
            
            *error_counts.entry(error_type).or_insert(0) += 1;
        }
        
        assert_eq!(error_counts.get("network"), Some(&1));
        assert_eq!(error_counts.get("storage"), Some(&1));
        assert_eq!(error_counts.get("validation"), Some(&1));
    }
}