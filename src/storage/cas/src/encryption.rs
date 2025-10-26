use crate::{CASError, CASResult};
use rand::{thread_rng, RngCore};
use std::convert::TryInto;

/// Encryption utilities for Content-Addressable Storage
/// 
/// Supports multiple encryption algorithms for data confidentiality:
/// - AES-256-GCM: Authenticated encryption with 256-bit key
/// - ChaCha20Poly1305: Modern stream cipher with authentication
/// - Key derivation using Argon2 for password-based encryption

/// AES-256-GCM encryption key size
pub const AES_KEY_SIZE: usize = 32; // 256 bits
/// AES-256-GCM nonce size
pub const AES_NONCE_SIZE: usize = 12; // 96 bits
/// Authentication tag size
pub const AUTH_TAG_SIZE: usize = 16; // 128 bits

/// ChaCha20Poly1305 key size
pub const CHACHA_KEY_SIZE: usize = 32; // 256 bits
/// ChaCha20Poly1305 nonce size
pub const CHACHA_NONCE_SIZE: usize = 12; // 96 bits

/// Argon2 salt size for key derivation
pub const SALT_SIZE: usize = 16; // 128 bits

/// Encrypted data structure
#[derive(Debug, Clone)]
pub struct EncryptedData {
    /// Encrypted payload
    pub ciphertext: Vec<u8>,
    /// Nonce/IV used for encryption
    pub nonce: Vec<u8>,
    /// Authentication tag (for AEAD ciphers)
    pub tag: Vec<u8>,
    /// Salt used for key derivation (if applicable)
    pub salt: Option<Vec<u8>>,
}

/// Encrypt data using AES-256-GCM
#[cfg(feature = "aes-gcm")]
pub fn encrypt_aes_gcm(plaintext: &[u8], key: &[u8; AES_KEY_SIZE]) -> CASResult<EncryptedData> {
    use aes_gcm::{Aes256Gcm, KeyInit, AeadInPlace, Nonce};
    
    // Generate random nonce
    let mut nonce_bytes = [0u8; AES_NONCE_SIZE];
    thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    
    // Create cipher instance
    let cipher = Aes256Gcm::new(key.into());
    
    // Encrypt the data
    let mut buffer = plaintext.to_vec();
    let tag = cipher
        .encrypt_in_place_detached(nonce, b"", &mut buffer)
        .map_err(|e| CASError::Encryption(format!("AES-GCM encryption failed: {}", e)))?;
    
    Ok(EncryptedData {
        ciphertext: buffer,
        nonce: nonce_bytes.to_vec(),
        tag: tag.to_vec(),
        salt: None,
    })
}

/// Decrypt data using AES-256-GCM
#[cfg(feature = "aes-gcm")]
pub fn decrypt_aes_gcm(encrypted: &EncryptedData, key: &[u8; AES_KEY_SIZE]) -> CASResult<Vec<u8>> {
    use aes_gcm::{Aes256Gcm, KeyInit, AeadInPlace, Nonce, Tag};
    
    // Validate nonce and tag sizes
    if encrypted.nonce.len() != AES_NONCE_SIZE {
        return Err(CASError::Encryption("Invalid nonce size for AES-GCM".to_string()));
    }
    if encrypted.tag.len() != AUTH_TAG_SIZE {
        return Err(CASError::Encryption("Invalid tag size for AES-GCM".to_string()));
    }
    
    let nonce = Nonce::from_slice(&encrypted.nonce);
    let tag = Tag::from_slice(&encrypted.tag);
    
    // Create cipher instance
    let cipher = Aes256Gcm::new(key.into());
    
    // Decrypt the data
    let mut buffer = encrypted.ciphertext.clone();
    cipher
        .decrypt_in_place_detached(nonce, b"", &mut buffer, tag)
        .map_err(|e| CASError::Encryption(format!("AES-GCM decryption failed: {}", e)))?;
    
    Ok(buffer)
}

/// Encrypt data using ChaCha20Poly1305
#[cfg(feature = "chacha20poly1305")]
pub fn encrypt_chacha20poly1305(
    plaintext: &[u8], 
    key: &[u8; CHACHA_KEY_SIZE]
) -> CASResult<EncryptedData> {
    use chacha20poly1305::{ChaCha20Poly1305, KeyInit, AeadInPlace, Nonce};
    
    // Generate random nonce
    let mut nonce_bytes = [0u8; CHACHA_NONCE_SIZE];
    thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    
    // Create cipher instance
    let cipher = ChaCha20Poly1305::new(key.into());
    
    // Encrypt the data
    let mut buffer = plaintext.to_vec();
    let tag = cipher
        .encrypt_in_place_detached(nonce, b"", &mut buffer)
        .map_err(|e| CASError::Encryption(format!("ChaCha20Poly1305 encryption failed: {}", e)))?;
    
    Ok(EncryptedData {
        ciphertext: buffer,
        nonce: nonce_bytes.to_vec(),
        tag: tag.to_vec(),
        salt: None,
    })
}

