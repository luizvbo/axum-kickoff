//! Helpers for reading environment variables.
//!
//! These helpers wrap `dotenvy::var` and attach the variable name as context
//! to any errors returned by `anyhow`.

use anyhow::Context;
use std::str::FromStr;

/// Read an optional environment variable.
///
/// Returns `Ok(None)` if the variable is not set and `Ok(Some(value))`
/// otherwise. Errors other than "not present" are propagated with the
/// variable name attached as context.
pub fn var(key: &str) -> anyhow::Result<Option<String>> {
    dotenvy::var(key)
        .map(Some)
        .or_else(|err| match err {
            dotenvy::Error::EnvVar(std::env::VarError::NotPresent) => Ok(None),
            _ if err.not_found() => Ok(None),
            _ => Err(err),
        })
        .with_context(|| key.to_string())
}

/// Read a required environment variable.
///
/// Returns an error if the variable is not set.
pub fn required_var(key: &str) -> anyhow::Result<String> {
    var(key)?.with_context(|| format!("{key} must be set"))
}

/// Read an optional environment variable and parse it into a typed value.
///
/// Returns `Ok(None)` if the variable is not set. Parse errors are
/// propagated with the variable name attached as context.
pub fn var_parsed<R: FromStr>(key: &str) -> anyhow::Result<Option<R>> {
    let value = match var(key)? {
        Some(value) => value,
        None => return Ok(None),
    };

    value
        .parse::<R>()
        .map(Some)
        .map_err(|_| anyhow::anyhow!("{key} has an invalid value"))
}
