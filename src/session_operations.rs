use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use cookie::Key;
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
    fn set_encryption_key(&mut self, encryption_key: &Option<Key>);
    fn encrypt(&self) -> String;
    fn decrypt(&self, encrypted: &str) -> String;
    fn from_storage(&self, stored: &StoredAs) -> Result<Box<dyn SessionOps>, SessionError>;
    fn merge(&mut self, data: HashMap<String, String>);
    fn clone_box(&self) -> Box<dyn SessionOps>;
}

impl Clone for Box<dyn SessionOps> {
    fn clone(&self) -> Box<dyn SessionOps> {
        self.clone_box()
    }
}