/// Decrypt data using ChaCha20Poly1305
#[cfg(feature = "chacha20poly1305")]
pub fn decrypt_chacha20poly1305(
    encrypted: &EncryptedData, 
    key: &[u8; CHACHA_KEY_SIZE]
) -> CASResult<Vec<u8>> {
    use chacha20poly1305::{ChaCha20Poly1305, KeyInit, AeadInPlace, Nonce, Tag};
    
    // Validate nonce and tag sizes
    if encrypted.nonce.len() != CHACHA_NONCE_SIZE {
        return Err(CASError::Encryption("Invalid nonce size for ChaCha20Poly1305".to_string()));
    }
    if encrypted.tag.len() != AUTH_TAG_SIZE {
        return Err(CASError::Encryption("Invalid tag size for ChaCha20Poly1305".to_string()));
    }
    
    let nonce = Nonce::from_slice(&encrypted.nonce);
    let tag = Tag::from_slice(&encrypted.tag);
    
    // Create cipher instance
    let cipher = ChaCha20Poly1305::new(key.into());
    
    // Decrypt the data
    let mut buffer = encrypted.ciphertext.clone();
    cipher
        .decrypt_in_place_detached(nonce, b"", &mut buffer, tag)
        .map_err(|e| CASError::Encryption(format!("ChaCha20Poly1305 decryption failed: {}", e)))?;
    
    Ok(buffer)
}

/// Derive encryption key from password using Argon2
#[cfg(feature = "argon2")]
pub fn derive_key_from_password(
    password: &[u8], 
    salt: Option<&[u8]>
) -> CASResult<([u8; 32], Vec<u8>)> {
    use argon2::{Argon2, password_hash::{PasswordHasher, SaltString}};
    
    // Generate salt if not provided
    let salt_bytes = match salt {
        Some(s) => s.to_vec(),
        None => {
            let mut salt = vec![0u8; SALT_SIZE];
            thread_rng().fill_bytes(&mut salt);
            salt
        }
    };
    
    // Create Argon2 instance with secure defaults
    let argon2 = Argon2::default();
    
    // Create salt string for argon2
    let salt_string = SaltString::encode_b64(&salt_bytes)
        .map_err(|e| CASError::Encryption(format!("Salt encoding failed: {}", e)))?;
    
    // Derive key
    let hash = argon2
        .hash_password(password, &salt_string)
        .map_err(|e| CASError::Encryption(format!("Key derivation failed: {}", e)))?;
    
    let key_bytes = hash.hash
        .ok_or_else(|| CASError::Encryption("Hash extraction failed".to_string()))?
        .as_bytes();
    
    if key_bytes.len() < 32 {
        return Err(CASError::Encryption("Derived key too short".to_string()));
    }
    
    let mut key = [0u8; 32];
    key.copy_from_slice(&key_bytes[..32]);
    
    Ok((key, salt_bytes))
}

/// Generate a random encryption key
pub fn generate_random_key<const N: usize>() -> [u8; N] {
    let mut key = [0u8; N];
    thread_rng().fill_bytes(&mut key);
    key
}

/// Securely wipe sensitive data from memory
pub fn secure_wipe(data: &mut [u8]) {
    use std::ptr::write_volatile;
    use std::sync::atomic::{compiler_fence, Ordering};
    
    for byte in data.iter_mut() {
        unsafe {
            write_volatile(byte, 0);
        }
    }
    compiler_fence(Ordering::SeqCst);
}

/// Key management structure for storing encryption keys
pub struct KeyManager {
    /// Master key for encrypting other keys
    master_key: [u8; 32],
    /// Derived keys cache (in production, use secure key storage)
    key_cache: std::collections::HashMap<String, [u8; 32]>,
}

impl KeyManager {
    /// Create a new key manager with a random master key
    pub fn new() -> Self {
        Self {
            master_key: generate_random_key::<32>(),
            key_cache: std::collections::HashMap::new(),
        }
    }
    
    /// Create a new key manager from an existing master key
    pub fn from_master_key(master_key: [u8; 32]) -> Self {
        Self {
            master_key,
            key_cache: std::collections::HashMap::new(),
        }
    }
    
