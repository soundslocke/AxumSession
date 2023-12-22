#[cfg(feature = "advanced")]
use crate::SessionError;
use crate::{DatabasePool, SessionData, SessionID, SessionStore};
use async_trait::async_trait;
use axum_core::extract::FromRequestParts;

#[cfg(feature = "key-store")]
use fastbloom_rs::Membership;
use http::{self, request::Parts, StatusCode};
use serde::Serialize;
use std::{
    convert::From,
    fmt::Debug,
    marker::{Send, Sync},
};
use uuid::Uuid;

/// A Session Store.
///
/// Provides a Storage Handler to SessionStore and contains the SessionID(UUID) of the current session.
///
/// This is Auto generated by the Session Layer Upon Service Execution.
#[derive(Debug, Clone)]
pub struct Session<T>
where
    T: DatabasePool + Clone + Debug + Sync + Send + 'static,
{
    /// The SessionStore that holds all the Sessions.
    pub(crate) store: SessionStore<T>,
    /// The Sessions current ID for lookng up its store.
    pub(crate) id: SessionID,
}

/// Adds FromRequestParts<B> for Session
///
/// Returns the Session from Axums request extensions state.
#[async_trait]
impl<T, S> FromRequestParts<S> for Session<T>
where
    T: DatabasePool + Clone + Debug + Sync + Send + 'static,
    S: Send + Sync,
{
    type Rejection = (http::StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts.extensions.get::<Session<T>>().cloned().ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Can't extract Axum `Session`. Is `SessionLayer` enabled?",
        ))
    }
}

