use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

pub mod offline_sync;
pub mod resource_manager;
pub mod system_notifications;
pub mod file_watcher;

/// Grahmos OS Integration for PRISM Offline-First Operation
/// 
/// Provides interfaces for integrating PRISM with Grahmos OS capabilities:
/// - Offline-first data synchronization
/// - Resource quotas and management
/// - System notifications and events
/// - File system integration and watching
/// - Power management and device state

/// System resource information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemResources {
    /// Available storage space in bytes
    pub storage_available: u64,
    /// Total storage space in bytes
    pub storage_total: u64,
    /// Available memory in bytes
    pub memory_available: u64,
    /// Total memory in bytes
    pub memory_total: u64,
    /// CPU usage percentage (0-100)
    pub cpu_usage: f64,
    /// Network connectivity status
    pub network_status: NetworkStatus,
    /// Battery level (0-100, None if not applicable)
    pub battery_level: Option<u8>,
    /// Power source (battery, AC, etc.)
    pub power_source: PowerSource,
}

/// Network connectivity status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetworkStatus {
    Online,
    Offline,
    Limited,  // Restricted connectivity
    Metered,  // Data usage restrictions
}

/// Power source information
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PowerSource {
    Battery,
    AC,
    USB,
    Wireless,
    Unknown,
}

/// Device state for offline optimization
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceState {
    Active,      // Full performance mode
    PowerSaving, // Reduced performance to save battery
    Sleeping,    // Minimal operations
    Background,  // Running in background
}

/// System notification types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SystemNotification {
    ResourceLow { resource_type: String, threshold: f64 },
    NetworkChanged { new_status: NetworkStatus },
    PowerSourceChanged { new_source: PowerSource },
    DeviceStateChanged { new_state: DeviceState },
    FileSystemEvent { path: PathBuf, event_type: String },
    StorageQuotaWarning { used_bytes: u64, limit_bytes: u64 },
    SyncCompleted { items_synced: u64, bytes_synced: u64 },
    SyncFailed { error: String, retry_count: u32 },
}

/// Grahmos OS integration errors
#[derive(thiserror::Error, Debug)]
pub enum GrahmosError {
    #[error("System resource unavailable: {resource}")]
    ResourceUnavailable { resource: String },
    
    #[error("Insufficient permissions for operation: {operation}")]
    PermissionDenied { operation: String },
    
    #[error("Quota exceeded: {quota_type} limit {limit}")]
    QuotaExceeded { quota_type: String, limit: u64 },
    
    #[error("Offline operation failed: {reason}")]
    OfflineError { reason: String },
    
    #[error("System integration error: {0}")]
    SystemError(String),
    
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

pub type GrahmosResult<T> = Result<T, GrahmosError>;

/// Main Grahmos OS integration interface
#[async_trait::async_trait]
pub trait GrahmosIntegration: Send + Sync {
    /// Get current system resources
    async fn get_system_resources(&self) -> GrahmosResult<SystemResources>;
    
    /// Register for system notifications
    async fn subscribe_notifications(&self) -> GrahmosResult<mpsc::UnboundedReceiver<SystemNotification>>;
    
    /// Set resource quotas for PRISM operation
    async fn set_resource_quotas(&self, quotas: ResourceQuotas) -> GrahmosResult<()>;
    
    /// Start offline synchronization
    async fn start_offline_sync(&self) -> GrahmosResult<()>;
    
    /// Stop offline synchronization
    async fn stop_offline_sync(&self) -> GrahmosResult<()>;
    
    /// Get offline sync status
    async fn get_sync_status(&self) -> GrahmosResult<SyncStatus>;
    
    /// Request permission for operation
    async fn request_permission(&self, permission: Permission) -> GrahmosResult<bool>;
    
    /// Register file system watcher
    async fn watch_directory(&self, path: PathBuf) -> GrahmosResult<()>;
    
    /// Unregister file system watcher
    async fn unwatch_directory(&self, path: PathBuf) -> GrahmosResult<()>;
    
    /// Optimize for device state
    async fn optimize_for_device_state(&self, state: DeviceState) -> GrahmosResult<()>;
}

/// Resource quotas configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceQuotas {
    /// Maximum storage usage in bytes
    pub max_storage_bytes: u64,
    /// Maximum memory usage in bytes
    pub max_memory_bytes: u64,
    /// Maximum CPU usage percentage
    pub max_cpu_percent: f64,
    /// Maximum network bandwidth in bytes/second
    pub max_bandwidth_bps: u64,
    /// Maximum number of open files
    pub max_open_files: u32,
}

