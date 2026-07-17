use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use guardian_db::guardian::error::GuardianError;
use guardian_db::traits::{KeyValueStore, Store};

use crate::error::{GuardianAdapterError, Result};

/// Minimal store surface used by the room projector.
///
/// Keeping this boundary small makes record and event behavior testable without
/// opening sockets or a persistent Guardian node.
#[async_trait]
pub trait RoomStore: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;
    async fn put(&self, key: &str, value: Vec<u8>) -> Result<()>;
    async fn all(&self) -> Result<HashMap<String, Vec<u8>>>;
    async fn share_ticket(&self) -> Result<String>;
    async fn close(&self) -> Result<()>;
}

pub(crate) struct GuardianRoomStore {
    inner: Arc<dyn KeyValueStore<Error = GuardianError>>,
}

impl GuardianRoomStore {
    pub(crate) fn new(inner: Arc<dyn KeyValueStore<Error = GuardianError>>) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl RoomStore for GuardianRoomStore {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        self.inner.get(key).await.map_err(Into::into)
    }

    async fn put(&self, key: &str, value: Vec<u8>) -> Result<()> {
        self.inner
            .put(key, value)
            .await
            .map(|_| ())
            .map_err(Into::into)
    }

    async fn all(&self) -> Result<HashMap<String, Vec<u8>>> {
        Ok(self.inner.all())
    }

    async fn share_ticket(&self) -> Result<String> {
        self.inner.share_ticket().await.map_err(Into::into)
    }

    async fn close(&self) -> Result<()> {
        Store::close(self.inner.as_ref())
            .await
            .map_err(|error| GuardianAdapterError::Guardian(error.to_string()))
    }
}
