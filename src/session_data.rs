use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashMap, fmt::Debug};

use crate::{SessionError, StoredAs};

#[async_trait]
pub trait SessionOps: Debug + Send + Sync {
    fn id(&self) -> String;
    fn set_id(&mut self, new_id: &str);

    /// Validates and checks if the session will be destroyed.
    /// If so, the data is cleared. Autoremove is updated regardless.
    ///
    fn service_clear(&mut self, memory_lifespan: Duration, clear_check: bool);

    /// Flag the session to renew/regenerate its ID. This deletes data
    /// from the database keyed with the old ID. This helps to enhance
    /// security when logging in or similar. The session's data is then
    /// stored with the new ID.
    ///
    /// # Examples
    /// ```rust ignore
    /// session.renew();
    /// ```
    ///
    fn renew(&mut self);
    fn prevent_renew(&mut self);
    fn will_renew(&self) -> bool;

    /// Force the session to be updated in the database. This updates
    /// the timestamp and ensures the session lives longer in the
    /// persistent database.
    ///
    /// # Examples
    /// ```rust ignore
    /// session.update();
    /// ```
    ///
    fn update(&mut self);
    fn prevent_update(&mut self);
    fn will_update(&self) -> bool;

    /// Flag the session to be destroyed.
    /// This will delete the session and cookies on response phase.
    ///
    /// # Examples
    /// ```rust ignore
    /// session.destroy();
    /// ```
    ///
    fn destroy(&mut self);
    fn will_destroy(&self) -> bool;

    /// Sets the session to a long term expiration. Useful for "remember me" setups.
    /// This will also update the database on response phase.
    ///
    /// # Examples
    /// ```rust ignore
    /// session.set_longterm(true);
    /// ```
    ///
    fn set_longterm(&mut self, longterm: bool);
    fn is_longterm(&self) -> bool;
    fn is_expired(&self) -> bool;
    fn expires_at(&self) -> DateTime<Utc>;
    fn set_expiration(&mut self, expires_at: DateTime<Utc>);
    fn autoremove_at(&self) -> DateTime<Utc>;

    /// Sets the date time for when this session should be automated
    /// removed from the storage back end.
    ///
    /// # Examples
    /// ```rust ignore
    /// session.set_autoremove(Utc::now() + TimeDelta::hours(8));
    /// ```
    ///
    fn set_autoremove(&mut self, autoremove_at: DateTime<Utc>);
    fn will_autoremove(&self, check_time: DateTime<Utc>) -> bool;

    /// Sets the current session to be storable.
    /// This will also update the database on response phase.
    ///
    /// This is only used when `SessionMode` is Manual or Storable.
    /// When true, this allows the session to be stored.
    ///
    /// This will delete and not allow a session to be stored if false.
    ///
    /// # Examples
    /// ```rust ignore
    /// session.set_storable(true);
    /// ```
    ///
    fn set_storable(&mut self, can_store: bool);
    fn is_storable(&self) -> bool;

    /// Gets data from the session's HashMap
    ///
    /// Provides an Option<T> that returns the requested data from the session store.
    /// Returns None if Key does not exist or if serde_json failed to deserialize.
    ///
    /// # Examples
    /// ```rust ignore
    /// let id = session.get("user-id").unwrap_or(0);
    /// ```
    ///
    fn get(&self, key: &str) -> Option<Value>;

    /// Removes a key and its value from the session and returns that
    /// value. This will also update the database on response phase.
    ///
    /// # Examples
    /// ```rust ignore
    /// let id = session.get_remove("user-id").unwrap_or(0);
    /// ```
    ///
    fn get_remove(&mut self, key: &str) -> Option<Value>;

    /// Assigns a value to a key in the session.
    /// This will also update the database on response phase.
    ///
    /// # Examples
    /// ```rust ignore
    /// session.set("user-id", 1);
    /// ```
    ///
    fn set(&mut self, key: &str, value: Value);

    /// Removes a key and its value from the session.
    /// This will also update the database on response phase.
    ///
    /// # Examples
    /// ```rust ignore
    /// let _ = session.remove("user-id");
    /// ```
    ///
    fn remove(&mut self, key: &str);

    /// Clears all data from the session.
    /// This will also update the database on response phase.
    ///
    /// # Examples
    /// ```rust ignore
    /// session.clear();
    /// ```
    ///
    fn clear(&mut self);

    /// Adds a request to the request counter.
    /// Used to determine if parallel requests exist and
    /// prevents data deletion until requests == 0.
    ///
    /// # Examples
    /// ```rust ignore
    /// session.add_request();
    /// ```
    ///
    fn add_request(&mut self);

    /// Removes a request from the request counter.
    /// Used to determine if parallel requests exist and
    /// prevents data deletion until requests == 0.
    ///
    /// # Examples
    /// ```rust ignore
    /// session.remove_request();
    /// ```
    ///
    fn remove_request(&mut self);

    /// Resets the request counter to 1.
    /// Used to determine if parallel requests exist and
    /// prevents data deletion until requests == 0.
    ///
    /// # Examples
    /// ```rust ignore
    /// session.reset_requests();
    /// ```
    ///
    fn reset_requests(&mut self);

    /// Checks if a session has any requests, which means it is
    /// being used in parallel.
    ///
    /// # Examples
    /// ```rust ignore
    /// session.is_parallel();
    /// ```
    ///
    fn is_parallel(&self) -> bool;
    fn to_string(&self) -> String;
    fn to_value(&self) -> Value;
    fn from_storage(&self, s: &StoredAs) -> Result<Box<dyn SessionOps>, SessionError>;
    fn merge_data(&mut self, data: HashMap<String, String>);
    fn clone_box(&self) -> Box<dyn SessionOps>;
}

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
        self.to_value().to_string()
    }

    #[inline]
    fn to_value(&self) -> Value {
        let value = serde_json::to_value(self);

        match value {
            Ok(v) => v,
            Err(error) => {
                tracing::error!("Could not make serde json value: {}", error.to_string());
                Value::default()
            }
        }
    }

    #[inline]
    fn from_storage(&self, stored: &StoredAs) -> Result<Box<dyn SessionOps>, SessionError> {
        let deserialized = match stored {
            StoredAs::String(s) => serde_json::from_str::<Self>(s),
            StoredAs::JsonValue(j) => serde_json::from_value::<Self>(j.clone()),
        };

        match deserialized {
            Ok(s) => Ok(Box::new(s)),
            Err(e) => Err(e.into()),
        }
    }

    #[inline]
    fn merge_data(&mut self, data: HashMap<String, String>) {
        self.data.extend(data);
    }

    #[inline]
    fn clone_box(&self) -> Box<dyn SessionOps> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn SessionOps> {
    fn clone(&self) -> Box<dyn SessionOps> {
        self.clone_box()
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