impl<S> Session<S>
where
    S: DatabasePool + Clone + Debug + Sync + Send + 'static,
{
    #[allow(clippy::needless_pass_by_ref_mut)]
    pub(crate) async fn new(store: SessionStore<S>, value: Option<Uuid>) -> (Self, bool) {
        let (id, is_new) = match value {
            Some(v) => (SessionID(v), false),
            None => (Self::generate_uuid(&store).await, true),
        };

        #[cfg(feature = "key-store")]
        if store.config.use_bloom_filters {
            let contained = {
                let filter = store.filter.read().await;
                filter.contains(id.inner().as_bytes())
            };

            if !contained {
                let mut filter = store.filter.write().await;
                filter.add(id.inner().as_bytes());
            }
        }

        (Self { id, store }, is_new)
    }

    #[cfg(feature = "key-store")]
    pub(crate) async fn generate_uuid(store: &SessionStore<S>) -> SessionID {
        loop {
            let token = Uuid::new_v4();

            if (!store.config.use_bloom_filters || store.auto_handles_expiry())
                && !store.inner.contains_key(&token.to_string())
            {
                //This fixes an already used but in database issue.
                if let Some(client) = &store.client {
                    // Unwrap should be safe to use as we would want it to crash if there was a major database error.
                    // This would mean the database no longer is online or the table missing etc.
                    if !client
                        .exists(&token.to_string(), &store.config.table_name)
                        .await
                        .unwrap()
                    {
                        return SessionID(token);
                    }
                } else {
                    return SessionID(token);
                }
            } else {
                let filter = store.filter.read().await;

                if !filter.contains(token.to_string().as_bytes()) {
                    return SessionID(token);
                }
            }
        }
    }

    #[cfg(not(feature = "key-store"))]
    pub(crate) async fn generate_uuid(store: &SessionStore<S>) -> SessionID {
        loop {
            let token = Uuid::new_v4();

            if !store.inner.contains_key(&token.to_string()) {
                //This fixes an already used but in database issue.
                if let Some(client) = &store.client {
                    // Unwrap should be safe to use as we would want it to crash if there was a major database error.
                    // This would mean the database no longer is online or the table missing etc.
                    if !client
                        .exists(&token.to_string(), &store.config.table_name)
                        .await
                        .unwrap()
                    {
                        return SessionID(token);
                    }
                } else {
                    return SessionID(token);
                }
            }
        }
    }
    /// Sets the Session to create the SessionData based on the current Session ID.
    /// You can only use this if SessionMode::Manual is set or it will Panic.
    /// This will also set the store to true similair to session.set_store(true);
    ///
    /// # Examples
    /// ```rust ignore
    /// session.create_data();
    /// ```
    ///
    #[inline]
    pub fn create_data(&self) {
        if !self.store.config.session_mode.is_manual() {
            panic!(
                "Session must be set to SessionMode::Manual in order to use create_data, 
                as the Session data is created already."
            );
        }
        let sess = SessionData::new(self.id.0, true, &self.store.config);
        self.store.inner.insert(self.id.inner(), sess);
    }

    /// Checks if the SessionData was created or not.
    ///
    /// # Examples
    /// ```rust ignore
    /// if session.data_exists() {
    ///     println!("data Exists");
    /// }
    /// ```
    ///
    #[inline]
    pub fn data_exists(&self) -> bool {
        self.store.inner.contains_key(&self.id.inner())
    }

    /// Sets the Session to renew its Session ID.
    /// This Deletes Session data from the database
    /// associated with the old UUID. This helps to enhance
    /// Security when logging into Secure area's across a website.
    /// The current sessions data will be pushed to the database
    /// with the new UUID.
    ///
    /// # Examples
    /// ```rust ignore
    /// session.renew();
    /// ```
    ///
    #[inline]
    pub fn renew(&self) {
        self.store.renew(self.id.inner());
    }

    /// Sets the Session to force update the database.
    /// This will increase the Timer on the sessions store
    /// making the session live longer in the persistant database.
    ///
    /// # Examples
    /// ```rust ignore
    /// session.renew();
    /// ```
    ///
    #[inline]
    pub fn update(&self) {
        self.store.update(self.id.inner());
    }

    /// Sets the Current Session to be Destroyed.
    /// This will Deleted the Session and Cookies upon Response Phase.
    ///
    /// # Examples
    /// ```rust ignore
    /// session.destroy();
    /// ```
    ///
    #[inline]
    pub fn destroy(&self) {
        self.store.destroy(self.id.inner());
    }

    /// Sets the Current Session to a long term expiration. Useful for Remember Me setups.
    /// This will also update the database on Response Phase.
    ///
    /// # Examples
    /// ```rust ignore
    /// session.set_longterm(true);
    /// ```
    ///
    #[inline]
    pub fn set_longterm(&self, longterm: bool) {
        self.store.set_longterm(self.id.inner(), longterm);
    }

    /// Allows the Current Session to store.
    /// This will also update the database on Response Phase.
    ///
    /// This is only used when `SessionMode` is Manual or OptIn.
    /// This will allow the Session to be stored if true.
    /// This will delete and not allow a session to be stored if false.
    ///
    /// # Examples
    /// ```rust ignore
    /// session.set_store(true);
    /// ```
    ///
    #[inline]
    pub fn set_store(&self, can_store: bool) {
        self.store.set_store(self.id.inner(), can_store);
    }

    /// Gets data from the Session's HashMap
    ///
    /// Provides an Option<T> that returns the requested data from the Sessions store.
    /// Returns None if Key does not exist or if serdes_json failed to deserialize.
    ///
    /// # Examples
    /// ```rust ignore
    /// let id = session.get("user-id").unwrap_or(0);
    /// ```
    ///
    ///Used to get data stored within SessionDatas hashmap from a key value.
    ///
    #[inline]
    pub fn get<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.store.get(self.id.inner(), key)
    }

    /// Removes a Key from the Current Session's HashMap returning it.
    ///
    /// Provides an Option<T> that returns the requested data from the Sessions store.
    /// Returns None if Key does not exist or if serdes_json failed to deserialize.
    ///
    /// # Examples
    /// ```rust ignore
    /// let id = session.get_remove("user-id").unwrap_or(0);
    /// ```
    ///
    /// Used to get data stored within SessionDatas hashmap from a key value.
    ///
    #[inline]
    pub fn get_remove<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.store.get_remove(self.id.inner(), key)
    }

    /// Sets data to the Current Session's HashMap.
    /// This will also update the database on Response Phase.
    ///
    /// # Examples
    /// ```rust ignore
    /// session.set("user-id", 1);
    /// ```
    ///
    #[inline]
    pub fn set(&self, key: &str, value: impl Serialize) {
        self.store.set(self.id.inner(), key, value);
    }

    /// Removes a Key from the Current Session's HashMap.
    /// Does not process the String into a Type, Just removes it.
    /// This will also update the database on Response Phase.
    ///
    /// # Examples
    /// ```rust ignore
    /// let _ = session.remove("user-id");
    /// ```
    ///
    #[inline]
    pub fn remove(&self, key: &str) {
        self.store.remove(self.id.inner(), key);
    }

    /// Clears all data from the Current Session's HashMap instantly.
    /// This will also update the database on Response Phase.
    ///
    /// # Examples
    /// ```rust ignore
    /// session.clear();
    /// ```
    ///
    #[inline]
    pub fn clear(&self) {
        self.store.clear_session_data(self.id.inner());
    }

    /// Returns a i64 count of how many Sessions exist.
    ///
    /// If the Session is persistant it will return all sessions within the database.
    /// If the Session is not persistant it will return a count within SessionStore.
    ///
    /// # Examples
    /// ```rust ignore
    /// let count = session.count().await;
    /// ```
    ///
    #[inline]
    pub async fn count(&self) -> i64 {
        self.store.count_sessions().await
    }

    /// Returns the SessionID for this Session.
    ///
    /// The SessionID contains the Uuid generated at the beginning of this Session.
    ///
    /// # Examples
    /// ```rust ignore
    /// let session_id = session.get_session_id();
    /// ```
    ///
    #[inline]
    pub fn get_session_id(&self) -> SessionID {
        self.id
    }

    /// Returns the store for this Session.
    ///
    /// The store contains everything that all sessions need.
    ///
    /// # Examples
    /// ```rust ignore
    /// let store = session.get_store();
    /// ```
    ///
    #[inline]
    pub fn get_store(&self) -> &SessionStore<S> {
        &self.store
    }

    /// Returns a mutable store for this Session.
    ///
    /// The store contains everything that all sessions need.
    ///
    /// # Examples
    /// ```rust ignore
    /// let store = session.get_store_mut();
    /// ```
    ///
    #[inline]
    pub fn get_mut_store(&mut self) -> &mut SessionStore<S> {
        &mut self.store
    }

    /// Removes a Request from the request counter
    /// used to deturmine if parallel requests exist.
    /// prevents data deletion until requests == 0.
    ///
    /// # Examples
    /// ```rust ignore
    /// session.remove_request();
    /// ```
    ///
    #[inline]
    pub(crate) fn remove_request(&self) {
        self.store.remove_session_request(self.id.inner());
    }

    /// Removes a Request from the request counter
    /// used to deturmine if parallel requests exist.
    /// prevents data deletion until requests == 0.
    ///
    /// # Examples
    /// ```rust ignore
    /// session.set_request();
    /// ```
    ///
    #[inline]
    pub(crate) fn set_request(&self) {
        self.store.set_session_request(self.id.inner());
    }

    /// checks if a session has more than one request.
    ///
    /// # Examples
    /// ```rust ignore
    /// session.is_parallel();
    /// ```
    ///
    #[inline]
    pub(crate) fn is_parallel(&self) -> bool {
        self.store.is_session_parallel(self.id.inner())
    }

    /// checks if a session exists and if it is outdated.
    ///
    /// # Examples
    /// ```rust ignore
    /// session.verify();
    /// ```
    ///
    #[cfg(feature = "advanced")]
    #[inline]
    pub fn verify(&self) -> Result<(), SessionError> {
        self.store.verify(self.id.inner())
    }

    /// Updates the sessions stored database expire time.
    /// Use this before forcing a update to the database store.
    /// will update the database expires based on
    /// if the session is longterm then configs max_lifespan.
    /// if not then configs lifespan.
    ///
    /// THIS WILL NOT UPDATE THE DATABASE SIDE.
    ///
    /// # Examples
    /// ```rust ignore
    /// session.update_database_expires();
    /// ```
    ///
    #[cfg(feature = "advanced")]
    #[inline]
    pub fn update_database_expires(&self) -> Result<(), SessionError> {
        self.store.update_database_expires(self.id.inner())
    }

    /// Updates the Sessions In memory auto remove timer.
    /// Will prevent it from being removed for the configs set memory_lifespan.
    ///
    /// # Examples
    /// ```rust ignore
    /// session.update_memory_expires();
    /// ```
    ///
    #[cfg(feature = "advanced")]
    #[inline]
    pub fn update_memory_expires(&self) -> Result<(), SessionError> {
        self.store.update_memory_expires(self.id.inner())
    }

    /// forces a update to the databases stored data for the session.
    /// Make sure to update the databases expire time before running this or
    /// the data could be unloaded by a request checking for outdated sessions.
    ///
    /// # Examples
    /// ```rust ignore
    /// session.force_database_update().await;
    /// ```
    ///
    #[cfg(feature = "advanced")]
    #[inline]
    pub async fn force_database_update(&self) -> Result<(), SessionError> {
        self.store.force_database_update(self.id.inner()).await
    }

    /// Removes the session from the memory store if it is not parallel.
    /// If it is parallel then each parallel session will need to call this once.
    /// when all parallel sessions are dead this gets unloaded.
    ///
    /// THIS DOES NOT CLEAR THE KEY STORE.
    ///
    /// # Examples
    /// ```rust ignore
    /// session.memory_remove_session();
    /// ```
    ///
    #[cfg(feature = "advanced")]
    #[inline]
    pub fn memory_remove_session(&self) -> Result<(), SessionError> {
        self.store.memory_remove_session(self.id.inner())
    }

    /// Removes the session from the Database store.
    ///
    /// THIS DOES NOT REMOVE THE KEY STORE.
    ///
    /// # Examples
    /// ```rust ignore
    /// session.database_remove_session().await;
    /// ```
    ///
    #[cfg(feature = "advanced")]
    #[inline]
    pub async fn database_remove_session(&self) -> Result<(), SessionError> {
        self.store.database_remove_session(self.id.inner()).await
    }
}

