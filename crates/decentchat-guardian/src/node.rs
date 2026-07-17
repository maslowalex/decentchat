use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use decentchat_core::{NodeId, RoomMetadata, SCHEMA_VERSION};
use guardian_db::guardian::GuardianDB;
use guardian_db::guardian::core::NewGuardianDBOptions;
use guardian_db::p2p::network::client::IrohClient;
use guardian_db::p2p::network::config::ClientConfig;
use guardian_db::traits::CreateDBOptions;
use iroh_docs::DocTicket;

use crate::error::{GuardianAdapterError, Result};
use crate::session::{RoomSession, SessionConfig, SessionEventReceiver};
use crate::store::{GuardianRoomStore, RoomStore};

const NODE_SECRET_FILE: &str = "node_secret.key";

#[derive(Clone, Debug)]
pub struct GuardianNodeConfig {
    pub data_dir: PathBuf,
    pub legacy_identity_path: Option<PathBuf>,
    pub port: u16,
    pub local_only: bool,
}

impl GuardianNodeConfig {
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
            legacy_identity_path: None,
            port: 0,
            local_only: false,
        }
    }
}

#[derive(Clone)]
pub struct GuardianNode {
    client: Arc<IrohClient>,
    db: Arc<GuardianDB>,
    data_dir: PathBuf,
}

impl GuardianNode {
    pub async fn open(config: GuardianNodeConfig) -> Result<Self> {
        std::fs::create_dir_all(&config.data_dir).map_err(|error| {
            GuardianAdapterError::IdentityMigration(format!(
                "cannot create {}: {error}",
                config.data_dir.display()
            ))
        })?;
        if let Some(legacy) = config.legacy_identity_path.as_deref() {
            migrate_legacy_identity(legacy, &config.data_dir)?;
        }

        let client_config = ClientConfig {
            data_store_path: Some(config.data_dir.clone()),
            port: config.port,
            enable_discovery_mdns: true,
            enable_discovery_n0: !config.local_only,
            ..Default::default()
        };

        let client = Arc::new(IrohClient::new(client_config).await?);
        let db = GuardianDB::new(
            client.as_ref().clone(),
            Some(NewGuardianDBOptions {
                directory: Some(config.data_dir.join("db")),
                backend: Some(client.backend().clone()),
                ..Default::default()
            }),
        )
        .await?;

        Ok(Self {
            client,
            db: Arc::new(db),
            data_dir: config.data_dir,
        })
    }

