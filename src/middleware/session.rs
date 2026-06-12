//! Session management utilities
//!
//! This module provides session management using signed cookies.
//! The session key is available in the app state for signing and verifying cookies.
//!
//! Note: The middleware integration requires further work to match Axum's
//! middleware signature requirements. For now, the utilities (SessionExtension,
//! decode, encode) are available for manual use in handlers.

use base64::{Engine, engine::general_purpose};
use cookie::Cookie;
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
