//! Post controller
//!
//! Handles CRUD operations for blog posts.
//! This serves as an example of a complete vertical slice feature.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::app::AppState;
use crate::middleware::CurrentUserId;
use crate::models::Post;
use crate::util::errors::{bad_request, internal_error, not_found, AppResult};
use crate::util::ApiResponse;

/// Request body for creating a new post
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreatePostRequest {
    pub title: String,
    pub content: String,
}

/// Request body for updating a post
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdatePostRequest {
    pub title: String,
    pub content: String,
}

/// Response for a single post
#[derive(Debug, Serialize, ToSchema)]
pub struct PostResponse {
    pub id: u64,
    pub title: String,
    pub content: String,
    pub published: bool,
    pub created_at: String,
    pub updated_at: String,
}

const DEFAULT_PER_PAGE: usize = 20;
const MAX_PER_PAGE: usize = 100;

/// Query parameters for post list pagination
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListPostsParams {
    /// Page number (1-based, default 1)
    #[param(minimum = 1)]
    pub page: Option<u32>,
    /// Items per page (default 20, max 100)
    #[param(minimum = 1)]
    pub per_page: Option<u32>,
}

/// Response for paginated post list
#[derive(Debug, Serialize, ToSchema)]
pub struct ListPostsResponse {
    pub data: Vec<PostResponse>,
    pub page: u32,
    pub per_page: usize,
}

/// List posts for the current user with pagination
#[utoipa::path(
    get,
    path = "/api/v1/posts",
    params(
        ListPostsParams
    ),
    responses(
        (status = 200, description = "Paginated list of posts", body = ListPostsResponse),
        (status = 401, description = "Unauthorized")
    ),
    tag = "Posts",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn list_posts(
    CurrentUserId(user_id): CurrentUserId,
    State(state): State<AppState>,
    Query(params): Query<ListPostsParams>,
) -> AppResult<Json<ListPostsResponse>> {
    let page = params.page.unwrap_or(1).max(1);
    let per_page = params
        .per_page
        .map(|p| (p as usize).min(MAX_PER_PAGE))
        .unwrap_or(DEFAULT_PER_PAGE);
    let offset = ((page - 1) as usize) * per_page;

    let mut db = state.0.database.db_clone();

    let posts = Post::filter(Post::fields().user_id().eq(user_id))
        .limit(per_page)
        .offset(offset)
        .exec(&mut db)
        .await
        .map_err(internal_error)?;

    let data: Vec<PostResponse> = posts
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

    Ok(Json(ListPostsResponse {
        data,
        page,
        per_page,
    }))
}

/// Show a single post
#[utoipa::path(
    get,
    path = "/api/v1/posts/{id}",
    params(
        ("id" = u64, Path, description = "Post ID")
    ),
    responses(
        (status = 200, description = "Post details", body = PostResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Post not found")
    ),
    tag = "Posts",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn show_post(
    Path(id): Path<u64>,
    CurrentUserId(user_id): CurrentUserId,
    State(state): State<AppState>,
) -> AppResult<Json<ApiResponse<PostResponse>>> {
    let mut db = state.0.database.db_clone();

    let post = Post::filter(Post::fields().id().eq(id))
        .filter(Post::fields().user_id().eq(user_id))
        .first()
        .exec(&mut db)
        .await
        .map_err(internal_error)?
        .ok_or_else(not_found)?;

    let post_response = PostResponse {
        id: post.id,
        title: post.title,
        content: post.content,
        published: post.published,
        created_at: post.created_at.to_string(),
        updated_at: post.updated_at.to_string(),
    };

    Ok(Json(ApiResponse::new(post_response)))
}

/// Create a new post
#[utoipa::path(
    post,
    path = "/api/v1/posts",
    request_body = CreatePostRequest,
    responses(
        (status = 201, description = "Post created", body = PostResponse),
        (status = 401, description = "Unauthorized"),
        (status = 400, description = "Bad request")
    ),
    tag = "Posts",
    security(
        ("bearer_auth" = [])
    )
)]
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
    .map_err(internal_error)?;

    let response = PostResponse {
        id: post.id,
        title: post.title,
        content: post.content,
        published: post.published,
        created_at: post.created_at.to_string(),
        updated_at: post.updated_at.to_string(),
    };

    Ok((StatusCode::CREATED, Json(ApiResponse::new(response))))
}

