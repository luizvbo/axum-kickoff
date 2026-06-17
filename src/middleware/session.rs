//! Session management utilities
//!
//! This module provides session management using signed cookies.
//! The session key is available in the app state for signing and verifying cookies.

use axum::extract::{Request, State};
use axum::http::header::SET_COOKIE;
use axum::middleware::Next;
use axum::response::Response;
use base64::{engine::general_purpose, Engine};
use cookie::{Cookie, CookieJar, Key};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

pub static COOKIE_NAME: &str = "axum_kickoff_session";

#[derive(Clone)]
pub struct SessionExtension(Arc<RwLock<Session>>);

impl SessionExtension {
    pub fn new(session: Session) -> Self {
        Self(Arc::new(RwLock::new(session)))
    }

    pub fn get(&self, key: &str) -> Option<String> {
        let session = self.0.read();
        session.data.get(key).cloned()
    }

    pub fn insert(&self, key: String, value: String) -> Option<String> {
        let mut session = self.0.write();
        session.dirty = true;
        session.data.insert(key, value)
    }

    pub fn remove(&self, key: &str) -> Option<String> {
        let mut session = self.0.write();
        session.dirty = true;
        session.data.remove(key)
    }

    pub fn is_dirty(&self) -> bool {
        let session = self.0.read();
        session.dirty
    }

    pub fn encode(&self) -> String {
        let session = self.0.read();
        encode(&session.data)
    }
}

/// Request extension holding the session data
pub struct Session {
    data: HashMap<String, String>,
    dirty: bool,
}

impl Session {
    pub fn new(data: HashMap<String, String>) -> Self {
        Self { data, dirty: false }
    }
}

pub fn decode(cookie: Cookie<'_>) -> HashMap<String, String> {
    let mut ret = HashMap::new();
    let bytes = general_purpose::STANDARD
        .decode(cookie.value().as_bytes())
        .unwrap_or_default();
    let mut parts = bytes.split(|&a| a == 0xff);
    while let (Some(key), Some(value)) = (parts.next(), parts.next()) {
        if key.is_empty() {
            break;
        }
        if let (Ok(key), Ok(value)) = (std::str::from_utf8(key), std::str::from_utf8(value)) {
            ret.insert(key.to_string(), value.to_string());
        }
    }
    ret
}

pub fn encode(h: &HashMap<String, String>) -> String {
    let mut ret = Vec::new();
    for (i, (k, v)) in h.iter().enumerate() {
        if i != 0 {
            ret.push(0xff)
        }
        ret.extend(k.bytes());
        ret.push(0xff);
        ret.extend(v.bytes());
    }
    while ret.len() * 8 % 6 != 0 {
        ret.push(0xff);
    }
    general_purpose::STANDARD.encode(&ret[..])
}

/// Session middleware
///
/// Extracts the session cookie from the request, decodes it, and provides
/// a SessionExtension to handlers. After the handler runs, if the session
/// was modified, it encodes it back to a signed cookie.
pub async fn middleware(State(session_key): State<Key>, req: Request, next: Next) -> Response {
    // Extract session cookie from request
    let session_data = req
        .headers()
        .get("cookie")
        .and_then(|cookie_header| {
            cookie_header
                .to_str()
                .ok()
                .and_then(|cookies| {
                    cookies
                        .split(';')
                        .find_map(|cookie| Cookie::parse(cookie.trim()).ok())
                })
                .filter(|cookie| cookie.name() == COOKIE_NAME)
        })
        .map(|cookie| {
            // Decode the cookie value
            let value = cookie.value();
            let decoded_cookie = Cookie::new(COOKIE_NAME.to_string(), value.to_string());
            decode(decoded_cookie)
        })
        .unwrap_or_default();

    // Create session extension
    let session = Session::new(session_data);
    let session_extension = SessionExtension::new(session);
    let session_arc = session_extension.0.clone();

    // Add session extension to request
    let mut req = req;
    req.extensions_mut().insert(session_extension);

    // Run the handler
    let mut response = next.run(req).await;

    // Check if session was modified
    let session = session_arc.read();
    if session.dirty {
        // Encode the session data
        let encoded = encode(&session.data);

        // Create signed cookie with encoded value
        let cookie = Cookie::build((COOKIE_NAME, encoded))
            .path("/")
            .http_only(true)
            .same_site(cookie::SameSite::Lax)
            .build();

        // Sign the cookie
        let mut jar = CookieJar::new();
        jar.signed_mut(&session_key).add(cookie);

        // Add Set-Cookie header to response
        if let Some(signed_cookie) = jar.get(COOKIE_NAME) {
            response
                .headers_mut()
                .insert(SET_COOKIE, signed_cookie.to_string().parse().unwrap());
        }
    }

    response
}
