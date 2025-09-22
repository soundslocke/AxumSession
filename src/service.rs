use crate::{DatabasePool, Session, SessionError, SessionStore, headers::*};
use axum::{BoxError, response::Response};
use bytes::Bytes;
use chrono::Utc;
#[cfg(feature = "key-store")]
use fastbloom_rs::Deletable;
use futures::future::BoxFuture;
use http::Request;
use http_body::Body as HttpBody;
use std::{
    convert::Infallible,
    fmt::{self, Debug, Formatter},
    task::{Context, Poll},
};
use tower_service::Service;

#[derive(Clone)]
pub struct SessionService<S, T>
where
    T: DatabasePool + Clone + Debug + Sync + Send + 'static,
{
    pub(crate) session_store: SessionStore<T>,
    pub(crate) inner: S,
}

pub(crate) fn trace_error<ResBody>(
    err: SessionError,
    msg: &str,
) -> Result<Response<ResBody>, Infallible>
where
    ResBody: HttpBody<Data = Bytes> + Default + Send + 'static,
    ResBody::Error: Into<BoxError>,
{
    tracing::error!(err = %err, msg);
    let mut res = Response::default();
    *res.status_mut() = http::StatusCode::INTERNAL_SERVER_ERROR;
    Ok(res)
}

impl<S, T, ReqBody, ResBody> Service<Request<ReqBody>> for SessionService<S, T>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>, Error = Infallible>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
    ReqBody: Send + 'static,
    Infallible: From<<S as Service<Request<ReqBody>>>::Error>,
    ResBody: HttpBody<Data = Bytes> + Default + Send + 'static,
    ResBody::Error: Into<BoxError>,
    T: DatabasePool + Clone + Debug + Sync + Send + 'static,
{
    type Response = Response<ResBody>;
    type Error = Infallible;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<ReqBody>) -> Self::Future {
        let store = self.session_store.clone();
        let not_ready_inner = self.inner.clone();
        let mut ready_inner = std::mem::replace(&mut self.inner, not_ready_inner);

        Box::pin(async move {
            let ip_user_agent = get_ips_hash(&req, &store);

            #[cfg(not(feature = "rest_mode"))]
            let cookies = get_cookies(req.headers());

            #[cfg(not(feature = "rest_mode"))]
            let (session_id, storable) = get_headers_and_key(&store, cookies, &ip_user_agent).await;

            #[cfg(feature = "rest_mode")]
            let headers = get_headers(&store, req.headers());

            #[cfg(feature = "rest_mode")]
            let (session_id, storable) = get_headers_and_key(&store, headers, &ip_user_agent).await;

            let (mut session, is_new) = match Session::new(store, session_id).await {
                Ok(v) => v,
                Err(err) => {
                    return trace_error(err, "failed to generate Session ID");
                }
            };

            // Check if the session id exists if not lets check if it exists in the database or generate a new session.
            // If manual mode is enabled then do not check for a Session unless the ID is not new.
            let check_database: bool = if is_new && !session.store.config.session_mode.is_manual() {
                let mut session_data = session.store.config.session_ops.clone_box();
                session_data.set_id(&session.id);
                session_data.set_storable(storable);

                session.store.inner.insert(session.id.clone(), session_data);

                false
            } else if !is_new || !session.store.config.session_mode.is_manual() {
                !session.store.service_session_data(&session)
            } else {
                false
            };

            if check_database {
                let mut fresh_session = session
                    .store
                    .load_session(session.id.clone())
                    .await
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| {
                        tracing::info!(
                            "Session {} did not exist in database so it was recreated.",
                            session.id.clone()
                        );

                        let mut session_data = session.store.config.session_ops.clone_box();
                        session_data.set_id(&session.id);
                        session_data.set_storable(storable);

                        session_data
                    });

                fresh_session
                    .set_autoremove(Utc::now() + session.store.config.memory.memory_lifespan);
                fresh_session.set_storable(storable);
                fresh_session.reset_requests();

                session
                    .store
                    .inner
                    .insert(session.id.clone(), fresh_session);
            }

            let (last_sweep, last_database_sweep) = {
                let timers = session.store.timers.read().await;
                (timers.last_expiry_sweep, timers.last_database_expiry_sweep)
            };

            // This branch runs less often, and we already have write access,
            // let's check if any sessions expired. We don't want to hog memory
            // forever by abandoned sessions (e.g. when a client lost their cookie)
            // throttle by memory lifespan - e.g. sweep every hour
            let current_time = Utc::now();

            if last_sweep <= current_time && !session.store.config.memory.memory_lifespan.is_zero()
            {
                tracing::info!(
                    "Session id {}: Session Memory Cleaning Started",
                    session.id.clone()
                );
                // Only unload these from filter if the Client is None as this means no database.
                // Otherwise only unload from the filter if removed from the Database.
                #[cfg(feature = "key-store")]
                if session.store.is_persistent()
                    && session.store.auto_handles_expiry()
                    && session.store.config.memory.use_bloom_filters
                {
                    let mut filter = session.store.filter.write().await;
                    session
                        .store
                        .inner
                        .iter()
                        .filter(|r| r.will_autoremove(current_time))
                        .for_each(|r| filter.remove(r.key().as_bytes()));
                }

                session
                    .store
                    .inner
                    .retain(|_k, v| !v.will_autoremove(current_time));

                session.store.timers.write().await.last_expiry_sweep =
                    Utc::now() + session.store.config.memory.purge_update;
                tracing::info!(
                    "Session id {}: Session Memory Cleaning Finished",
                    session.id
                );
            }

            // Throttle by database lifespan - e.g. sweep every 6 hours
            if last_database_sweep <= current_time && session.store.is_persistent() {
                tracing::info!(
                    "Session id {}: Session Database Cleaning Started",
                    session.id
                );
                //Remove any old keys that expired and Remove them from our loaded filter.
                #[cfg(feature = "key-store")]
                let expired = match session.store.cleanup().await {
                    Ok(v) => v,
                    Err(err) => {
                        return trace_error(
                            err,
                            "failed to remove expired session's from database",
                        );
                    }
                };

                #[cfg(not(feature = "key-store"))]
                if let Err(err) = session.store.cleanup().await {
                    return trace_error(err, "failed to remove expired session's from database");
                }

                #[cfg(feature = "key-store")]
                if !session.store.auto_handles_expiry() {
                    let mut filter = session.store.filter.write().await;
                    expired.iter().for_each(|id| filter.remove(id.as_bytes()));
                }

                session
                    .store
                    .timers
                    .write()
                    .await
                    .last_database_expiry_sweep =
                    Utc::now() + session.store.config.database.purge_database_update;
                tracing::info!(
                    "Session id {}: Session Database Cleaning Finished",
                    session.id
                );
            }

            // Sets a clone of the Store in the Extensions for Direct usage and sets the Session for Direct usage
            //req.extensions_mut().insert(store.clone());
            req.extensions_mut().insert(session.clone());

            let mut response = ready_inner.call(req).await?;

            let (renew, storable, destroy, loaded) = match session.store.inner.get(&session.id) {
                Some(session_data) => (
                    session_data.will_renew(),
                    session_data.is_storable(),
                    session_data.will_destroy(),
                    true,
                ),
                _ => (false, false, false, false),
            };

            tracing::trace!(
                renew = renew,
                storable = storable,
                destroy = destroy,
                loaded = loaded,
                "Session id: {}",
                session.id
            );

            if !destroy && (!session.store.config.session_mode.is_manual() || loaded) && renew {
                // Lets change the Session ID and destory the old Session from the database.
                let session_id = match Session::generate_id(&session.store).await {
                    Ok(v) => v,
                    Err(err) => {
                        return trace_error(err, "failed to Generate Session ID");
                    }
                };

                // Lets remove it from the database first.
                if session.store.is_persistent() {
                    if let Err(err) = session
                        .store
                        .database_remove_session(session.id.clone())
                        .await
                    {
                        return trace_error(err, "failed to remove session from database");
                    };
                }

                //lets remove it from the filter. if the bottom fails just means it did not exist or was already unloaded.
                #[cfg(feature = "key-store")]
                if session.store.config.memory.use_bloom_filters {
                    let mut filter = session.store.filter.write().await;
                    filter.remove(session.id.as_bytes());
                }

                // Let's remove, update, and reinsert.
                if let Some((_, mut session_data)) = session.store.inner.remove(&session.id) {
                    session_data.set_id(&session_id);
                    session_data.prevent_renew();
                    session.id = session_id;
                    session.store.inner.insert(session.id.clone(), session_data);
                }
            }

            // Add the Session ID so it can link back to a Session if one exists.
            if (!session.store.config.session_mode.is_opt_in() || storable)
                && session.store.is_persistent()
                && !destroy
            {
                let updated_session = match session.store.inner.get_mut(&session.id) {
                    Some(mut sess) => {
                        // Check if Database needs to be updated or not. TODO: Make updatable based on a timer for in memory only.
                        if session.store.config.database.always_save
                            || sess.will_update()
                            || !sess.is_expired()
                        {
                            if sess.is_longterm() {
                                sess.set_expiration(Utc::now() + session.store.config.max_lifespan);
                            } else {
                                sess.set_expiration(Utc::now() + session.store.config.lifespan);
                            };

                            sess.prevent_update();

                            Some(sess)
                        } else {
                            None
                        }
                    }
                    _ => None,
                };

                if let Some(sess) = updated_session {
                    if let Err(err) = session.store.store_session(&sess).await {
                        return trace_error(err, "failed to save session to database");
                    } else {
                        tracing::info!("Session id {}: was saved to the database.", session.id);
                    }
                }
            }

            //lets tell the system we can unload this request now.
            //If there are still more left the bottom wont unload anything.
            session.remove_request();

            if ((session.store.config.session_mode.is_opt_in() && !storable) || destroy)
                && !session.is_parallel()
            {
                #[cfg(feature = "key-store")]
                if session.store.config.memory.use_bloom_filters {
                    let mut filter = session.store.filter.write().await;
                    filter.remove(session.id.as_bytes());
                }

                let _ = session.store.inner.remove(&session.id);

                if session.store.is_persistent() {
                    if let Err(err) = session
                        .store
                        .database_remove_session(session.id.clone())
                        .await
                    {
                        return trace_error(err, "failed to remove session from database");
                    }
                }
            }

            // We will Deleted the data in memory as it should be stored in the database instead.
            // if user is using this without a database then it will only work as a per request data store.
            if session.store.config.memory.memory_lifespan.is_zero() && !session.is_parallel() {
                #[cfg(feature = "key-store")]
                if !session.store.is_persistent() && session.store.config.memory.use_bloom_filters {
                    let mut filter = session.store.filter.write().await;
                    filter.remove(session.id.as_bytes());
                }

                session.store.inner.remove(&session.id);
            }

            set_headers(
                &session,
                response.headers_mut(),
                &ip_user_agent,
                destroy,
                storable,
            );

            Ok(response)
        })
    }
}

impl<S, T> Debug for SessionService<S, T>
where
    S: Debug,
    T: DatabasePool + Clone + Debug + Sync + Send + 'static,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionService")
            .field("session_store", &self.session_store)
            .field("inner", &self.inner)
            .finish()
    }
}
