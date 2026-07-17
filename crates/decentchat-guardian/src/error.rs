use thiserror::Error;

#[derive(Debug, Error)]
pub enum GuardianAdapterError {
    #[error("Guardian DB error: {0}")]
    Guardian(String),

    #[error("invalid Guardian room ticket: {0}")]
    InvalidTicket(String),

    #[error(
        "legacy dchat tickets are unsupported; create a new Guardian room and share its raw ticket"
    )]
    LegacyTicket,

    #[error("unsupported schema version {version} in {key}; this build supports version 1")]
    UnsupportedSchema { key: String, version: u64 },

    #[error("invalid record at {key}: {reason}")]
    InvalidRecord { key: String, reason: String },

    #[error("room metadata did not arrive within {0:?}")]
    RoomMetadataTimeout(std::time::Duration),

    #[error("identity migration failed: {0}")]
    IdentityMigration(String),

    #[error("room is closed")]
    Closed,
}

pub type Result<T> = std::result::Result<T, GuardianAdapterError>;

impl From<guardian_db::guardian::error::GuardianError> for GuardianAdapterError {
    fn from(value: guardian_db::guardian::error::GuardianError) -> Self {
        Self::Guardian(value.to_string())
    }
}
