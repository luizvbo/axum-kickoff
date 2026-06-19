# API Token Scopes

This document explains the generic API token scope system used in axum-kickoff.

## Overview

API tokens support fine-grained permissions through two types of scopes:

1. **Action Scopes** - Control what actions a token can perform
2. **Resource Scopes** - Control which resources a token can access

## Action Scopes

Action scopes define what actions a token can perform:

| Scope | Description | Example Actions |
|-------|-------------|----------------|
| `read` | Can read resources | GET requests, listing resources |
| `create` | Can create new resources | POST requests to create entities |
| `update` | Can modify existing resources | PUT/PATCH requests |
| `delete` | Can remove resources | DELETE requests |
| `admin` | Full administrative access | All actions, including administrative operations |

### Usage Example

```rust
use crate::util::auth::AuthCheck;
use crate::models::token::ActionScope;

// Require read scope for listing posts
let check = AuthCheck::default()
    .with_action_scope(ActionScope::Read);

// Require create scope for creating posts
let check = AuthCheck::default()
    .with_action_scope(ActionScope::Create);

// Admin scope grants all permissions
let check = AuthCheck::default()
    .with_action_scope(ActionScope::Admin);
```

## Resource Scopes

Resource scopes control which specific resources a token can access. They support wildcard patterns:

| Pattern | Description | Matches |
|---------|-------------|---------|
| `posts` | Exact match | Only `posts` |
| `posts*` | Prefix match | `posts`, `posts-comments`, `posts-meta` |
| `*` | Wildcard | All resources |

### Usage Example

```rust
use crate::util::auth::AuthCheck;

// Restrict token to only access posts
let check = AuthCheck::default()
    .for_resource("posts");

// Restrict token to posts and related resources
let check = AuthCheck::default()
    .for_resource("posts*");

// Allow token to access any resource (when combined with endpoint scope)
let check = AuthCheck::default()
    .allow_any_resource_scope();
```

## Combining Scopes

You can combine endpoint and resource scopes for fine-grained control:

```rust
// Token can read posts but not modify them
let check = AuthCheck::default()
    .with_action_scope(ActionScope::Read)
    .for_resource("posts");

// Token can create and update posts but not delete them
let check = AuthCheck::default()
    .with_action_scope(ActionScope::Create)
    .for_resource("posts*");
```

## Creating Scoped Tokens

When creating an API token, you can specify scopes:

```rust
use crate::models::token::{ApiToken, ActionScope, ResourceScope};

let action_scopes = vec![ActionScope::Read, ActionScope::Create];
let resource_scopes = vec!["posts".to_string(), "users".to_string()];

let token = ApiToken::new(
    user_id,
    "My Token".to_string(),
    hashed_token,
    Some(resource_scopes),
    Some(action_scopes),
    None, // no expiration
);
```

## Legacy Tokens

Tokens without scopes (legacy tokens) are granted full access for backward compatibility. However, new tokens should always specify scopes for security.

## Security Best Practices

1. **Principle of Least Privilege**: Grant only the minimum scopes needed
2. **Use Resource Scopes**: Restrict tokens to specific resources when possible
3. **Set Expiration**: Always set an expiration date for tokens
4. **Rotate Regularly**: Periodically rotate tokens, especially for production
5. **Monitor Usage**: Track token usage and revoke suspicious tokens

## Examples by Use Case

### Read-Only API Token
```rust
let action_scopes = vec![ActionScope::Read];
let resource_scopes = vec!["*".to_string()]; // All resources
```

### Content Management Token
```rust
let action_scopes = vec![ActionScope::Create, ActionScope::Update];
let resource_scopes = vec!["posts*".to_string(), "pages*".to_string()];
```

### Admin Token
```rust
let action_scopes = vec![ActionScope::Admin];
let resource_scopes = None; // Admin doesn't need resource restrictions
```

### Third-Party Integration Token
```rust
let action_scopes = vec![ActionScope::Read];
let resource_scopes = vec!["api*".to_string()]; // Only API endpoints
```