/// Update a post
#[utoipa::path(
    patch,
    path = "/api/v1/posts/{id}",
    params(
        ("id" = u64, Path, description = "Post ID")
    ),
    request_body = UpdatePostRequest,
    responses(
        (status = 200, description = "Post updated", body = PostResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Post not found"),
        (status = 400, description = "Bad request")
    ),
    tag = "Posts",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn update_post(
    Path(id): Path<u64>,
    CurrentUserId(user_id): CurrentUserId,
    State(state): State<AppState>,
    Json(req): Json<UpdatePostRequest>,
) -> AppResult<Json<ApiResponse<PostResponse>>> {
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
        .map_err(internal_error)?
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
    .map_err(internal_error)?;

    let response = PostResponse {
        id: post.id,
        title: new_title,
        content: new_content,
        published: post.published,
        created_at: post.created_at.to_string(),
        updated_at: new_updated_at.to_string(),
    };

    Ok(Json(ApiResponse::new(response)))
}

/// Delete a post
#[utoipa::path(
    delete,
    path = "/api/v1/posts/{id}",
    params(
        ("id" = u64, Path, description = "Post ID")
    ),
    responses(
        (status = 204, description = "Post deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Post not found")
    ),
    tag = "Posts",
    security(
        ("bearer_auth" = [])
    )
)]
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
        .map_err(internal_error)?
        .ok_or_else(not_found)?;

    post.delete()
        .exec(&mut db)
        .await
        .map_err(internal_error)?;

    Ok(StatusCode::NO_CONTENT)
}

/// Publish a post
#[utoipa::path(
    post,
    path = "/api/v1/posts/{id}/publish",
    params(
        ("id" = u64, Path, description = "Post ID")
    ),
    responses(
        (status = 200, description = "Post published", body = PostResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Post not found")
    ),
    tag = "Posts",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn publish_post(
    Path(id): Path<u64>,
    CurrentUserId(user_id): CurrentUserId,
    State(state): State<AppState>,
) -> AppResult<Json<ApiResponse<PostResponse>>> {
    let mut db = state.0.database.db_clone();

    let mut post = Post::filter(Post::fields().id().eq(id))
        .filter(Post::fields().user_id().eq(user_id))
        .first()
        .exec(&mut db)
        .await
        .map_err(internal_error)?
        .ok_or_else(not_found)?;

    let new_published = true;
    let new_updated_at = jiff::Timestamp::now();

    toasty::update!(post {
        published: new_published,
        updated_at: new_updated_at,
    })
    .exec(&mut db)
    .await
    .map_err(internal_error)?;

    let response = PostResponse {
        id: post.id,
        title: post.title,
        content: post.content,
        published: new_published,
        created_at: post.created_at.to_string(),
        updated_at: new_updated_at.to_string(),
    };

    Ok(Json(ApiResponse::new(response)))
}

/// Unpublish a post
#[utoipa::path(
    post,
    path = "/api/v1/posts/{id}/unpublish",
    params(
        ("id" = u64, Path, description = "Post ID")
    ),
    responses(
        (status = 200, description = "Post unpublished", body = PostResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Post not found")
    ),
    tag = "Posts",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn unpublish_post(
    Path(id): Path<u64>,
    CurrentUserId(user_id): CurrentUserId,
    State(state): State<AppState>,
) -> AppResult<Json<ApiResponse<PostResponse>>> {
    let mut db = state.0.database.db_clone();

    let mut post = Post::filter(Post::fields().id().eq(id))
        .filter(Post::fields().user_id().eq(user_id))
        .first()
        .exec(&mut db)
        .await
        .map_err(internal_error)?
        .ok_or_else(not_found)?;

    let new_published = false;
    let new_updated_at = jiff::Timestamp::now();

    toasty::update!(post {
        published: new_published,
        updated_at: new_updated_at,
    })
    .exec(&mut db)
    .await
    .map_err(internal_error)?;

    let response = PostResponse {
        id: post.id,
        title: post.title,
        content: post.content,
        published: new_published,
        created_at: post.created_at.to_string(),
        updated_at: new_updated_at.to_string(),
    };

    Ok(Json(ApiResponse::new(response)))
}
