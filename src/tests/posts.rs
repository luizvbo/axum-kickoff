//! Post CRUD integration tests
//!
//! Tests all 7 post endpoints: list, show, create, update, delete, publish, unpublish.

use crate::tests::{CookieUser, RequestHelper, TestApp};
use http::StatusCode;
use serde_json::{json, Value};

/// Helper to set up an authenticated user with CSRF token.
/// Returns (cookie_user, csrf_token, user_id).
async fn setup_auth_user() -> (CookieUser, String, u64) {
    let app = TestApp::new().await;
    let mut db = app.db().db_clone();

    let user = app
        .user_builder("post_test_user")
        .build(&mut db)
        .await
        .expect("Failed to create user");

    let user_id = user.id;
    let session_key = app.config.session_key.clone();
    let cookie_user = CookieUser::new(app, user_id, session_key);
    let csrf_token = cookie_user.init_csrf().await;

    (cookie_user, csrf_token, user_id)
}

#[tokio::test]
async fn create_post_returns_201() {
    let (cookie_user, csrf_token, _) = setup_auth_user().await;

    let headers = cookie_user.headers_with_csrf(&csrf_token);
    let response = cookie_user
        .post_with_headers::<Value>(
            "/api/v1/posts",
            &json!({
                "title": "My First Post",
                "content": "Hello world!"
            }),
            headers,
        )
        .await;

    response.assert_status(StatusCode::CREATED);

    let body = response.into_json::<Value>().await;
    let data = &body["data"];
    assert_eq!(data["title"], "My First Post");
    assert_eq!(data["content"], "Hello world!");
    assert_eq!(data["published"], false);
}