    /// Derive a key for a specific purpose (e.g., storage block encryption)
    pub fn derive_key(&mut self, purpose: &str) -> CASResult<[u8; 32]> {
        if let Some(&key) = self.key_cache.get(purpose) {
            return Ok(key);
        }
        
        // Use HKDF to derive purpose-specific keys
        #[cfg(feature = "hkdf")]
        {
            use hkdf::Hkdf;
            use sha2::Sha256;
            
            let hk = Hkdf::<Sha256>::new(None, &self.master_key);
            let mut okm = [0u8; 32];
            hk.expand(purpose.as_bytes(), &mut okm)
                .map_err(|e| CASError::Encryption(format!("Key derivation failed: {}", e)))?;
            
            self.key_cache.insert(purpose.to_string(), okm);
            Ok(okm)
        }
        
        #[cfg(not(feature = "hkdf"))]
        {
            // Fallback: simple key derivation using BLAKE3
            let mut hasher = blake3::Hasher::new();
            hasher.update(&self.master_key);
            hasher.update(purpose.as_bytes());
            let hash = hasher.finalize();
            
            let key: [u8; 32] = hash.as_bytes()[..32].try_into()
                .map_err(|_| CASError::Encryption("Key derivation failed".to_string()))?;
            
            self.key_cache.insert(purpose.to_string(), key);
            Ok(key)
        }
    }
    
    /// Clear sensitive data from memory
    pub fn clear(&mut self) {
        secure_wipe(&mut self.master_key);
        for (_, key) in self.key_cache.iter_mut() {
            secure_wipe(key);
        }
        self.key_cache.clear();
    }
}

impl Drop for KeyManager {
    fn drop(&mut self) {
        self.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_generation() {
        let key1 = generate_random_key::<32>();
        let key2 = generate_random_key::<32>();
        
        // Keys should be different
        assert_ne!(key1, key2);
        
        // Keys should not be all zeros
        assert_ne!(key1, [0u8; 32]);
        assert_ne!(key2, [0u8; 32]);
    }

    #[test]
    fn test_secure_wipe() {
        let mut data = [0x42u8; 32];
        assert_eq!(data[0], 0x42);
        
        secure_wipe(&mut data);
        assert_eq!(data[0], 0x00);
    }

    #[test]
    fn test_key_manager() {
        let mut km = KeyManager::new();
        
        let key1 = km.derive_key("purpose1").unwrap();
        let key2 = km.derive_key("purpose2").unwrap();
        let key1_again = km.derive_key("purpose1").unwrap();
        
        // Different purposes should yield different keys
        assert_ne!(key1, key2);
        
        // Same purpose should yield same key (cached)
        assert_eq!(key1, key1_again);
    }

    #[cfg(all(feature = "aes-gcm", feature = "argon2"))]
    #[test]
    fn test_password_based_encryption() {
        let password = b"strong_password_123";
        let plaintext = b"This is secret data that needs encryption";
        
        // Derive key from password
        let (key, salt) = derive_key_from_password(password, None).unwrap();
        
        // Encrypt data
        let encrypted = encrypt_aes_gcm(plaintext, &key).unwrap();
        
        // Verify ciphertext is different from plaintext
        assert_ne!(encrypted.ciphertext, plaintext);
        
        // Decrypt and verify
        let decrypted = decrypt_aes_gcm(&encrypted, &key).unwrap();
        assert_eq!(decrypted, plaintext);
        
        // Verify that same password produces same key with same salt
        let (key2, _) = derive_key_from_password(password, Some(&salt)).unwrap();
        assert_eq!(key, key2);
    }

    #[cfg(feature = "aes-gcm")]
    #[test]
    fn test_aes_gcm_encryption() {
        let key = generate_random_key::<AES_KEY_SIZE>();
        let plaintext = b"Hello, this is a test message for AES-GCM encryption!";
        
        // Encrypt
        let encrypted = encrypt_aes_gcm(plaintext, &key).unwrap();
        
        // Verify structure
        assert!(!encrypted.ciphertext.is_empty());
        assert_eq!(encrypted.nonce.len(), AES_NONCE_SIZE);
        assert_eq!(encrypted.tag.len(), AUTH_TAG_SIZE);
        
        // Ciphertext should be different from plaintext
        assert_ne!(encrypted.ciphertext, plaintext);
        
        // Decrypt and verify
        let decrypted = decrypt_aes_gcm(&encrypted, &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[cfg(feature = "chacha20poly1305")]
    #[test]
    fn test_chacha20poly1305_encryption() {
        let key = generate_random_key::<CHACHA_KEY_SIZE>();
        let plaintext = b"Hello, this is a test message for ChaCha20Poly1305 encryption!";
        
        // Encrypt
        let encrypted = encrypt_chacha20poly1305(plaintext, &key).unwrap();
        
        // Verify structure
        assert!(!encrypted.ciphertext.is_empty());
        assert_eq!(encrypted.nonce.len(), CHACHA_NONCE_SIZE);
        assert_eq!(encrypted.tag.len(), AUTH_TAG_SIZE);
        
        // Ciphertext should be different from plaintext
        assert_ne!(encrypted.ciphertext, plaintext);
        
        // Decrypt and verify
        let decrypted = decrypt_chacha20poly1305(&encrypted, &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }
}