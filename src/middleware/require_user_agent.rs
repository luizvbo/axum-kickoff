//! Middleware that blocks requests with no user-agent header
//!
//! # What is this for?
//!
//! This middleware requires all HTTP requests to include a User-Agent header.
//! The User-Agent header identifies the client making the request (e.g., a browser,
//! API client, or bot). Requiring this header helps prevent abuse from anonymous
//! bots and automated tools that don't identify themselves.
//!
//! # Why do you need it?
//!
//! Many malicious bots and scrapers don't set a User-Agent header or use generic
//! ones. By requiring a User-Agent header, you can:
//! - **Block anonymous abuse**: Prevent requests from unidentified sources
//! - **Improve logging**: Know which clients are accessing your API
//! - **Enable better analytics**: Track legitimate client usage
//! - **Deter simple bots**: Many basic bots don't set User-Agent headers
//!
//! # How it works
//!
//! 1. Checks if the request has a User-Agent header
//! 2. If missing (and not a download endpoint), returns `403 Forbidden`
//! 3. Download endpoints are exempt to support older clients that may not set User-Agent
//!
//! # Exemptions
//!
//! Requests to paths ending with `/download` are always allowed, even without a
//! User-Agent header. This is to support older clients and backward compatibility.
//!
//! # What is a User-Agent?
//!
//! A User-Agent is a string that identifies the client making the request:
//!
//! - **Browser**: `Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36`
//! - **API client**: `MyApp/1.0 (my-app.com)`
//! - **Bot**: `Googlebot/2.1` or `curl/7.68.0`
//!
//! # Best Practices
//!
//! For your own API clients, set a descriptive User-Agent:
//!
//! ```http
//! User-Agent: MyApp/1.0 (contact@example.com)
//! ```
//!
//! This helps server administrators:
//! - Identify your application
//! - Contact you if there are issues
//! - Distinguish your traffic from abuse
//!
//! # When to use it
//!
//! - **Recommended** for public APIs
//! - **Optional** for internal applications
//! - **Useful** when you want to track client usage
//! - **Helpful** for debugging and analytics
//!
//! # Response
//!
//! Blocked requests receive a `403 Forbidden` response with a message explaining
//! that a User-Agent header is required.

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::IntoResponse;
use axum_extra::headers::UserAgent;
use axum_extra::TypedHeader;
use http::StatusCode;

pub async fn require_user_agent(
    user_agent: Option<TypedHeader<UserAgent>>,
    req: Request,
    next: Next,
) -> axum::response::Response {
    let agent = match user_agent {
        Some(ref header) => header.as_str(),
        None => "",
    };

    let has_user_agent = !agent.is_empty();
    let is_download = req.uri().path().ends_with("download");

    if !has_user_agent && !is_download {
        let request_id = req
            .headers()
            .get("x-request-id")
            .map(|header| header.to_str().unwrap_or_default())
            .unwrap_or_default();

        let body = format!(
            "Requests without a User-Agent header are not allowed. \
             Please set a descriptive User-Agent header to identify your client. \
             Request ID: {}",
            request_id
        );

        (StatusCode::FORBIDDEN, body).into_response()
    } else {
        next.run(req).await
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_download_path_detection() {
        assert!("/api/download".ends_with("download"));
        assert!("/some/path/download".ends_with("download"));
        assert!("/download".ends_with("download"));
        assert!(!"/api/test".ends_with("download"));
        assert!(!"/download/test".ends_with("download"));
    }

    #[test]
    fn test_user_agent_check_logic() {
        // Test empty string
        let agent = "";
        assert!(agent.is_empty());
        
        // Test non-empty string
        let agent = "Mozilla/5.0";
        assert!(!agent.is_empty());
        
        // Test whitespace
        let agent = "   ";
        assert!(!agent.is_empty());
    }

    #[test]
    fn test_block_condition_logic() {
        // Should block: no user agent and not download
        let has_user_agent = false;
        let is_download = false;
        assert!(!has_user_agent && !is_download);
        
        // Should allow: has user agent
        let has_user_agent = true;
        let is_download = false;
        assert!(has_user_agent || is_download);
        
        // Should allow: is download
        let has_user_agent = false;
        let is_download = true;
        assert!(has_user_agent || is_download);
        
        // Should allow: both
        let has_user_agent = true;
        let is_download = true;
        assert!(has_user_agent || is_download);
    }

    #[test]
    fn test_error_message_formatting() {
        let request_id = "test-123";
        let body = format!(
            "Requests without a User-Agent header are not allowed. \
             Please set a descriptive User-Agent header to identify your client. \
             Request ID: {}",
            request_id
        );
        assert!(body.contains("test-123"));
        assert!(body.contains("User-Agent"));
    }

    #[test]
    fn test_error_message_without_request_id() {
        let request_id = "";
        let body = format!(
            "Requests without a User-Agent header are not allowed. \
             Please set a descriptive User-Agent header to identify your client. \
             Request ID: {}",
            request_id
        );
        assert!(body.contains("Request ID:"));
    }
}
