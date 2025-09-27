use crate::{DatabaseError, DatabasePool, Session, SessionOps, SessionStore, StoredAs};
use async_trait::async_trait;

///Null's Session Helper type for a DatabaseLess Session.
pub type SessionNullSession = Session<SessionNullPool>;
///Null's Session Store Helper type for a DatabaseLess Session.
pub type SessionNullSessionStore = SessionStore<SessionNullPool>;

/// Null Pool type for a DatabaseLess Session.
/// Use this when you do not want to load any database.
#[derive(Debug, Clone)]
pub struct SessionNullPool;

#[async_trait]
impl DatabasePool for SessionNullPool {
    async fn initiate(&self, _table_name: &str) -> Result<(), DatabaseError> {
        Ok(())
    }

    async fn delete_by_expiry(&self, _table_name: &str) -> Result<Vec<String>, DatabaseError> {
        Ok(Vec::new())
    }

    async fn count(&self, _table_name: &str) -> Result<i64, DatabaseError> {
        return Ok(0);
    }

    async fn store(
        &self,
        _session: &Box<dyn SessionOps>,
        _table_name: &str,
    ) -> Result<(), DatabaseError> {
        Ok(())
    }

    async fn load(&self, _id: &str, _table_name: &str) -> Result<Option<StoredAs>, DatabaseError> {
        Ok(None)
    }

    async fn delete_one_by_id(&self, _id: &str, _table_name: &str) -> Result<(), DatabaseError> {
        Ok(())
    }

    async fn exists(&self, _id: &str, _table_name: &str) -> Result<bool, DatabaseError> {
        Ok(false)
    }

    async fn delete_all(&self, _table_name: &str) -> Result<(), DatabaseError> {
        Ok(())
    }

    async fn get_ids(&self, _table_name: &str) -> Result<Vec<String>, DatabaseError> {
        Ok(Vec::new())
    }

    fn auto_handles_expiry(&self) -> bool {
        false
    }
}
