use thiserror::Error;

/// Main error type for PRISM operations
#[derive(Error, Debug)]
pub enum PrismError {
    #[error("Network error: {0}")]
    Network(#[from] NetworkError),
    
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),
    
    #[error("Consensus error: {0}")]
    Consensus(#[from] ConsensusError),
    
    #[error("CRDT error: {0}")]
    Crdt(#[from] CrdtError),
    
    #[error("Agent error: {source}")]
    Agent { 
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>
    },
    
    #[error("Configuration error: {message}")]
    Configuration { message: String },
    
    #[error("Authentication error: {message}")]
    Authentication { message: String },
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] SerializationError),
    
    #[error("Timeout error: operation timed out after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },
    
    #[error("Validation error: {message}")]
    Validation { message: String },
}

#[derive(Error, Debug)]
pub enum NetworkError {
    #[error("Connection failed to peer {peer_id}: {reason}")]
    ConnectionFailed {
        peer_id: String,
        reason: String,
    },
    
    #[error("Network partition detected: {details}")]
    NetworkPartition { details: String },
    
    #[error("Peer discovery failed: {reason}")]
    DiscoveryFailed { reason: String },
    
    #[error("Message routing failed: {destination} unreachable")]
    RoutingFailed { destination: String },
    
    #[error("Protocol error: {message}")]
    Protocol { message: String },
    
    #[error("Bandwidth limit exceeded: {current_mbps}Mbps > {limit_mbps}Mbps")]
    BandwidthLimit {
        current_mbps: u32,
        limit_mbps: u32,
    },
}

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Block not found: hash {hash}")]
    BlockNotFound { hash: String },
    
    #[error("Storage corruption detected: {details}")]
    Corruption { details: String },
    
    #[error("Storage full: {used_bytes} / {capacity_bytes} bytes")]
    StorageFull {
        used_bytes: u64,
        capacity_bytes: u64,
    },
    
    #[error("Index corruption: {reason}")]
    IndexCorruption { reason: String },
    
    #[error("Integrity check failed: expected {expected}, got {actual}")]
    IntegrityCheckFailed {
        expected: String,
        actual: String,
    },
    
    #[error("Database error: {0}")]
    Database(#[from] rocksdb::Error),
}

#[derive(Error, Debug)]
pub enum ConsensusError {
    #[error("No quorum: need {required} nodes, have {available}")]
    NoQuorum {
        required: usize,
        available: usize,
    },
    
    #[error("Byzantine behavior detected from node {node_id}")]
    ByzantineBehavior { node_id: String },
    
    #[error("Consensus timeout: no agreement after {timeout_ms}ms")]
    ConsensusTimeout { timeout_ms: u64 },
    
    #[error("Invalid proposal: {reason}")]
    InvalidProposal { reason: String },
    
    #[error("View change failed: {reason}")]
    ViewChangeFailed { reason: String },
    
    #[error("Leader election failed: {reason}")]
    LeaderElectionFailed { reason: String },
}

#[derive(Error, Debug)]
pub enum CrdtError {
    #[error("Merge conflict: {details}")]
    MergeConflict { details: String },
    
    #[error("Invalid operation: {operation} not applicable to current state")]
    InvalidOperation { operation: String },
    
    #[error("Clock skew detected: local={local_time}, remote={remote_time}")]
    ClockSkew {
        local_time: u64,
        remote_time: u64,
    },
    
    #[error("Causality violation: operation {operation_id} depends on missing operation")]
    CausalityViolation { operation_id: String },
}

#[derive(Error, Debug)]
pub enum SerializationError {
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    
    #[error("MessagePack error: {0}")]
    MessagePack(#[from] rmp_serde::encode::Error),
    
    #[error("MessagePack decode error: {0}")]
    MessagePackDecode(#[from] rmp_serde::decode::Error),
    
    #[error("Bincode error: {0}")]
    Bincode(#[from] Box<bincode::ErrorKind>),
}

/// Convenient Result type for PRISM operations
pub type PrismResult<T> = Result<T, PrismError>;

/// Trait for converting any error into a PrismError::Agent
pub trait IntoPrismError {
    fn into_prism_error(self) -> PrismError;
}

impl<E> IntoPrismError for E 
where 
    E: std::error::Error + Send + Sync + 'static
{
    fn into_prism_error(self) -> PrismError {
        PrismError::Agent {
            source: Box::new(self)
        }
    }
}

/// Macro for quick error creation
#[macro_export]
macro_rules! prism_error {
    ($variant:ident, $($field:ident: $value:expr),*) => {
        PrismError::$variant {
            $($field: $value),*
        }
    };
}

/// Macro for configuration errors
#[macro_export]
macro_rules! config_error {
    ($message:expr) => {
        PrismError::Configuration {
            message: $message.to_string()
        }
    };
    ($fmt:expr, $($arg:tt)*) => {
        PrismError::Configuration {
            message: format!($fmt, $($arg)*)
        }
    };
}

/// Macro for validation errors
#[macro_export]
macro_rules! validation_error {
    ($message:expr) => {
        PrismError::Validation {
            message: $message.to_string()
        }
    };
    ($fmt:expr, $($arg:tt)*) => {
        PrismError::Validation {
            message: format!($fmt, $($arg)*)
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_chain() {
        let storage_err = StorageError::BlockNotFound {
            hash: "abc123".to_string(),
        };
        let prism_err: PrismError = storage_err.into();
        
        assert!(matches!(prism_err, PrismError::Storage(_)));
        assert!(prism_err.to_string().contains("Block not found"));
    }

    #[test]
    fn test_error_macros() {
        let config_err = config_error!("Invalid port: {}", 65536);
        assert!(matches!(config_err, PrismError::Configuration { .. }));
        
        let validation_err = validation_error!("Agent ID cannot be empty");
        assert!(matches!(validation_err, PrismError::Validation { .. }));
    }
}