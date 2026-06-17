//! Post controller
//!
//! Handles CRUD operations for blog posts.
//! This serves as an example of a complete vertical slice feature.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use serde::{Deserialize, Serialize};

use crate::app::AppState;
use crate::middleware::CurrentUserId;
use crate::models::Post;
use crate::util::errors::{bad_request, not_found, server_error, AppResult};

/// Request body for creating a new post
#[derive(Debug, Deserialize)]
pub struct CreatePostRequest {
    pub title: String,
    pub content: String,
}

/// Request body for updating a post
#[derive(Debug, Deserialize)]
pub struct UpdatePostRequest {
    pub title: String,
    pub content: String,
}

/// Response for a single post
#[derive(Debug, Serialize)]
pub struct PostResponse {
    pub id: u64,
    pub title: String,
    pub content: String,
    pub published: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// List all posts for the current user
pub async fn list_posts(
    CurrentUserId(user_id): CurrentUserId,
    State(state): State<AppState>,
) -> AppResult<Json<Vec<PostResponse>>> {
    let mut db = state.0.database.db_clone();

    let posts = Post::filter(Post::fields().user_id().eq(user_id))
        .exec(&mut db)
        .await
        .map_err(|e| server_error(e.to_string()))?;

    let posts: Vec<PostResponse> = posts
        .into_iter()
        .map(|p| PostResponse {
            id: p.id,
            title: p.title,
            content: p.content,
            published: p.published,
            created_at: p.created_at.to_string(),
            updated_at: p.updated_at.to_string(),
        })
        .collect();

    Ok(Json(posts))
}

/// Show a single post
pub async fn show_post(
    Path(id): Path<u64>,
    CurrentUserId(user_id): CurrentUserId,
    State(state): State<AppState>,
) -> AppResult<Json<PostResponse>> {
    let mut db = state.0.database.db_clone();

    let post = Post::filter(Post::fields().id().eq(id))
        .filter(Post::fields().user_id().eq(user_id))
        .first()
        .exec(&mut db)
        .await
        .map_err(|e| server_error(e.to_string()))?
        .ok_or_else(not_found)?;

    let post_response = PostResponse {
        id: post.id,
        title: post.title,
        content: post.content,
        published: post.published,
        created_at: post.created_at.to_string(),
        updated_at: post.updated_at.to_string(),
    };

    Ok(Json(post_response))
}

/// Create a new post
pub async fn create_post(
    CurrentUserId(user_id): CurrentUserId,
    State(state): State<AppState>,
    Json(req): Json<CreatePostRequest>,
) -> AppResult<impl IntoResponse> {
    // Validate input
    if req.title.trim().is_empty() {
        return Err(bad_request("Title cannot be empty"));
    }
    if req.content.trim().is_empty() {
        return Err(bad_request("Content cannot be empty"));
    }

    let mut db = state.0.database.db_clone();

    let post = Post::new(user_id, req.title, req.content);

    let post = toasty::create!(Post {
        user_id: post.user_id,
        title: post.title,
        content: post.content,
        published: post.published,
        created_at: post.created_at,
        updated_at: post.updated_at,
    })
    .exec(&mut db)
    .await
    .map_err(|e| server_error(e.to_string()))?;

    let response = PostResponse {
        id: post.id,
        title: post.title,
        content: post.content,
        published: post.published,
        created_at: post.created_at.to_string(),
        updated_at: post.updated_at.to_string(),
    };

    Ok((StatusCode::CREATED, Json(response)))
}

/// Update a post
pub async fn update_post(
    Path(id): Path<u64>,
    CurrentUserId(user_id): CurrentUserId,
    State(state): State<AppState>,
    Json(req): Json<UpdatePostRequest>,
) -> AppResult<Json<PostResponse>> {
    // Validate input
    if req.title.trim().is_empty() {
        return Err(bad_request("Title cannot be empty"));
    }
    if req.content.trim().is_empty() {
        return Err(bad_request("Content cannot be empty"));
    }

    let mut db = state.0.database.db_clone();

    let mut post = Post::filter(Post::fields().id().eq(id))
        .filter(Post::fields().user_id().eq(user_id))
        .first()
        .exec(&mut db)
        .await
        .map_err(|e| server_error(e.to_string()))?
        .ok_or_else(not_found)?;

    let new_title = req.title.clone();
    let new_content = req.content.clone();
    let new_updated_at = jiff::Timestamp::now();

    toasty::update!(post {
        title: new_title.clone(),
        content: new_content.clone(),
        updated_at: new_updated_at,
    })
    .exec(&mut db)
    .await
    .map_err(|e| server_error(e.to_string()))?;

    let response = PostResponse {
        id: post.id,
        title: new_title,
        content: new_content,
        published: post.published,
        created_at: post.created_at.to_string(),
        updated_at: new_updated_at.to_string(),
    };

    Ok(Json(response))
}

/// Delete a post
pub async fn delete_post(
    Path(id): Path<u64>,
    CurrentUserId(user_id): CurrentUserId,
    State(state): State<AppState>,
) -> AppResult<StatusCode> {
    let mut db = state.0.database.db_clone();

    let post = Post::filter(Post::fields().id().eq(id))
        .filter(Post::fields().user_id().eq(user_id))
        .first()
        .exec(&mut db)
        .await
        .map_err(|e| server_error(e.to_string()))?
        .ok_or_else(not_found)?;

    post.delete()
        .exec(&mut db)
        .await
        .map_err(|e| server_error(e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

/// Publish a post
pub async fn publish_post(
    Path(id): Path<u64>,
    CurrentUserId(user_id): CurrentUserId,
    State(state): State<AppState>,
) -> AppResult<Json<PostResponse>> {
    let mut db = state.0.database.db_clone();

    let mut post = Post::filter(Post::fields().id().eq(id))
        .filter(Post::fields().user_id().eq(user_id))
        .first()
        .exec(&mut db)
        .await
        .map_err(|e| server_error(e.to_string()))?
        .ok_or_else(not_found)?;

    let new_published = true;
    let new_updated_at = jiff::Timestamp::now();

    toasty::update!(post {
        published: new_published,
        updated_at: new_updated_at,
    })
    .exec(&mut db)
    .await
    .map_err(|e| server_error(e.to_string()))?;

    let response = PostResponse {
        id: post.id,
        title: post.title,
        content: post.content,
        published: new_published,
        created_at: post.created_at.to_string(),
        updated_at: new_updated_at.to_string(),
    };

    Ok(Json(response))
}

/// Unpublish a post
pub async fn unpublish_post(
    Path(id): Path<u64>,
    CurrentUserId(user_id): CurrentUserId,
    State(state): State<AppState>,
) -> AppResult<Json<PostResponse>> {
    let mut db = state.0.database.db_clone();

    let mut post = Post::filter(Post::fields().id().eq(id))
        .filter(Post::fields().user_id().eq(user_id))
        .first()
        .exec(&mut db)
        .await
        .map_err(|e| server_error(e.to_string()))?
        .ok_or_else(not_found)?;

    let new_published = false;
    let new_updated_at = jiff::Timestamp::now();

    toasty::update!(post {
        published: new_published,
        updated_at: new_updated_at,
    })
    .exec(&mut db)
    .await
    .map_err(|e| server_error(e.to_string()))?;

    let response = PostResponse {
        id: post.id,
        title: post.title,
        content: post.content,
        published: new_published,
        created_at: post.created_at.to_string(),
        updated_at: new_updated_at.to_string(),
    };

    Ok(Json(response))
}
