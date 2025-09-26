use chrono::{DateTime, Duration, Utc};
use cookie::Key;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashMap, fmt::Debug};

use crate::{SessionError, SessionOps, StoredAs, encrypt};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionData {
    #[serde(skip)]
    pub id: String,
    pub data: HashMap<String, String>,
    #[serde(skip)]
    pub expires_at: DateTime<Utc>,
    #[serde(skip)]
    pub autoremove_at: DateTime<Utc>,
    #[serde(skip)]
    pub destroy: bool,
    #[serde(skip)]
    pub renew: bool,
    pub longterm: bool,
    #[serde(skip)]
    pub store: bool,
    #[serde(skip)]
    pub update: bool,
    #[serde(skip)]
    pub requests: usize,
    #[serde(skip)]
    pub encryption_key: Option<Key>,
}

impl Default for SessionData {
    fn default() -> Self {
        Self {
            id: "".to_string(),
            data: HashMap::new(),
            expires_at: Utc::now(),
            destroy: true,
            renew: false,
            autoremove_at: Utc::now(),
            longterm: false,
            store: false,
            update: false,
            requests: 0,
            encryption_key: None,
        }
    }
}

impl SessionOps for SessionData {
    #[inline]
    fn id(&self) -> String {
        self.id.clone()
    }

    #[inline]
    fn set_id(&mut self, new_id: &str) {
        self.id = new_id.to_string();
        self.update = true;
    }

    #[inline]
    fn service_clear(&mut self, memory_lifespan: Duration, clear_check: bool) {
        if clear_check && self.autoremove_at < Utc::now() {
            self.update = true;

            if self.is_expired() {
                self.data.clear();
            }
        }

        self.autoremove_at = Utc::now() + memory_lifespan;
    }

    #[inline]
    fn renew(&mut self) {
        self.renew = true;
        self.update = true;
    }

    #[inline]
    fn prevent_renew(&mut self) {
        self.renew = false;
        self.update = true;
    }

    #[inline]
    fn will_renew(&self) -> bool {
        self.renew
    }

    #[inline]
    fn update(&mut self) {
        self.update = true;
    }

    #[inline]
    fn prevent_update(&mut self) {
        self.update = false;
    }

    #[inline]
    fn will_update(&self) -> bool {
        self.update
    }

    #[inline]
    fn destroy(&mut self) {
        self.destroy = true;
    }

    #[inline]
    fn will_destroy(&self) -> bool {
        self.destroy
    }

    #[inline]
    fn set_longterm(&mut self, longterm: bool) {
        self.longterm = longterm;
        self.update = true;
    }

    #[inline]
    fn is_longterm(&self) -> bool {
        self.longterm
    }

    fn expires_at(&self) -> DateTime<Utc> {
        self.expires_at.clone()
    }

    #[inline]
    fn is_expired(&self) -> bool {
        self.expires_at < Utc::now()
    }

    #[inline]
    fn set_expiration(&mut self, expires_at: DateTime<Utc>) {
        self.expires_at = expires_at;
        self.update = true;
    }

    #[inline]
    fn autoremove_at(&self) -> DateTime<Utc> {
        self.autoremove_at.clone()
    }

    #[inline]
    fn set_autoremove(&mut self, autoremove_at: DateTime<Utc>) {
        self.autoremove_at = autoremove_at;
        self.update = true;
    }

    #[inline]
    fn will_autoremove(&self, time: DateTime<Utc>) -> bool {
        self.autoremove_at < time
    }

    #[inline]
    fn set_storable(&mut self, can_store: bool) {
        self.store = can_store;
        self.update = true;
    }

    #[inline]
    fn is_storable(&self) -> bool {
        self.store
    }

    #[inline]
    fn get(&self, key: &str) -> Option<Value> {
        let string = self.data.get(key)?;
        serde_json::from_str(string).ok()
    }

    #[inline]
    fn get_remove(&mut self, key: &str) -> Option<Value> {
        let string = self.data.remove(key)?;
        self.update = true;
        serde_json::from_str(&string).ok()
    }

    #[inline]
    fn set(&mut self, key: &str, value: Value) {
        let value = serde_json::to_string(&value).unwrap_or_else(|_| "".to_string());
        let _ = self.data.insert(key.to_string(), value);
        self.update = true;
    }

    #[inline]
    fn remove(&mut self, key: &str) {
        let _ = self.data.remove(key);
        self.update = true;
    }

    #[inline]
    fn clear(&mut self) {
        self.data.clear();
        self.update = true;
    }

    #[inline]
    fn add_request(&mut self) {
        self.requests = self.requests.saturating_add(1);
    }

    #[inline]
    fn remove_request(&mut self) {
        self.requests = self.requests.saturating_sub(1);
    }

    #[inline]
    fn reset_requests(&mut self) {
        self.requests = 1;
    }

    #[inline]
    fn is_parallel(&self) -> bool {
        self.requests >= 1
    }

    #[inline]
    fn to_string(&self) -> String {
        self.encrypt()
    }

    #[inline]
    fn to_value(&self) -> Value {
        let value = serde_json::to_value(self);

        match value {
            Ok(value) => value,
            Err(error) => {
                tracing::error!("Could not make serde json value: {}", error.to_string());
                Value::default()
            }
        }
    }

    #[inline]
    fn set_encryption_key(&mut self, encryption_key: &Option<Key>) {
        self.encryption_key = encryption_key.clone();
    }

    #[inline]
    fn encrypt(&self) -> String {
        let unencrypted = self.to_value().to_string();

        match &self.encryption_key {
            None => unencrypted,
            Some(key) => match encrypt::encrypt(&self.id, &unencrypted, &key) {
                Ok(encrypted) => encrypted,
                Err(err) => {
                    tracing::error!(err = %err, "Failed to encrypt session data.");
                    String::new()
                }
            },
        }
    }

    #[inline]
    fn decrypt(&self, encrypted: &str) -> String {
        match &self.encryption_key {
            None => encrypted.to_string(),
            Some(key) => match encrypt::decrypt(&self.id, &encrypted, &key) {
                Ok(value) => value,
                Err(err) => {
                    tracing::error!(err = %err, "Failed to decrypt session data.");
                    String::new()
                }
            },
        }
    }

    #[inline]
    fn from_storage(&self, stored: &StoredAs) -> Result<Box<dyn SessionOps>, SessionError> {
        let deserialized = match stored {
            StoredAs::String(json) => serde_json::from_str::<Self>(&self.decrypt(&json)),
            StoredAs::JsonValue(value) => serde_json::from_value(value.clone()),
        };

        match deserialized {
            Ok(session) => Ok(Box::new(session)),
            Err(err) => Err(err.into()),
        }
    }

    #[inline]
    fn merge(&mut self, data: HashMap<String, String>) {
        self.data.extend(data);
    }

    #[inline]
    fn clone_box(&self) -> Box<dyn SessionOps> {
        Box::new(self.clone())
    }
}

/// Internal timers
///
/// Used to keep track of the last ran expiration check for both database and memory session data.
///
#[derive(Debug)]
pub(crate) struct SessionTimers {
    pub(crate) last_expiry_sweep: DateTime<Utc>,
    pub(crate) last_database_expiry_sweep: DateTime<Utc>,
}
