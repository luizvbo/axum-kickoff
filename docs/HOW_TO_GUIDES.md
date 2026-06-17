# How-to Guides

This section contains practical guides for common tasks when building with axum-kickoff.

## Guides

### Adding Features

- **[Add a New Page](ADD_NEW_PAGE.md)** - Learn how to add a new page with routing, handlers, and templates
- **[Add a New Model](ADD_NEW_MODEL.md)** - Learn how to create database models using Toasty ORM
- **[Add a Protected Route](ADD_PROTECTED_ROUTE.md)** - Learn how to require authentication for routes
- **[Add an HTMX Form](ADD_HTMX_FORM.md)** - Learn how to create forms with CSRF protection and partial updates

### Deployment

- **[Production Checklist](PRODUCTION_CHECKLIST.md)** - Comprehensive checklist for deploying to production

## Quick Reference

### Creating a new feature
1. Define your model in `src/models/`
2. Run `cargo run --bin toasty` to generate database code
3. Create a controller in `src/controllers/`
4. Add routes in `src/router.rs`
5. Create templates in `templates/`
6. Test your changes

### Adding authentication
1. Use the `CurrentUser` extractor in your handler
2. The extractor automatically checks session or API token
3. Returns 401 for unauthenticated requests
4. See [Add a Protected Route](ADD_PROTECTED_ROUTE.md) for details

### Working with HTMX
1. Add `hx-post`, `hx-target`, `hx-swap` attributes to forms
2. Return HTML from your handler
3. HTMX automatically updates the page
4. See [Add an HTMX Form](ADD_HTMX_FORM.md) for details

## Common Patterns

### Fetching data in a handler
```rust
pub async fn list_items(
    State(state): State<AppState>,
) -> AppResult<impl IntoResponse> {
    let mut db = state.0.database.db_clone();

    let items = Item::all()
        .exec(&mut db)
        .await
        .map_err(|e| server_error(e.to_string()))?;

    Ok(Json(items))
}
```

### Creating data in a handler
```rust
pub async fn create_item(
    State(state): State<AppState>,
    Json(req): Json<CreateItemRequest>,
) -> AppResult<impl IntoResponse> {
    let mut db = state.0.database.db_clone();

    let item = toasty::create!(Item {
        name: req.name,
        created_at: jiff::Timestamp::now(),
    })
    .exec(&mut db)
    .await
    .map_err(|e| server_error(e.to_string()))?;

    Ok(Json(item))
}
```

### Filtering data
```rust
let items = Item::filter(Item::fields().user_id().eq(user_id))
    .exec(&mut db)
    .await?;
```

## Next Steps

- Read the [Architecture documentation](ARCHITECTURE.md) to understand the system design
- Review the [Authentication documentation](AUTHENTICATION.md) for auth system details
- Check the [Configuration reference](CONFIGURATION.md) for all available options
- See the [Deployment guide](DEPLOYMENT.md) for production deployment