#[tokio::test]
async fn create_post_with_empty_title_returns_400() {
    let (cookie_user, csrf_token, _) = setup_auth_user().await;

    let headers = cookie_user.headers_with_csrf(&csrf_token);
    let response = cookie_user
        .post_with_headers::<Value>(
            "/api/v1/posts",
            &json!({
                "title": "  ",
                "content": "content"
            }),
            headers,
        )
        .await;

    response.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_post_with_empty_content_returns_400() {
    let (cookie_user, csrf_token, _) = setup_auth_user().await;

    let headers = cookie_user.headers_with_csrf(&csrf_token);
    let response = cookie_user
        .post_with_headers::<Value>(
            "/api/v1/posts",
            &json!({
                "title": "title",
                "content": ""
            }),
            headers,
        )
        .await;

    response.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn list_posts_returns_user_posts() {
    let (cookie_user, _csrf_token, user_id) = setup_auth_user().await;

    // Insert posts directly into DB
    let mut db = cookie_user.app().db().db_clone();
    cookie_user
        .app()
        .post_builder(user_id, "Post 1")
        .build(&mut db)
        .await
        .expect("Failed to create post 1");
    cookie_user
        .app()
        .post_builder(user_id, "Post 2")
        .build(&mut db)
        .await
        .expect("Failed to create post 2");

    let response = cookie_user.get::<Value>("/api/v1/posts").await;

    response.assert_status(StatusCode::OK);

    let body = response.into_json::<Value>().await;
    let data = body["data"].as_array().expect("data should be an array");
    assert_eq!(data.len(), 2);
}

#[tokio::test]
async fn list_posts_returns_all_posts() {
    let app = TestApp::new().await;
    let mut db = app.db().db_clone();

    // Create two users
    let user1 = app
        .user_builder("user1")
        .build(&mut db)
        .await
        .expect("Failed to create user1");
    let user2 = app
        .user_builder("user2")
        .build(&mut db)
        .await
        .expect("Failed to create user2");

    // Create posts for both users
    app.post_builder(user1.id, "User1 Post")
        .build(&mut db)
        .await
        .expect("Failed to create post for user1");
    app.post_builder(user2.id, "User2 Post")
        .build(&mut db)
        .await
        .expect("Failed to create post for user2");

    let session_key = app.config.session_key.clone();
    let cookie_user = CookieUser::new(app, user1.id, session_key);
    let _ = cookie_user.init_csrf().await;

    let response = cookie_user.get::<Value>("/api/v1/posts").await;

    response.assert_status(StatusCode::OK);

    let body = response.into_json::<Value>().await;
    let data = body["data"].as_array().expect("data should be an array");
    assert_eq!(data.len(), 2);
}

#[tokio::test]
async fn show_post_returns_post_details() {
    let (cookie_user, _csrf_token, user_id) = setup_auth_user().await;

    let mut db = cookie_user.app().db().db_clone();
    let post = cookie_user
        .app()
        .post_builder(user_id, "Show Me")
        .content("Detailed content")
        .build(&mut db)
        .await
        .expect("Failed to create post");

    let response = cookie_user
        .get::<Value>(&format!("/api/v1/posts/{}", post.id))
        .await;

    response.assert_status(StatusCode::OK);

    let body = response.into_json::<Value>().await;
    let data = &body["data"];
    assert_eq!(data["title"], "Show Me");
    assert_eq!(data["content"], "Detailed content");
}

#[tokio::test]
async fn show_post_returns_404_for_nonexistent() {
    let (cookie_user, _csrf_token, _) = setup_auth_user().await;

    let response = cookie_user.get::<Value>("/api/v1/posts/99999").await;

    response.assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn show_post_returns_public_post_details() {
    let app = TestApp::new().await;
    let mut db = app.db().db_clone();

    let user1 = app
        .user_builder("owner")
        .build(&mut db)
        .await
        .expect("Failed to create owner");

    let post = app
        .post_builder(user1.id, "Owner's Post")
        .build(&mut db)
        .await
        .expect("Failed to create post");

    let session_key = app.config.session_key.clone();
    let cookie_user = CookieUser::new(app, user1.id, session_key);
    let _ = cookie_user.init_csrf().await;

    let response = cookie_user
        .get::<Value>(&format!("/api/v1/posts/{}", post.id))
        .await;

    response.assert_status(StatusCode::OK);

    let body = response.into_json::<Value>().await;
    let data = &body["data"];
    assert_eq!(data["title"], "Owner's Post");
}

#[tokio::test]
async fn update_post_returns_updated_post() {
    let (cookie_user, csrf_token, user_id) = setup_auth_user().await;

    let mut db = cookie_user.app().db().db_clone();
    let post = cookie_user
        .app()
        .post_builder(user_id, "Original Title")
        .content("Original content")
        .build(&mut db)
        .await
        .expect("Failed to create post");

    let headers = cookie_user.headers_with_csrf(&csrf_token);
    let response = cookie_user
        .patch_with_headers::<Value>(
            &format!("/api/v1/posts/{}", post.id),
            &json!({
                "title": "Updated Title",
                "content": "Updated content"
            }),
            headers,
        )
        .await;

    response.assert_status(StatusCode::OK);

    let body = response.into_json::<Value>().await;
    let data = &body["data"];
    assert_eq!(data["title"], "Updated Title");
    assert_eq!(data["content"], "Updated content");
}

#[tokio::test]
async fn update_post_returns_404_for_nonexistent() {
    let (cookie_user, csrf_token, _) = setup_auth_user().await;

    let headers = cookie_user.headers_with_csrf(&csrf_token);
    let response = cookie_user
        .patch_with_headers::<Value>(
            "/api/v1/posts/99999",
            &json!({
                "title": "Updated",
                "content": "Updated"
            }),
            headers,
        )
        .await;

    response.assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn update_post_with_empty_title_returns_400() {
    let (cookie_user, csrf_token, user_id) = setup_auth_user().await;

    let mut db = cookie_user.app().db().db_clone();
    let post = cookie_user
        .app()
        .post_builder(user_id, "Title")
        .build(&mut db)
        .await
        .expect("Failed to create post");

    let headers = cookie_user.headers_with_csrf(&csrf_token);
    let response = cookie_user
        .patch_with_headers::<Value>(
            &format!("/api/v1/posts/{}", post.id),
            &json!({
                "title": "",
                "content": "content"
            }),
            headers,
        )
        .await;

    response.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn delete_post_returns_204() {
    let (cookie_user, csrf_token, user_id) = setup_auth_user().await;

    let mut db = cookie_user.app().db().db_clone();
    let post = cookie_user
        .app()
        .post_builder(user_id, "Delete Me")
        .build(&mut db)
        .await
        .expect("Failed to create post");

    let headers = cookie_user.headers_with_csrf(&csrf_token);
    let response = cookie_user
        .delete_with_headers::<Value>(&format!("/api/v1/posts/{}", post.id), headers)
        .await;

    response.assert_status(StatusCode::NO_CONTENT);

    // Verify the post is gone
    let response = cookie_user
        .get::<Value>(&format!("/api/v1/posts/{}", post.id))
        .await;
    response.assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn delete_post_returns_404_for_nonexistent() {
    let (cookie_user, csrf_token, _) = setup_auth_user().await;

    let headers = cookie_user.headers_with_csrf(&csrf_token);
    let response = cookie_user
        .delete_with_headers::<Value>("/api/v1/posts/99999", headers)
        .await;

    response.assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn delete_post_returns_404_for_other_users_post() {
    let app = TestApp::new().await;
    let mut db = app.db().db_clone();

    let user1 = app
        .user_builder("owner")
        .build(&mut db)
        .await
        .expect("Failed to create owner");
    let user2 = app
        .user_builder("deleter")
        .build(&mut db)
        .await
        .expect("Failed to create deleter");

    let post = app
        .post_builder(user1.id, "Owner's Post")
        .build(&mut db)
        .await
        .expect("Failed to create post");

    let session_key = app.config.session_key.clone();
    let cookie_user = CookieUser::new(app, user2.id, session_key);
    let csrf_token = cookie_user.init_csrf().await;

    let headers = cookie_user.headers_with_csrf(&csrf_token);
    let response = cookie_user
        .delete_with_headers::<Value>(&format!("/api/v1/posts/{}", post.id), headers)
        .await;

    response.assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn publish_post_sets_published_true() {
    let (cookie_user, csrf_token, user_id) = setup_auth_user().await;

    let mut db = cookie_user.app().db().db_clone();
    let post = cookie_user
        .app()
        .post_builder(user_id, "Publish Me")
        .build(&mut db)
        .await
        .expect("Failed to create post");

    assert!(!post.published);

    let headers = cookie_user.headers_with_csrf(&csrf_token);
    let response = cookie_user
        .post_with_headers::<Value>(
            &format!("/api/v1/posts/{}/publish", post.id),
            &[] as &[u8],
            headers,
        )
        .await;

    response.assert_status(StatusCode::OK);

    let body = response.into_json::<Value>().await;
    let data = &body["data"];
    assert_eq!(data["published"], true);
}

#[tokio::test]
async fn publish_post_returns_404_for_nonexistent() {
    let (cookie_user, csrf_token, _) = setup_auth_user().await;

    let headers = cookie_user.headers_with_csrf(&csrf_token);
    let response = cookie_user
        .post_with_headers::<Value>("/api/v1/posts/99999/publish", &[] as &[u8], headers)
        .await;

    response.assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn unpublish_post_sets_published_false() {
    let (cookie_user, csrf_token, user_id) = setup_auth_user().await;

    let mut db = cookie_user.app().db().db_clone();
    let post = cookie_user
        .app()
        .post_builder(user_id, "Unpublish Me")
        .published(true)
        .build(&mut db)
        .await
        .expect("Failed to create post");

    assert!(post.published);

    let headers = cookie_user.headers_with_csrf(&csrf_token);
    let response = cookie_user
        .post_with_headers::<Value>(
            &format!("/api/v1/posts/{}/unpublish", post.id),
            &[] as &[u8],
            headers,
        )
        .await;

    response.assert_status(StatusCode::OK);

    let body = response.into_json::<Value>().await;
    let data = &body["data"];
    assert_eq!(data["published"], false);
}

#[tokio::test]
async fn unpublish_post_returns_404_for_nonexistent() {
    let (cookie_user, csrf_token, _) = setup_auth_user().await;

    let headers = cookie_user.headers_with_csrf(&csrf_token);
    let response = cookie_user
        .post_with_headers::<Value>("/api/v1/posts/99999/unpublish", &[] as &[u8], headers)
        .await;

    response.assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn posts_require_authentication() {
    let app = TestApp::new().await;
    let anon = crate::tests::AnonymousUser::new(app);

    let response = anon
        .post::<Value>("/api/v1/posts", &json!({"title": "t", "content": "c"}))
        .await;

    response.assert_status(StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn create_post_without_csrf_returns_error() {
    let app = TestApp::new().await;
    let mut db = app.db().db_clone();

    let user = app
        .user_builder("no_csrf_user")
        .build(&mut db)
        .await
        .expect("Failed to create user");

    let session_key = app.config.session_key.clone();
    let cookie_user = CookieUser::new(app, user.id, session_key);
    let _ = cookie_user.init_csrf().await;

    // POST without CSRF token header
    let response = cookie_user
        .post::<Value>("/api/v1/posts", &json!({"title": "t", "content": "c"}))
        .await;

    assert!(response.status().is_client_error());
    assert_ne!(response.status(), StatusCode::CREATED);
}
