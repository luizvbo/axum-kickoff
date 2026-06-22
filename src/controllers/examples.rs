//! Example controllers demonstrating HTMX + Askama patterns
//!
//! This module contains practical examples of:
//! - Full-page Askama templates
//! - Partial template responses
//! - HTMX form handling
//! - Validation error handling
//! - Redirect vs HTML partial conventions
//! - JSON API endpoints

use askama::Template;
use axum::extract::{Extension, Form, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Json, Redirect, Response};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::app::AppState;
use crate::middleware::{get_or_create_csrf_token, SessionExtension};
use crate::router::HtmlTemplate;

// ============================================================================
// Contact Form Example - HTMX Form with Validation
// ============================================================================

/// Contact form request data
#[derive(Debug, Deserialize)]
pub struct ContactForm {
    pub name: String,
    pub email: String,
    pub message: String,
}

/// Contact form page - Full-page Askama template
#[derive(Template)]
#[template(path = "examples/contact.html")]
struct ContactPageTemplate {
    csrf_token: String,
}

/// Contact form success partial - Returned on successful submission
#[derive(Template)]
#[template(path = "examples/contact_success.html")]
struct ContactSuccessTemplate {
    name: String,
    email: String,
}

/// Contact form errors partial - Returned on validation failure
#[derive(Template)]
#[template(path = "examples/contact_errors.html")]
struct ContactErrorsTemplate {
    errors: Vec<String>,
}

/// Render the contact form page (full-page template)
pub async fn contact_page(Extension(session): Extension<SessionExtension>) -> impl IntoResponse {
    let csrf_token = get_or_create_csrf_token(&session);
    let template = ContactPageTemplate { csrf_token };
    HtmlTemplate(template)
}

/// Handle contact form submission (HTMX endpoint)
///
/// This demonstrates the HTMX pattern:
/// - On success: Return HTML partial with success message
/// - On error: Return HTML partial with validation errors
/// - HTMX swaps the response into the target element
pub async fn contact_submit(
    Extension(_session): Extension<SessionExtension>,
    State(_state): State<AppState>,
    Form(form): Form<ContactForm>,
) -> Response {
    // Validate the form
    let mut errors = Vec::new();

    if form.name.trim().is_empty() {
        errors.push("Name is required".to_string());
    } else if form.name.len() < 2 {
        errors.push("Name must be at least 2 characters".to_string());
    }

    if form.email.trim().is_empty() {
        errors.push("Email is required".to_string());
    } else if !form.email.contains('@') {
        errors.push("Email must be valid".to_string());
    }

    if form.message.trim().is_empty() {
        errors.push("Message is required".to_string());
    } else if form.message.len() < 10 {
        errors.push("Message must be at least 10 characters".to_string());
    }

    // Return errors partial if validation fails
    if !errors.is_empty() {
        let template = ContactErrorsTemplate { errors };
        match template.render() {
            Ok(html) => return Html(html).into_response(),
            Err(err) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to render template: {}", err),
                )
                    .into_response()
            }
        }
    }

    // In a real app, you would save to database here
    // let mut db = state.database.db_clone();
    // ... save contact message ...

    // Return success partial
    let template = ContactSuccessTemplate {
        name: form.name,
        email: form.email,
    };
    match template.render() {
        Ok(html) => Html(html).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to render template: {}", err),
        )
            .into_response(),
    }
}

// ============================================================================
// Counter Example - Simple HTMX State Updates
// ============================================================================

/// Counter page template
#[derive(Template)]
#[template(path = "examples/counter.html")]
struct CounterPageTemplate {
    csrf_token: String,
}

/// Counter partial template - Returned on increment/decrement
#[derive(Template)]
#[template(path = "examples/counter_partial.html")]
struct CounterPartialTemplate {
    count: i32,
}

/// Render the counter page
pub async fn counter_page(Extension(session): Extension<SessionExtension>) -> impl IntoResponse {
    let csrf_token = get_or_create_csrf_token(&session);
    let template = CounterPageTemplate { csrf_token };
    HtmlTemplate(template)
}

/// Increment counter (HTMX endpoint)
pub async fn counter_increment(Form(params): Form<HashMap<String, String>>) -> impl IntoResponse {
    let count: i32 = params.get("count")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    let template = CounterPartialTemplate { count: count + 1 };
    HtmlTemplate(template)
}

/// Decrement counter (HTMX endpoint)
pub async fn counter_decrement(Form(params): Form<HashMap<String, String>>) -> impl IntoResponse {
    let count: i32 = params.get("count")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    let template = CounterPartialTemplate { count: count - 1 };
    HtmlTemplate(template)
}

// ============================================================================
// JSON API Example - Traditional REST endpoint
// ============================================================================

/// JSON API response example
#[derive(Debug, Serialize)]
pub struct ExampleJsonResponse {
    pub message: String,
    pub timestamp: String,
    pub data: HashMap<String, String>,
}

/// JSON endpoint example
///
/// This demonstrates a traditional JSON API endpoint
/// that can be used by JavaScript fetch() or other HTTP clients.
pub async fn example_json() -> impl IntoResponse {
    let mut data = HashMap::new();
    data.insert("framework".to_string(), "Axum".to_string());
    data.insert("templating".to_string(), "Askama".to_string());
    data.insert("frontend".to_string(), "HTMX".to_string());

    let response = ExampleJsonResponse {
        message: "This is a JSON API response example".to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        data,
    };

    Json(response)
}

// ============================================================================
// Redirect vs HTML Partial Convention
// ============================================================================

/// Demonstrates when to redirect vs return HTML partial
///
/// Convention:
/// - HTMX requests: Return HTML partial (no redirect)
/// - Traditional form submissions: Redirect on success
/// - Navigation actions: Redirect to new page
///
/// You can detect HTMX requests via the "HX-Request" header
pub async fn example_redirect_convention() -> impl IntoResponse {
    // This would check for HTMX header in a real implementation
    // let is_htmx = headers.get("HX-Request").is_some();

    // For HTMX: return HTML partial
    // For traditional: return Redirect

    // Example redirect:
    Redirect::to("/examples/contact")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contact_form_validation_empty() {
        let form = ContactForm {
            name: String::new(),
            email: String::new(),
            message: String::new(),
        };

        let mut errors = Vec::new();
        if form.name.trim().is_empty() {
            errors.push("Name is required".to_string());
        }
        if form.email.trim().is_empty() {
            errors.push("Email is required".to_string());
        }
        if form.message.trim().is_empty() {
            errors.push("Message is required".to_string());
        }

        assert_eq!(errors.len(), 3);
    }

    #[test]
    fn test_contact_form_validation_valid() {
        let form = ContactForm {
            name: "John Doe".to_string(),
            email: "john@example.com".to_string(),
            message: "This is a valid message".to_string(),
        };

        let mut errors = Vec::new();
        if form.name.trim().is_empty() {
            errors.push("Name is required".to_string());
        }
        if !form.email.contains('@') {
            errors.push("Email must be valid".to_string());
        }
        if form.message.trim().is_empty() {
            errors.push("Message is required".to_string());
        }

        assert_eq!(errors.len(), 0);
    }
}
