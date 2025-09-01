use async_trait::async_trait;
use axum_session::{DatabaseError, DatabasePool, Session, SessionOps, SessionStore, StoredAs};
use redis_pool::SingleRedisPool;

use crate::key;

///Redis's Session Helper type for the DatabasePool.
pub type SessionRedisSession = Session<SessionRedisPool>;
///Redis's Session Store Helper type for the DatabasePool.
pub type SessionRedisSessionStore = SessionStore<SessionRedisPool>;

///Redis's Pool type for the DatabasePool. Needs a redis Client.
#[derive(Clone)]
pub struct SessionRedisPool {
    pool: SingleRedisPool,
}

impl From<SingleRedisPool> for SessionRedisPool {
    fn from(pool: SingleRedisPool) -> Self {
        SessionRedisPool { pool }
    }
}

impl std::fmt::Debug for SessionRedisPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionRedisPool").finish()
    }
}

#[async_trait]
impl DatabasePool for SessionRedisPool {
    async fn initiate(&self, _table_name: &str) -> Result<(), DatabaseError> {
        // Redis does not actually use Tables so there is no way we can make one.
        Ok(())
    }

    async fn delete_by_expiry(&self, _table_name: &str) -> Result<Vec<String>, DatabaseError> {
        // Redis does this for use using the Expiry Options.
        Ok(Vec::new())
    }

    async fn count(&self, table_name: &str) -> Result<i64, DatabaseError> {
        let mut con = match self.pool.acquire().await {
            Ok(v) => v,
            Err(err) => return Err(DatabaseError::GenericAcquire(err.to_string())),
        };

        let count: i64 = if table_name.is_empty() {
            match redis::cmd("DBSIZE").query_async(&mut con).await {
                Ok(v) => v,
                Err(err) => return Err(DatabaseError::GenericSelectError(err.to_string())),
            }
        } else {
            // Assuming we have a table name, we need to count all the keys that match the table name.
            // We can't use DBSIZE because that would count all the keys in the database.
            let keys =
                match super::redis_tools::scan_keys(&mut con, &format!("{table_name}:*")).await {
                    Ok(v) => v,
                    Err(err) => return Err(DatabaseError::GenericSelectError(err.to_string())),
                };
            keys.len() as i64
        };

        Ok(count)
    }

    async fn store(
        &self,
        session: &Box<dyn SessionOps>,
        table_name: &str,
    ) -> Result<(), DatabaseError> {
        let mut con = self
            .pool
            .acquire()
            .await
            .map_err(|err| DatabaseError::GenericAcquire(err.to_string()))?;

        let key = key(&session.id(), table_name);

        redis::pipe()
            .atomic() //makes this a transaction.
            .set(&key, session.to_string())
            .ignore()
            .expire_at(&key, session.expires_at().timestamp())
            .ignore()
            .query_async::<()>(&mut con)
            .await
            .map_err(|err| DatabaseError::GenericSelectError(err.to_string()))?;

        Ok(())
    }

    async fn load(&self, id: &str, table_name: &str) -> Result<Option<StoredAs>, DatabaseError> {
        let mut con = self
            .pool
            .acquire()
            .await
            .map_err(|err| DatabaseError::GenericAcquire(err.to_string()))?;

        let key = key(id, table_name);

        let result: String = redis::cmd("GET")
            .arg(key)
            .query_async(&mut con)
            .await
            .map_err(|err| DatabaseError::GenericSelectError(err.to_string()))?;

        Ok(Some(result.into()))
    }

    async fn delete_one_by_id(&self, id: &str, table_name: &str) -> Result<(), DatabaseError> {
        let mut con = self
            .pool
            .acquire()
            .await
            .map_err(|err| DatabaseError::GenericAcquire(err.to_string()))?;

        let key = key(id, table_name);

        redis::cmd("DEL")
            .arg(key)
            .query_async::<()>(&mut con)
            .await
            .map_err(|err| DatabaseError::GenericDeleteError(err.to_string()))?;
        Ok(())
    }

    async fn exists(&self, id: &str, table_name: &str) -> Result<bool, DatabaseError> {
        let mut con = self
            .pool
            .acquire()
            .await
            .map_err(|err| DatabaseError::GenericAcquire(err.to_string()))?;

        let key = key(id, table_name);

        let exists: bool = redis::cmd("EXISTS")
            .arg(key)
            .query_async(&mut con)
            .await
            .map_err(|err| DatabaseError::GenericSelectError(err.to_string()))?;

        Ok(exists)
    }

    async fn delete_all(&self, table_name: &str) -> Result<(), DatabaseError> {
        let mut con = self
            .pool
            .acquire()
            .await
            .map_err(|err| DatabaseError::GenericAcquire(err.to_string()))?;

        if table_name.is_empty() {
            redis::cmd("FLUSHDB")
                .query_async::<()>(&mut con)
                .await
                .map_err(|err| DatabaseError::GenericDeleteError(err.to_string()))?;
        } else {
            // Assuming we have a table name, we need to delete all the keys that match the table name.
            // We can't use FLUSHDB because that would delete all the keys in the database.
            let keys = super::redis_tools::scan_keys(&mut con, &format!("{table_name}:*"))
                .await
                .map_err(|err| DatabaseError::GenericSelectError(err.to_string()))?;

            for key in keys {
                redis::cmd("DEL")
                    .arg(key)
                    .query_async::<()>(&mut con)
                    .await
                    .map_err(|err| DatabaseError::GenericDeleteError(err.to_string()))?;
            }
        }

        Ok(())
    }

    async fn get_ids(&self, table_name: &str) -> Result<Vec<String>, DatabaseError> {
        let mut con = self
            .pool
            .acquire()
            .await
            .map_err(|err| DatabaseError::GenericAcquire(err.to_string()))?;

        let table_name = if table_name.is_empty() {
            "*".to_string()
        } else {
            format!("{table_name}:0")
        };

        let result: Vec<String> =
            super::redis_tools::scan_keys(&mut con, &format!("{table_name}:*"))
                .await
                .map_err(|err| DatabaseError::GenericSelectError(err.to_string()))?;

        Ok(result)
    }

    fn auto_handles_expiry(&self) -> bool {
        true
    }
}
