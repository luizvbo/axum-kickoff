# Authentication

This document describes the authentication system in {{project-name}}, including OAuth sign-in, session management, and API tokens.

## Overview

{{project-name}} supports multiple authentication methods:

{% if oauth_github or oauth_google or oauth_facebook %}- **OAuth sign-in**: OAuth 2.0 flow with {% if oauth_github %}GitHub{% endif %}{% if oauth_google %}{% if oauth_github %}, {% endif %}Google{% endif %}{% if oauth_facebook %}{% if oauth_github or oauth_google %}, {% endif %}Facebook{% endif %} (providers were selected when this project was generated)
{% endif %}- **Session-Based**: Signed cookie sessions for web users
- **API Tokens**: Scoped tokens for programmatic access

{% if oauth_github or oauth_google or oauth_facebook %}## OAuth Sign-In

Each compiled-in provider is enabled at runtime when **both** its client ID
and client secret are set in the environment; setting only one is a startup
error. The login page (`/login`) lists a sign-in button per enabled provider.

### Provider Setup

{% if oauth_github %}#### GitHub

1. Create a GitHub OAuth App:
   - Go to [GitHub Developer Settings](https://github.com/settings/developers)
   - Click "New OAuth App"
   - Set the authorization callback URL to: `https://your-domain.com/api/v1/auth/github/callback`

2. Configure environment variables:
   ```bash
   GH_CLIENT_ID=your_client_id
   GH_CLIENT_SECRET=your_client_secret
   GH_REDIRECT_URI=https://your-domain.com/api/v1/auth/github/callback
   ```

{% endif %}{% if oauth_google %}#### Google

1. Create an OAuth client at the
   [Google Cloud Console](https://console.cloud.google.com/apis/credentials)
   (type "Web application").
2. Add `https://your-domain.com/api/v1/auth/google/callback` to the
   "Authorized redirect URIs".
3. Configure environment variables:
   ```bash
   GOOGLE_CLIENT_ID=your_client_id
   GOOGLE_CLIENT_SECRET=your_client_secret
   GOOGLE_REDIRECT_URI=https://your-domain.com/api/v1/auth/google/callback
   ```

{% endif %}{% if oauth_facebook %}#### Facebook

1. Create an app at the [Meta for Developers portal](https://developers.facebook.com/apps)
   and add the "Facebook Login" product.
2. Add `https://your-domain.com/api/v1/auth/facebook/callback` under
   "Valid OAuth Redirect URIs" in the Facebook Login settings.
3. Configure environment variables (the app ID doubles as the client ID):
   ```bash
   FACEBOOK_CLIENT_ID=your_app_id
   FACEBOOK_CLIENT_SECRET=your_app_secret
   FACEBOOK_REDIRECT_URI=https://your-domain.com/api/v1/auth/facebook/callback
   ```

{% endif %}### OAuth Flow

```
1. User clicks a "Sign in with <provider>" button on /login
2. Redirect to the provider's authorization endpoint
3. User authorizes the application
4. Provider redirects to /api/v1/auth/<provider>/callback with an
   authorization code
5. Server verifies the CSRF state and that the flow was started for this
   provider, then exchanges the code for an access token (with PKCE)
6. Server fetches the user profile from the provider
7. Server creates/updates the user in the database (keyed by
   provider + provider user ID)
8. Server creates a session cookie
9. Redirect to the originally requested page
```

### Endpoints

- `GET /login` - Sign-in page listing the enabled providers
- `GET /api/v1/auth/{provider}/authorize` - Initiate the OAuth flow
- `GET /api/v1/auth/{provider}/callback` - OAuth callback
- `POST /api/v1/auth/logout` / `POST /logout` - Logout and clear session

Unknown or unconfigured providers return a controlled `400 Bad Request`.

### Implementation

The OAuth flow is implemented in `src/controllers/auth.rs` as a single pair
of provider-generic handlers; provider endpoints, scopes, and profile-field
mappings live in the registry in `src/oauth.rs`:

```rust
pub async fn oauth_authorize(
    Path(provider): Path<String>,
    State(app): State<AppState>,
) -> Result<Redirect, AppError> {
    // Resolve provider, generate OAuth URL, redirect
}

pub async fn oauth_callback(
    Path(provider): Path<String>,
    Query(params): Query<CallbackQuery>,
    State(app): State<AppState>,
) -> Result<Redirect, AppError> {
    // Verify state + provider match
    // Exchange code for token (PKCE)
    // Fetch user profile
    // Create/update user
    // Create session
    // Redirect
}
```

### Account Linking

Users are identified by the composite key `(provider, provider_user_id)` —
e.g. `("github", "583231")`. Accounts are **not** linked across providers by
email: several providers can return absent or unverified email addresses, so
email-based auto-linking would be an account-takeover risk. Signing in with a
different provider creates a separate account.
{% endif %}

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

- User ID and login (`user_id`, `user_login`)
- CSRF token (`csrf_token`)
- Transient OAuth state (`oauth_state`, `oauth_pkce_verifier`, `oauth_provider`, `redirect_to`)

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

Session cookies expire via `Max-Age` (90 days). Expiration is enforced by the
browser, not the server — revoke access by locking the account instead.

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

- **Provider**: OAuth provider slug (`github`, `google`, `facebook`)
- **Provider User ID**: Unique user ID assigned by the provider
- **Login**: Username/handle from the provider profile
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
2. **Validate Redirect URI**: Ensure the redirect URI matches the provider's app settings exactly
3. **Scope Limitation**: Request minimum required scopes from each provider
4. **State Parameter**: Use state parameter to prevent CSRF (implemented)
5. **No Email-Based Linking**: Never auto-link accounts across providers by email alone (unverified emails are an account-takeover vector)

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

- Check the provider's `*_REDIRECT_URI` matches its app settings exactly
- Ensure HTTPS is used in production
- Verify the provider's `*_CLIENT_ID` and `*_CLIENT_SECRET` are correct — setting only one is a startup error
- A `400` on `/api/v1/auth/{provider}/...` means the provider isn't enabled: either it wasn't compiled in at generation time, or its credentials aren't set

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

### OAuth Implementation

The OAuth flow uses the `oauth2` crate (authorization-code flow with PKCE):

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