    pub fn node_id(&self) -> NodeId {
        NodeId::from_bytes(*self.client.node_id().as_bytes())
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn identity_path(&self) -> PathBuf {
        self.data_dir.join(NODE_SECRET_FILE)
    }

    pub async fn peer_count(&self) -> usize {
        self.client.backend().list_active_connections().await.len()
    }

    pub async fn create_room(
        &self,
        name: &str,
        config: SessionConfig,
    ) -> Result<(RoomSession, SessionEventReceiver)> {
        if name.trim().is_empty() {
            return Err(GuardianAdapterError::InvalidRecord {
                key: "meta/room".into(),
                reason: "room name must not be empty".into(),
            });
        }

        let store_name = format!("room-{}", short_hash(name.as_bytes()));
        let store = self.db.key_value(&store_name, None).await?;
        let store: Arc<dyn RoomStore> = Arc::new(GuardianRoomStore::new(store));

        if store.get("meta/room").await?.is_none() {
            let metadata = RoomMetadata {
                version: SCHEMA_VERSION,
                name: name.to_owned(),
                created_at_ms: now_ms(),
            };
            store
                .put(
                    "meta/room",
                    serde_json::to_vec(&metadata).map_err(|error| {
                        GuardianAdapterError::InvalidRecord {
                            key: "meta/room".into(),
                            reason: error.to_string(),
                        }
                    })?,
                )
                .await?;
        }

        RoomSession::open(store, self.node_id(), config).await
    }

    pub async fn join_room(
        &self,
        ticket: &str,
        config: SessionConfig,
    ) -> Result<(RoomSession, SessionEventReceiver)> {
        validate_ticket(ticket)?;
        let store_name = format!("ticket-{}", short_hash(ticket.as_bytes()));
        let options = CreateDBOptions {
            doc_ticket: Some(ticket.to_owned()),
            ..Default::default()
        };
        let store = self.db.key_value(&store_name, Some(options)).await?;
        let store: Arc<dyn RoomStore> = Arc::new(GuardianRoomStore::new(store));

        let deadline = tokio::time::Instant::now() + config.join_timeout;
        while store.get("meta/room").await?.is_none() {
            if tokio::time::Instant::now() >= deadline {
                return Err(GuardianAdapterError::RoomMetadataTimeout(
                    config.join_timeout,
                ));
            }
            tokio::time::sleep(config.projection_interval).await;
        }

        RoomSession::open(store, self.node_id(), config).await
    }

    pub async fn shutdown(&self) -> Result<()> {
        self.client.backend().shutdown().await.map_err(Into::into)
    }

    pub fn reset_identity(data_dir: &Path) -> Result<bool> {
        let paths = [
            data_dir.join(NODE_SECRET_FILE),
            data_dir.join("db").join("identity.json"),
        ];
        let mut removed = false;
        for path in paths {
            if !path.exists() {
                continue;
            }
            std::fs::remove_file(&path).map_err(|error| {
                GuardianAdapterError::IdentityMigration(format!(
                    "cannot remove {}: {error}",
                    path.display()
                ))
            })?;
            removed = true;
        }
        Ok(removed)
    }
}

pub fn validate_ticket(ticket: &str) -> Result<()> {
    if ticket.starts_with("dchat") {
        return Err(GuardianAdapterError::LegacyTicket);
    }
    ticket
        .parse::<DocTicket>()
        .map(|_| ())
        .map_err(|error| GuardianAdapterError::InvalidTicket(error.to_string()))
}

fn migrate_legacy_identity(legacy_path: &Path, data_dir: &Path) -> Result<bool> {
    let guardian_path = data_dir.join(NODE_SECRET_FILE);
    if guardian_path.exists() || !legacy_path.exists() {
        return Ok(false);
    }

    let bytes = std::fs::read(legacy_path).map_err(|error| {
        GuardianAdapterError::IdentityMigration(format!(
            "cannot read {}: {error}",
            legacy_path.display()
        ))
    })?;
    if bytes.len() != 32 {
        return Err(GuardianAdapterError::IdentityMigration(format!(
            "{} must contain exactly 32 raw bytes, found {}",
            legacy_path.display(),
            bytes.len()
        )));
    }
    std::fs::write(&guardian_path, bytes).map_err(|error| {
        GuardianAdapterError::IdentityMigration(format!(
            "cannot write {}: {error}",
            guardian_path.display()
        ))
    })?;
    Ok(true)
}

fn short_hash(bytes: &[u8]) -> String {
    hex::encode(&blake3::hash(bytes).as_bytes()[..16])
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_legacy_raw_identity_once() {
        let temp = tempfile::tempdir().unwrap();
        let legacy = temp.path().join("identity.key");
        let guardian = temp.path().join("guardian");
        std::fs::create_dir_all(&guardian).unwrap();
        std::fs::write(&legacy, [9_u8; 32]).unwrap();

        assert!(migrate_legacy_identity(&legacy, &guardian).unwrap());
        assert_eq!(
            std::fs::read(guardian.join(NODE_SECRET_FILE)).unwrap(),
            [9; 32]
        );
        let expected = iroh::SecretKey::from_bytes(&[9; 32]).public();
        let migrated = iroh::SecretKey::from_bytes(
            &std::fs::read(guardian.join(NODE_SECRET_FILE))
                .unwrap()
                .try_into()
                .unwrap(),
        )
        .public();
        assert_eq!(migrated, expected);
        assert!(!migrate_legacy_identity(&legacy, &guardian).unwrap());
    }

    #[test]
    fn force_reset_removes_guardian_identity_files() {
        let temp = tempfile::tempdir().unwrap();
        let db_dir = temp.path().join("db");
        std::fs::create_dir_all(&db_dir).unwrap();
        std::fs::write(temp.path().join(NODE_SECRET_FILE), [1; 32]).unwrap();
        std::fs::write(db_dir.join("identity.json"), b"{}").unwrap();

        assert!(GuardianNode::reset_identity(temp.path()).unwrap());
        assert!(!temp.path().join(NODE_SECRET_FILE).exists());
        assert!(!db_dir.join("identity.json").exists());
        assert!(!GuardianNode::reset_identity(temp.path()).unwrap());
    }

    #[test]
    fn rejects_legacy_ticket_explicitly() {
        assert!(matches!(
            validate_ticket("dchat123"),
            Err(GuardianAdapterError::LegacyTicket)
        ));
    }

    #[test]
    fn rejects_non_guardian_ticket() {
        assert!(matches!(
            validate_ticket("not-a-ticket"),
            Err(GuardianAdapterError::InvalidTicket(_))
        ));
    }
}
