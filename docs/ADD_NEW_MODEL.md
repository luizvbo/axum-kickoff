# Add a New Model

This guide shows you how to add a new database model using Toasty ORM.

## Overview

Toasty uses a schema-based approach where you define your models in Rust code. Migrations are generated as SQL files and applied explicitly (`just migration-generate` / `just migration-apply`) — the derive macro itself needs no codegen step.

## Step 1: Define the Model

Create a new file in `src/models/` (e.g., `src/models/post.rs`):

```rust
use toasty::Model;

#[derive(Debug, Model)]
pub struct Post {
    /// Primary key - auto-generated
    #[key]
    #[auto]
    pub id: u64,

    /// Foreign key to the user who created this post
    pub user_id: u64,

    /// Post title
    pub title: String,

    /// Post content
    pub content: String,

    /// Whether the post is published
    pub published: bool,

    /// Timestamp when the post was created
    pub created_at: jiff::Timestamp,

    /// Timestamp when the post was last updated
    pub updated_at: jiff::Timestamp,
}
```

## Step 2: Register the Model

Add the model to `src/models/mod.rs`:

```rust
pub mod user;
pub mod token;
pub mod post;
pub mod product;  // Add this line

pub use user::User;
pub use token::{ActionScope, ApiToken, ResourceScope};
pub use post::Post;
pub use product::Product;  // Add this line
```

## Step 3: Generate a Migration

`#[derive(toasty::Model)]` generates the query methods at compile time — but
the database schema still needs a migration:

```bash
cargo run --bin {{project-name}} -- migrate migration generate
# then apply it:
cargo run --bin {{project-name}} -- migrate
# or: just migration-generate && just migration-apply
```

## Step 4: Add Helper Methods (Optional)

Add convenience methods to your model implementation:

```rust
impl Post {
    /// Create a new post
    pub fn new(user_id: u64, title: String, content: String) -> Self {
        let now = jiff::Timestamp::now();
        Self {
            id: 0, // Will be auto-generated
            user_id,
            title,
            content,
            published: false,
            created_at: now,
            updated_at: now,
        }
    }

    /// Mark the post as published
    pub fn publish(&mut self) {
        self.published = true;
        self.updated_at = jiff::Timestamp::now();
    }

    /// Update the post content
    pub fn update_content(&mut self, content: String) {
        self.content = content;
        self.updated_at = jiff::Timestamp::now();
    }
}
```

## Step 5: Create a Test Data Builder

Add a test helper in `src/tests/builders.rs`:

```rust
use crate::models::Post;

pub struct PostBuilder {
    user_id: u64,
    title: String,
    content: String,
    published: bool,
}

impl PostBuilder {
    pub fn new(user_id: u64) -> Self {
        Self {
            user_id,
            title: "Test Post".to_string(),
            content: "Test content".to_string(),
            published: false,
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    pub fn content(mut self, content: impl Into<String>) -> Self {
        self.content = content.into();
        self
    }

    pub fn published(mut self, published: bool) -> Self {
        self.published = published;
        self
    }

    /// Insert into the database (matches `src/tests/builders.rs`)
    pub async fn build(self, db: &mut toasty::Db) -> anyhow::Result<Post> {
        let post = toasty::create!(Post {
            user_id: self.user_id,
            title: self.title,
            content: self.content,
            published: self.published,
            created_at: jiff::Timestamp::now(),
            updated_at: jiff::Timestamp::now(),
        })
        .exec(db)
        .await?;
        Ok(post)
    }
}
```

## Complete Example

Here's a complete example for a `Post` model:

### Model Definition (`src/models/post.rs`)

```rust
use toasty::Model;

#[derive(Debug, Model)]
pub struct Post {
    #[key]
    #[auto]
    pub id: u64,

    pub user_id: u64,
    pub title: String,
    pub content: String,
    pub published: bool,
    pub created_at: jiff::Timestamp,
    pub updated_at: jiff::Timestamp,
}

impl Post {
    pub fn new(user_id: u64, title: String, content: String) -> Self {
        let now = jiff::Timestamp::now();
        Self {
            id: 0,
            user_id,
            title,
            content,
            published: false,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn publish(&mut self) {
        self.published = true;
        self.updated_at = jiff::Timestamp::now();
    }
}
```

### Using the Model in a Controller

```rust
use axum::extract::State;
use crate::app::AppState;
use crate::middleware::CurrentUserId;
use crate::models::Post;
use crate::util::errors::{db_error, AppResult};

pub async fn create_post(
    CurrentUserId(user_id): CurrentUserId,
    State(state): State<AppState>,
    Json(req): Json<CreatePostRequest>,
) -> AppResult<impl IntoResponse> {
    let mut db = state.0.database.db_clone();

    let post = toasty::create!(Post {
        user_id,
        title: req.title,
        content: req.content,
        published: false,
        created_at: jiff::Timestamp::now(),
        updated_at: jiff::Timestamp::now(),
    })
    .exec(&mut db)
    .await
    .map_err(db_error)?;

    Ok(Json(post))
}

pub async fn list_posts(
    State(state): State<AppState>,
) -> AppResult<impl IntoResponse> {
    let mut db = state.0.database.db_clone();

    let posts = Post::all()
        .exec(&mut db)
        .await
        .map_err(db_error)?;

    Ok(Json(posts))
}
```

### Testing with the Builder

```rust
#[tokio::test]
async fn test_post_builder() {
    let app = TestApp::new().await;
    let mut db = app.db().db_clone();

    let post = app
        .post_builder(1, "My Post")
        .published(true)
        .build(&mut db)
        .await
        .unwrap();

    assert_eq!(post.title, "My Post");
    assert!(post.published);
}
```

## Adding Relationships

To add relationships between models, use foreign keys:

```rust
#[derive(Debug, Model)]
pub struct Comment {
    #[key]
    #[auto]
    pub id: u64,

    pub post_id: u64,  // Foreign key to Post
    pub user_id: u64,  // Foreign key to User
    pub content: String,
    pub created_at: jiff::Timestamp,
}
```

Query with relationships:

```rust
let comments = Comment::filter(Comment::fields().post_id().eq(post_id))
    .exec(&mut db)
    .await?;
```

## Adding Indexes

Add indexes to improve query performance:

```rust
#[derive(Debug, Model)]
pub struct Post {
    #[key]
    #[auto]
    pub id: u64,

    #[index]
    pub user_id: u64,  // Index for faster user queries

    #[index]
    pub published: bool,  // Index for filtering published posts

    pub title: String,
    pub content: String,
    pub created_at: jiff::Timestamp,
    pub updated_at: jiff::Timestamp,
}
```

## Migration Notes

Nothing applies migrations automatically — `server` connects but never
migrates. After generating a migration, apply it with
`cargo run -- migrate` / `just migration-apply`. For production:

1. Always back up your database before schema changes
2. Test migrations in a staging environment first
3. Deploy via `script/release.sh`, which runs `migrate` before `server`

## Next Steps

- Learn how to [add a new page](ADD_NEW_PAGE.md) to display your model
- Learn how to [add a protected route](ADD_PROTECTED_ROUTE.md) for model operations
- Learn how to [add an HTMX form](ADD_HTMX_FORM.md) for creating model instances