/// Offline synchronization status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStatus {
    /// Whether sync is currently active
    pub is_syncing: bool,
    /// Items pending synchronization
    pub pending_items: u64,
    /// Last successful sync timestamp
    pub last_sync: Option<chrono::DateTime<chrono::Utc>>,
    /// Current sync progress (0.0-1.0)
    pub sync_progress: f64,
    /// Number of failed sync attempts
    pub failed_attempts: u32,
    /// Next scheduled sync time
    pub next_sync: Option<chrono::DateTime<chrono::Utc>>,
}

/// System permissions that may be required
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Permission {
    FileSystemAccess,
    NetworkAccess,
    NotificationAccess,
    BackgroundExecution,
    ResourceMonitoring,
    SystemIntegration,
}

/// Mock implementation for development and testing
pub struct MockGrahmosIntegration {
    resources: SystemResources,
    notification_sender: Option<mpsc::UnboundedSender<SystemNotification>>,
}

impl MockGrahmosIntegration {
    pub fn new() -> Self {
        Self {
            resources: SystemResources {
                storage_available: 100 * 1024 * 1024 * 1024, // 100GB
                storage_total: 1024 * 1024 * 1024 * 1024,    // 1TB
                memory_available: 8 * 1024 * 1024 * 1024,    // 8GB
                memory_total: 16 * 1024 * 1024 * 1024,       // 16GB
                cpu_usage: 25.0,
                network_status: NetworkStatus::Online,
                battery_level: Some(75),
                power_source: PowerSource::Battery,
            },
            notification_sender: None,
        }
    }
}

#[async_trait::async_trait]
impl GrahmosIntegration for MockGrahmosIntegration {
    async fn get_system_resources(&self) -> GrahmosResult<SystemResources> {
        Ok(self.resources.clone())
    }
    
    async fn subscribe_notifications(&self) -> GrahmosResult<mpsc::UnboundedReceiver<SystemNotification>> {
        let (sender, receiver) = mpsc::unbounded_channel();
        // Store sender for mock notifications if needed
        Ok(receiver)
    }
    
    async fn set_resource_quotas(&self, _quotas: ResourceQuotas) -> GrahmosResult<()> {
        info!("Resource quotas set (mock implementation)");
        Ok(())
    }
    
    async fn start_offline_sync(&self) -> GrahmosResult<()> {
        info!("Offline sync started (mock implementation)");
        Ok(())
    }
    
    async fn stop_offline_sync(&self) -> GrahmosResult<()> {
        info!("Offline sync stopped (mock implementation)");
        Ok(())
    }
    
    async fn get_sync_status(&self) -> GrahmosResult<SyncStatus> {
        Ok(SyncStatus {
            is_syncing: false,
            pending_items: 0,
            last_sync: Some(chrono::Utc::now()),
            sync_progress: 1.0,
            failed_attempts: 0,
            next_sync: Some(chrono::Utc::now() + chrono::Duration::minutes(30)),
        })
    }
    
    async fn request_permission(&self, permission: Permission) -> GrahmosResult<bool> {
        info!("Permission {:?} granted (mock implementation)", permission);
        Ok(true)
    }
    
    async fn watch_directory(&self, path: PathBuf) -> GrahmosResult<()> {
        info!("Watching directory {:?} (mock implementation)", path);
        Ok(())
    }
    
    async fn unwatch_directory(&self, path: PathBuf) -> GrahmosResult<()> {
        info!("Stopped watching directory {:?} (mock implementation)", path);
        Ok(())
    }
    
    async fn optimize_for_device_state(&self, state: DeviceState) -> GrahmosResult<()> {
        info!("Optimized for device state {:?} (mock implementation)", state);
        Ok(())
    }
}

/// Configuration for Grahmos integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrahmosConfig {
    /// Enable offline-first operation
    pub enable_offline_mode: bool,
    /// Sync interval in seconds
    pub sync_interval_seconds: u64,
    /// Resource monitoring interval in seconds
    pub monitor_interval_seconds: u64,
    /// Enable system notifications
    pub enable_notifications: bool,
    /// Default resource quotas
    pub default_quotas: ResourceQuotas,
    /// Directories to watch for changes
    pub watch_directories: Vec<PathBuf>,
}

impl Default for GrahmosConfig {
    fn default() -> Self {
        Self {
            enable_offline_mode: true,
            sync_interval_seconds: 300,  // 5 minutes
            monitor_interval_seconds: 60, // 1 minute
            enable_notifications: true,
            default_quotas: ResourceQuotas {
                max_storage_bytes: 10 * 1024 * 1024 * 1024, // 10GB
                max_memory_bytes: 1024 * 1024 * 1024,       // 1GB
                max_cpu_percent: 50.0,
                max_bandwidth_bps: 10 * 1024 * 1024,        // 10MB/s
                max_open_files: 1000,
            },
            watch_directories: vec![],
        }
    }
}