#[derive(Debug, Clone)]
pub struct ReadOnlySession<T>
where
    T: DatabasePool + Clone + Debug + Sync + Send + 'static,
{
    pub(crate) store: SessionStore<T>,
    pub(crate) id: SessionID,
}

impl<T> From<Session<T>> for ReadOnlySession<T>
where
    T: DatabasePool + Clone + Debug + Sync + Send + 'static,
{
    fn from(session: Session<T>) -> Self {
        ReadOnlySession {
            store: session.store,
            id: session.id,
        }
    }
}

/// Adds FromRequestParts<B> for Session
///
/// Returns the Session from Axums request extensions state.
#[async_trait]
impl<T, S> FromRequestParts<S> for ReadOnlySession<T>
where
    T: DatabasePool + Clone + Debug + Sync + Send + 'static,
    S: Send + Sync,
{
    type Rejection = (http::StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let session = parts.extensions.get::<Session<T>>().cloned().ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Can't extract Axum `Session`. Is `SessionLayer` enabled?",
        ))?;

        Ok(session.into())
    }
}

impl<S> ReadOnlySession<S>
where
    S: DatabasePool + Clone + Debug + Sync + Send + 'static,
{
    /// Gets data from the Session's HashMap
    ///
    /// Provides an Option<T> that returns the requested data from the Sessions store.
    /// Returns None if Key does not exist or if serdes_json failed to deserialize.
    ///
    /// # Examples
    /// ```rust ignore
    /// let id = session.get("user-id").unwrap_or(0);
    /// ```
    ///
    ///Used to get data stored within SessionDatas hashmap from a key value.
    ///
    #[inline]
    pub fn get<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.store.get(self.id.inner(), key)
    }

    /// Returns a i64 count of how many Sessions exist.
    ///
    /// If the Session is persistant it will return all sessions within the database.
    /// If the Session is not persistant it will return a count within SessionStore.
    ///
    /// # Examples
    /// ```rust ignore
    /// let count = session.count().await;
    /// ```
    ///
    #[inline]
    pub async fn count(&self) -> i64 {
        self.store.count_sessions().await
    }
}
