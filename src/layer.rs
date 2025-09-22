use std::{
    fmt::Debug,
    marker::{Send, Sync},
};

use crate::{DatabasePool, SessionData, SessionOps, SessionService, SessionStore};
use tower_layer::Layer;

/// Sessions Layer used with Axum to activate the Service.
///
/// # Examples
/// ```rust ignore
/// use axum_session::{SessionNullPool, SessionConfig, SessionStore, SessionLayer};
/// use uuid::Uuid;
///
/// let config = SessionConfig::default();
/// let session_store = SessionStore::<SessionNullPool>::new(None, config).await.unwrap();
/// let layer = SessionLayer::new(session_store);
/// ```
///
#[derive(Clone)]
pub struct SessionLayer<D, O = SessionData>
where
    D: DatabasePool + Clone + Debug + Sync + Send + 'static,
    O: SessionOps + Clone + Debug + Send + Sync + 'static,
{
    session_store: SessionStore<D, O>,
}

impl<D, O> SessionLayer<D, O>
where
    D: DatabasePool + Clone + Debug + Sync + Send + 'static,
    O: SessionOps + Clone + Debug + Send + Sync + 'static,
{
    /// Constructs a SessionLayer used with Axum to activate the Service.
    ///
    /// # Examples
    /// ```rust ignore
    /// use axum_session::{SessionNullPool, SessionConfig, SessionStore, SessionLayer};
    /// use uuid::Uuid;
    ///
    /// let config = SessionConfig::default();
    /// let session_store = SessionStore::<SessionNullPool>::new(None, config).await.unwrap();
    /// let layer = SessionLayer::new(session_store);
    /// ```
    ///
    #[inline]
    pub fn new(session_store: SessionStore<D, O>) -> Self {
        SessionLayer { session_store }
    }
}

impl<S, D, O> Layer<S> for SessionLayer<D, O>
where
    D: DatabasePool + Clone + Debug + Sync + Send + 'static,
    O: SessionOps + Clone + Debug + Send + Sync + 'static,
{
    type Service = SessionService<S, D, O>;

    fn layer(&self, inner: S) -> Self::Service {
        SessionService {
            session_store: self.session_store.clone(),
            inner,
        }
    }
}
