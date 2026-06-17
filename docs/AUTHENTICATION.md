# Authentication

This document describes the authentication system in axum-kickoff, including GitHub OAuth, session management, and API tokens.

## Overview

axum-kickoff supports multiple authentication methods:

- **GitHub OAuth**: OAuth 2.0 flow with GitHub
- **Session-Based**: Signed cookie sessions for web users
- **API Tokens**: Scoped tokens for programmatic access

## GitHub OAuth

### Setup

1. Create a GitHub OAuth App:
   - Go to [GitHub Developer Settings](https://github.com/settings/developers)
   - Click "New OAuth App"
   - Set the authorization callback URL to: `https://your-domain.com/auth/github/callback`

2. Configure environment variables:
   ```bash
   GITHUB_CLIENT_ID=your_client_id
   GITHUB_CLIENT_SECRET=your_client_secret
   GITHUB_REDIRECT_URI=https://your-domain.com/auth/github/callback
   ```

### OAuth Flow

```
1. User clicks "Login with GitHub"
2. Redirect to GitHub authorize endpoint
3. User authorizes application
4. GitHub redirects to callback endpoint with authorization code
5. Server exchanges code for access token
6. Server fetches user profile from GitHub
7. Server creates/updates user in database
8. Server creates session cookie
9. Redirect to dashboard
```

### Endpoints

- `GET /auth/github` - Initiate OAuth flow
- `GET /auth/github/callback` - OAuth callback
- `GET /auth/logout` - Logout and clear session

### Implementation

The OAuth flow is implemented in `src/controllers/auth.rs`:

```rust
pub async fn github_authorize(
    State(app): State<AppState>,
) -> Result<Redirect, AppError> {
    // Generate OAuth URL and redirect
}

pub async fn github_callback(
    Query(params): Query<CallbackParams>,
    State(app): State<AppState>,
) -> Result<Redirect, AppError> {
    // Exchange code for token
    // Fetch user profile
    // Create/update user
    // Create session
    // Redirect
}
```

## Session Management

### Overview

Sessions are managed using signed cookies with the `axum-extra` crate's cookie-signed feature.

### Session Key

The session key is configured via the `SESSION_KEY` environment variable:

```bash
SESSION_KEY=your-secret-key-minimum-64-bytes-long
```

**Important:** The session key must be at least 64 bytes long for security. Generate a secure key:

```bash
openssl rand -base64 64
```

### Session Data

Sessions store:

- User ID
- Session creation time
- Session expiration time

### Session Middleware

The session middleware in `src/middleware/session.rs`:

- Validates session cookies
- Extracts user information from sessions
- Attaches user context to requests

### Session Security

- **Signed Cookies**: Sessions are signed with HMAC to prevent tampering
- **Secure Flag**: Cookies are marked as secure in production (HTTPS only)
- **HttpOnly Flag**: Cookies are not accessible via JavaScript
- **SameSite**: Cookies are set with `SameSite=Lax` to prevent CSRF

### Session Expiration

Sessions expire after a configurable period (default: 24 hours). Configure via environment variable:

```bash
SESSION_EXPIRATION_HOURS=24
```

## API Tokens

### Overview

API tokens provide programmatic access to the API with fine-grained permissions via scopes.

### Token Structure

API tokens have the following properties:

- **Name**: Human-readable name for the token
- **Token Hash**: SHA-256 hash of the token (stored in database)
- **Action Scopes**: Permissions for actions (read, create, update, delete, admin)
- **Resource Scopes**: Permissions for specific resources
- **Expiration**: Optional expiration date
- **Last Used**: Timestamp of last use

### Creating API Tokens

#### Via API

```bash
curl -X POST http://localhost:3000/api/tokens \
  -H "Authorization: Bearer YOUR_SESSION_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "My Token",
    "action_scopes": ["read"],
    "resource_scopes": ["posts"],
    "expires_at": "2024-12-31T23:59:59Z"
  }'
```

#### Response

```json
{
  "id": "token_id",
  "name": "My Token",
  "token": "axk_abc123...",  // Only shown on creation
  "action_scopes": ["read"],
  "resource_scopes": ["posts"],
  "expires_at": "2024-12-31T23:59:59Z",
  "created_at": "2024-01-01T00:00:00Z"
}
```

**Important:** Save the token value immediately, as it won't be shown again.

### Using API Tokens

Include the token in the `Authorization` header:

```bash
curl http://localhost:3000/api/posts \
  -H "Authorization: Bearer axk_abc123..."
```

### Token Scopes

See [API Token Scopes Documentation](api-token-scopes.md) for detailed information about the scope system.

#### Action Scopes

- `read`: Can read resources
- `create`: Can create new resources
- `update`: Can modify existing resources
- `delete`: Can remove resources
- `admin`: Full administrative access

#### Resource Scopes

Resource scopes control which resources a token can access:

- `posts`: Only access posts
- `posts*`: Access posts and related resources (posts-comments, posts-meta)
- `*`: Access all resources

### Token Management

#### List Tokens

```bash
curl http://localhost:3000/api/tokens \
  -H "Authorization: Bearer YOUR_SESSION_TOKEN"
```

#### Revoke Token

```bash
curl -X DELETE http://localhost:3000/api/tokens/TOKEN_ID \
  -H "Authorization: Bearer YOUR_SESSION_TOKEN"
```

### Token Security

- **Hashed Storage**: Tokens are hashed with SHA-256 before storage
- **Scope Validation**: Tokens are validated against endpoint and resource scopes
- **Expiration**: Tokens can have optional expiration dates
- **Revocation**: Tokens can be revoked at any time

### Legacy Tokens

Tokens without scopes (legacy tokens) are granted full access for backward compatibility. However, new tokens should always specify scopes for security.

## Authentication Middleware

### Session Middleware

The session middleware (`src/middleware/session.rs`):

1. Extracts session cookie from request
2. Validates session signature
3. Checks session expiration
4. Attaches user context to request extensions

### API Token Middleware

The API token middleware (`src/middleware/api_token.rs`):

1. Extracts `Authorization` header
2. Validates token format
3. Looks up token in database
4. Checks token expiration
5. Validates token scopes against endpoint requirements
6. Attaches user context to request extensions

### AuthCheck Pattern

The `AuthCheck` pattern in `src/util/auth.rs` provides a declarative way to specify authentication requirements:

```rust
use crate::util::auth::AuthCheck;
use crate::models::token::ActionScope;

// Require read scope for listing posts
let check = AuthCheck::default()
    .with_action_scope(ActionScope::Read)
    .for_crate("posts");

// Admin scope grants all permissions
let check = AuthCheck::default()
    .with_action_scope(ActionScope::Admin);
```

## User Model

The user model (`src/models/user.rs`) stores:

- **GitHub ID**: Unique GitHub user ID
- **GitHub Login**: GitHub username
- **Avatar URL**: Profile picture URL
- **Created At**: Account creation timestamp
- **Account Lock Reason**: Optional reason for account lock
- **Account Lock Until**: Optional lock expiration

### Account Locking

Accounts can be locked to prevent abuse:

```rust
user.account_lock_reason = Some("Violation of terms".to_string());
user.account_lock_until = Some(Utc::now() + Duration::days(7));
```

Locked accounts cannot authenticate until the lock expires.

## Security Best Practices

### For OAuth

1. **Use HTTPS**: OAuth requires HTTPS in production
2. **Validate Redirect URI**: Ensure redirect URI matches exactly
3. **Scope Limitation**: Request minimum required scopes from GitHub
4. **State Parameter**: Use state parameter to prevent CSRF (implemented)

### For Sessions

1. **Secure Session Key**: Use a cryptographically secure 64+ byte key
2. **Rotate Keys**: Rotate session keys periodically
3. **Short Expiration**: Use short session expiration (24 hours or less)
4. **Secure Cookies**: Enable secure flag in production
5. **HttpOnly**: Always use HttpOnly flag

### For API Tokens

1. **Principle of Least Privilege**: Grant only necessary scopes
2. **Resource Scopes**: Restrict tokens to specific resources
3. **Set Expiration**: Always set expiration dates
4. **Rotate Regularly**: Rotate tokens periodically
5. **Monitor Usage**: Track token usage and revoke suspicious tokens
6. **Secure Storage**: Store tokens securely (environment variables, secret managers)

### General

1. **Never Log Tokens**: Never log session keys or API tokens
2. **Use Environment Variables**: Store secrets in environment variables
3. **Audit Logs**: Log authentication events for security monitoring
4. **Rate Limiting**: Apply rate limiting to authentication endpoints
5. **Account Locking**: Implement account locking for abuse prevention

## Troubleshooting

### OAuth Callback Fails

- Check `GITHUB_REDIRECT_URI` matches GitHub OAuth app settings exactly
- Ensure HTTPS is used in production
- Verify `GITHUB_CLIENT_ID` and `GITHUB_CLIENT_SECRET` are correct

### Session Not Persisting

- Check `SESSION_KEY` is at least 64 bytes
- Verify cookie domain matches application domain
- Ensure cookies are enabled in browser
- Check for CORS issues (if using separate frontend)

### API Token Rejected

- Verify token is correctly formatted: `Bearer axk_...`
- Check token hasn't expired
- Verify token has required scopes for endpoint
- Ensure token hasn't been revoked

### Account Locked

- Check `account_lock_until` timestamp
- Verify lock reason in database
- Contact administrator if lock is incorrect

## Implementation Details

### GitHub OAuth Implementation

The OAuth flow uses the `oauth2` crate:

```rust
use oauth2::{
    AuthorizationCode,
    ClientId,
    ClientSecret,
    CsrfToken,
    RedirectUrl,
    TokenResponse,
};
```

### Session Encoding

Sessions are encoded using `axum-extra`'s signed cookies:

```rust
use axum_extra::extract::cookie::SignedCookieJar;
use axum_extra::extract::cookie::Key;
```

### Token Hashing

API tokens are hashed using SHA-256:

```rust
use sha2::{Sha256, Digest};
use hex;

let hash = Sha256::digest(token.as_bytes());
let token_hash = hex::encode(hash);
```

## See Also

- [API Token Scopes Documentation](api-token-scopes.md)
- [Configuration Documentation](CONFIGURATION.md)
- [Middleware Documentation](MIDDLEWARE.md)
- [Security Headers Documentation](MIDDLEWARE.md#security-headers)
