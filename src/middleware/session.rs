//! Session management utilities
//!
//! This module provides session management using signed cookies.
//! The session key is available in the app state for signing and verifying cookies.

use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum_extra::extract::cookie::SignedCookieJar;
use base64::{engine::general_purpose, Engine};
use cookie::{Cookie, Key};
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
/// Extracts the session cookie from the request, verifies its signature,
/// decodes it, and provides a SessionExtension to handlers. After the handler
/// runs, if the session was modified, it encodes it back to a signed cookie.
pub async fn middleware(State(session_key): State<Key>, req: Request, next: Next) -> Response {
    // Create SignedCookieJar from request headers for automatic signature verification
    let jar = SignedCookieJar::from_headers(req.headers(), session_key.clone());

    // Decode session cookie - signature is automatically verified by SignedCookieJar
    // If signature is invalid, get() returns None
    let session_data = jar.get(COOKIE_NAME).map(decode).unwrap_or_default();

    // Create session extension
    let session = Session::new(session_data);
    let session_extension = SessionExtension::new(session);
    let session_arc = session_extension.0.clone();

    // Add session extension to request
    let mut req = req;
    req.extensions_mut().insert(session_extension);

    // Run the handler
    let response = next.run(req).await;

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

        // Return updated jar with response - SignedCookieJar implements IntoResponse
        (jar.add(cookie), response).into_response()
    } else {
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_empty() {
        let data = HashMap::new();
        let encoded = encode(&data);
        let decoded = decode(Cookie::new(COOKIE_NAME.to_string(), encoded));
        assert!(decoded.is_empty());
    }

    #[test]
    fn test_encode_decode_single_pair() {
        let mut data = HashMap::new();
        data.insert("key1".to_string(), "value1".to_string());
        let encoded = encode(&data);
        let decoded = decode(Cookie::new(COOKIE_NAME.to_string(), encoded));
        assert_eq!(decoded.get("key1"), Some(&"value1".to_string()));
    }

    #[test]
    fn test_encode_decode_multiple_pairs() {
        let mut data = HashMap::new();
        data.insert("key1".to_string(), "value1".to_string());
        data.insert("key2".to_string(), "value2".to_string());
        data.insert("key3".to_string(), "value3".to_string());
        let encoded = encode(&data);
        let decoded = decode(Cookie::new(COOKIE_NAME.to_string(), encoded));
        assert_eq!(decoded.get("key1"), Some(&"value1".to_string()));
        assert_eq!(decoded.get("key2"), Some(&"value2".to_string()));
        assert_eq!(decoded.get("key3"), Some(&"value3".to_string()));
    }

    #[test]
    fn test_encode_decode_special_characters() {
        let mut data = HashMap::new();
        data.insert("key".to_string(), "value with spaces".to_string());
        data.insert("key2".to_string(), "special:chars=123".to_string());
        let encoded = encode(&data);
        let decoded = decode(Cookie::new(COOKIE_NAME.to_string(), encoded));
        assert_eq!(decoded.get("key"), Some(&"value with spaces".to_string()));
        assert_eq!(decoded.get("key2"), Some(&"special:chars=123".to_string()));
    }

    #[test]
    fn test_encode_decode_unicode() {
        let mut data = HashMap::new();
        data.insert("key".to_string(), "value with emoji 🎉".to_string());
        data.insert("key2".to_string(), "中文".to_string());
        let encoded = encode(&data);
        let decoded = decode(Cookie::new(COOKIE_NAME.to_string(), encoded));
        assert_eq!(decoded.get("key"), Some(&"value with emoji 🎉".to_string()));
        assert_eq!(decoded.get("key2"), Some(&"中文".to_string()));
    }

    #[test]
    fn test_session_extension_get() {
        let mut data = HashMap::new();
        data.insert("key1".to_string(), "value1".to_string());
        let session = Session::new(data);
        let extension = SessionExtension::new(session);
        assert_eq!(extension.get("key1"), Some("value1".to_string()));
        assert_eq!(extension.get("nonexistent"), None);
    }

    #[test]
    fn test_session_extension_insert() {
        let data = HashMap::new();
        let session = Session::new(data);
        let extension = SessionExtension::new(session);
        assert_eq!(
            extension.insert("key1".to_string(), "value1".to_string()),
            None
        );
        assert_eq!(extension.get("key1"), Some("value1".to_string()));
        assert!(extension.is_dirty());
    }

    #[test]
    fn test_session_extension_insert_overwrite() {
        let mut data = HashMap::new();
        data.insert("key1".to_string(), "old_value".to_string());
        let session = Session::new(data);
        let extension = SessionExtension::new(session);
        assert_eq!(
            extension.insert("key1".to_string(), "new_value".to_string()),
            Some("old_value".to_string())
        );
        assert_eq!(extension.get("key1"), Some("new_value".to_string()));
    }

    #[test]
    fn test_session_extension_remove() {
        let mut data = HashMap::new();
        data.insert("key1".to_string(), "value1".to_string());
        let session = Session::new(data);
        let extension = SessionExtension::new(session);
        assert_eq!(extension.remove("key1"), Some("value1".to_string()));
        assert_eq!(extension.get("key1"), None);
        assert!(extension.is_dirty());
    }

    #[test]
    fn test_session_extension_remove_nonexistent() {
        let data = HashMap::new();
        let session = Session::new(data);
        let extension = SessionExtension::new(session);
        assert_eq!(extension.remove("nonexistent"), None);
        assert!(extension.is_dirty());
    }

    #[test]
    fn test_session_extension_is_dirty_initial() {
        let data = HashMap::new();
        let session = Session::new(data);
        let extension = SessionExtension::new(session);
        assert!(!extension.is_dirty());
    }

    #[test]
    fn test_session_extension_is_dirty_after_insert() {
        let data = HashMap::new();
        let session = Session::new(data);
        let extension = SessionExtension::new(session);
        extension.insert("key".to_string(), "value".to_string());
        assert!(extension.is_dirty());
    }

    #[test]
    fn test_session_extension_encode() {
        let mut data = HashMap::new();
        data.insert("key1".to_string(), "value1".to_string());
        let session = Session::new(data);
        let extension = SessionExtension::new(session);
        let encoded = extension.encode();
        assert!(!encoded.is_empty());
    }

    #[test]
    fn test_session_new() {
        let mut data = HashMap::new();
        data.insert("key".to_string(), "value".to_string());
        let session = Session::new(data);
        assert_eq!(session.data.get("key"), Some(&"value".to_string()));
        assert!(!session.dirty);
    }

    #[test]
    fn test_encode_roundtrip() {
        let mut data = HashMap::new();
        data.insert("user_id".to_string(), "123".to_string());
        data.insert("username".to_string(), "testuser".to_string());
        let encoded = encode(&data);
        let decoded = decode(Cookie::new(COOKIE_NAME.to_string(), encoded));
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded.get("user_id"), Some(&"123".to_string()));
        assert_eq!(decoded.get("username"), Some(&"testuser".to_string()));
    }

    #[test]
    fn test_decode_invalid_base64() {
        let cookie = Cookie::new(COOKIE_NAME.to_string(), "invalid_base64!!!");
        let decoded = decode(cookie);
        assert!(decoded.is_empty());
    }

    #[test]
    fn test_decode_empty_value() {
        let cookie = Cookie::new(COOKIE_NAME.to_string(), "");
        let decoded = decode(cookie);
        assert!(decoded.is_empty());
    }
}
