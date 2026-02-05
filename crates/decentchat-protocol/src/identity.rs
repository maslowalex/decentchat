//! Identity management for node authentication.
//!
//! Each node has a unique identity derived from an Ed25519 keypair.
//! The secret key can be persisted to disk for identity continuity across restarts.

use std::path::Path;

use decentchat_core::NodeId;
use iroh::SecretKey;

use crate::error::{ProtocolError, Result};

/// A node's cryptographic identity.
///
/// Wraps an iroh SecretKey and provides methods for deriving the public NodeId.
pub struct Identity {
    secret_key: SecretKey,
}

impl std::fmt::Debug for Identity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Don't expose the secret key in debug output.
        f.debug_struct("Identity")
            .field("node_id", &self.node_id())
            .finish()
    }
}

impl Identity {
    /// Generate a new random identity.
    pub fn generate() -> Self {
        let secret_key = SecretKey::generate(&mut rand::rng());
        Self { secret_key }
    }

    /// Load identity from file, or generate and persist a new one.
    ///
    /// The key is stored as raw 32 bytes. If the file exists but is malformed,
    /// returns an error rather than silently overwriting.
    pub fn load_or_generate(path: &Path) -> Result<Self> {
        assert!(
            !path.as_os_str().is_empty(),
            "identity path must not be empty"
        );

        if path.exists() {
            let bytes = std::fs::read(path).map_err(|e| {
                ProtocolError::IdentityError(format!("failed to read key file: {e}"))
            })?;

            let key_bytes: [u8; 32] = bytes.try_into().map_err(|_| {
                ProtocolError::IdentityError(
                    "key file has invalid length, expected 32 bytes".into(),
                )
            })?;

            let secret_key = SecretKey::from_bytes(&key_bytes);
            Ok(Self { secret_key })
        } else {
            let identity = Self::generate();
            identity.persist(path)?;
            Ok(identity)
        }
    }

    /// Persist the secret key to a file.
    fn persist(&self, path: &Path) -> Result<()> {
        // Ensure parent directory exists.
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent).map_err(|e| {
                ProtocolError::IdentityError(format!("failed to create key directory: {e}"))
            })?;
        }

        std::fs::write(path, self.secret_key.to_bytes())
            .map_err(|e| ProtocolError::IdentityError(format!("failed to write key file: {e}")))?;

        Ok(())
    }

    /// Get the public NodeId derived from this identity.
    pub fn node_id(&self) -> NodeId {
        let public_key = self.secret_key.public();
        NodeId::from_bytes(public_key.as_bytes().to_owned())
    }

    /// Get a reference to the underlying secret key.
    ///
    /// Used when constructing iroh Endpoints that need to authenticate as this identity.
    pub fn secret_key(&self) -> &SecretKey {
        &self.secret_key
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn generate_produces_unique_identities() {
        let id1 = Identity::generate();
        let id2 = Identity::generate();
        assert_ne!(id1.node_id(), id2.node_id());
    }

    #[test]
    fn node_id_is_deterministic() {
        let identity = Identity::generate();
        assert_eq!(identity.node_id(), identity.node_id());
    }

    #[test]
    fn load_or_generate_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_key");

        assert!(!path.exists());
        let identity = Identity::load_or_generate(&path).unwrap();
        assert!(path.exists());

        // File should contain 32 bytes.
        let bytes = fs::read(&path).unwrap();
        assert_eq!(bytes.len(), 32);

        // Reloading should produce the same identity.
        let reloaded = Identity::load_or_generate(&path).unwrap();
        assert_eq!(identity.node_id(), reloaded.node_id());
    }

    #[test]
    fn load_or_generate_rejects_invalid_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad_key");

        // Write an invalid key (wrong length).
        fs::write(&path, [0u8; 16]).unwrap();

        let result = Identity::load_or_generate(&path);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("invalid length"));
    }
}
