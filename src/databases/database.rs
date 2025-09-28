use async_trait::async_trait;
use serde_json::Value;
use thiserror::Error;

use crate::SessionOps;

/// The Trait used to implement a database client or client pool.
///
/// This can be freely implemented but default implementations for
/// several databases are included.
#[async_trait]
pub trait DatabasePool {
    /// Creates the database table when starting the app.
    async fn initiate(&self, table_name: &str) -> Result<(), DatabaseError>;

    /// Count the number of stored sessions.
    async fn count(&self, table_name: &str) -> Result<i64, DatabaseError>;

    /// Store a session.
    async fn store(
        &self,
        session: &Box<dyn SessionOps>,
        table_name: &str,
    ) -> Result<(), DatabaseError>;

    /// Load a session.
    async fn load(&self, id: &str, table_name: &str) -> Result<Option<StoredAs>, DatabaseError>;

    /// Delete a single session.
    async fn delete(&self, id: &str, table_name: &str) -> Result<(), DatabaseError>;

    /// Does this session exist?
    async fn exists(&self, id: &str, table_name: &str) -> Result<bool, DatabaseError>;

    /// Delete all sessions that have expired.
    async fn delete_expired(&self, table_name: &str) -> Result<Vec<String>, DatabaseError>;

    /// Delete all sessions.
    async fn delete_all(&self, table_name: &str) -> Result<(), DatabaseError>;

    /// Get all session IDs.
    async fn get_ids(&self, table_name: &str) -> Result<Vec<String>, DatabaseError>;

    /// Does this database handle session expiration automatically?
    fn auto_handles_expiry(&self) -> bool;
}

#[derive(Debug)]
pub enum StoredAs {
    String(String),
    JsonValue(Value),
}

impl Default for StoredAs {
    fn default() -> Self {
        StoredAs::String("".to_string())
    }
}

impl From<String> for StoredAs {
    fn from(s: String) -> Self {
        StoredAs::String(s)
    }
}

impl From<&str> for StoredAs {
    fn from(s: &str) -> Self {
        StoredAs::String(s.to_string())
    }
}

impl From<Value> for StoredAs {
    fn from(v: Value) -> Self {
        StoredAs::JsonValue(v)
    }
}

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("Database insert error {0}")]
    GenericAcquire(String),
    #[error("Database insert error {0}")]
    GenericInsertError(String),
    #[error("Database select error {0}")]
    GenericSelectError(String),
    #[error("Database create error {0}")]
    GenericCreateError(String),
    #[error("Database delete error {0}")]
    GenericDeleteError(String),
    #[error("{0}")]
    GenericNotSupportedError(String),
}
