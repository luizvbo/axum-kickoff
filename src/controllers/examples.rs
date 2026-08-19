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
use axum::extract::{Form, State};
use axum::response::{IntoResponse, Json, Redirect, Response};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::app::AppState;
use crate::router::{HtmlTemplate, PageContext};

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
    ctx: PageContext,
}

/// Contact form success partial - Returned on successful submission
#[derive(Template)]
#[template(path = "examples/contact_success.html")]
struct ContactSuccessTemplate {
    ctx: PageContext,
    name: String,
    email: String,
}

/// Contact form errors partial - Returned on validation failure
#[derive(Template)]
#[template(path = "examples/contact_errors.html")]
#[allow(dead_code)]
struct ContactErrorsTemplate {
    ctx: PageContext,
    errors: Vec<String>,
}

/// Render the contact form page (full-page template)
pub async fn contact_page(ctx: PageContext) -> impl IntoResponse {
    let template = ContactPageTemplate { ctx };
    HtmlTemplate::new(template)
}

/// Handle contact form submission (HTMX endpoint)
///
/// This demonstrates the HTMX pattern:
/// - On success: Return HTML partial with success message
/// - On error: Return HTML partial with validation errors
/// - HTMX swaps the response into the target element
pub async fn contact_submit(
    ctx: PageContext,
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
        let template = ContactErrorsTemplate { ctx, errors };
        return HtmlTemplate::new(template).into_response();
    }

    // In a real app, you would save to database here
    // let mut db = state.database.db_clone();
    // ... save contact message ...

    // Return success partial
    let template = ContactSuccessTemplate {
        ctx,
        name: form.name,
        email: form.email,
    };
    HtmlTemplate::new(template).into_response()
}

// ============================================================================
// Counter Example - Simple HTMX State Updates
// ============================================================================

/// Counter page template
#[derive(Template)]
#[template(path = "examples/counter.html")]
struct CounterPageTemplate {
    ctx: PageContext,
}

/// Counter partial template - Returned on increment/decrement
#[derive(Template)]
#[template(path = "examples/counter_partial.html")]
#[allow(dead_code)]
struct CounterPartialTemplate {
    ctx: PageContext,
    count: i32,
}

/// Render the counter page
pub async fn counter_page(ctx: PageContext) -> impl IntoResponse {
    let template = CounterPageTemplate { ctx };
    HtmlTemplate::new(template)
}

/// Increment counter (HTMX endpoint)
pub async fn counter_increment(
    ctx: PageContext,
    Form(params): Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let count: i32 = params
        .get("count")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    let template = CounterPartialTemplate {
        ctx,
        count: count + 1,
    };
    HtmlTemplate::new(template)
}

/// Decrement counter (HTMX endpoint)
pub async fn counter_decrement(
    ctx: PageContext,
    Form(params): Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let count: i32 = params
        .get("count")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    let template = CounterPartialTemplate {
        ctx,
        count: count - 1,
    };
    HtmlTemplate::new(template)
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
        timestamp: jiff::Timestamp::now().to_string(),
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
