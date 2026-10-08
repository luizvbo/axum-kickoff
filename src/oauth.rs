//! OAuth provider registry and user-profile mapping.
//!
//! Provider specs are compiled into the application (the `oauth_*`
//! cargo-generate options control which ones exist). A spec being compiled in
//! does not make the provider usable at runtime: it still needs its client
//! credentials in the environment. See `config::Server::oauth_providers`.

use serde_json::Value;

/// Static description of an OAuth provider: endpoints, scopes, environment
/// variables carrying its client credentials, and userinfo mapping.
#[derive(Debug)]
pub struct OAuthProviderSpec {
    /// URL-safe identifier used in routes (`/api/v1/auth/{slug}/authorize`)
    /// and stored in `users.provider`.
    pub slug: &'static str,
    /// Human-readable name shown on the login page and in error messages.
    pub display_name: &'static str,
    /// OAuth2 authorization endpoint.
    pub authorize_url: &'static str,
    /// OAuth2 token endpoint.
    pub token_url: &'static str,
    /// Userinfo endpoint; must return the authenticated profile as JSON.
    pub userinfo_url: &'static str,
    /// OAuth2 scopes requested during authorization.
    pub scopes: &'static [&'static str],
    /// Environment variable holding the OAuth client ID.
    pub client_id_env: &'static str,
    /// Environment variable holding the OAuth client secret.
    pub client_secret_env: &'static str,
    /// Environment variable optionally overriding the callback URL. When
    /// unset it defaults to
    /// `<scheme>://<DOMAIN_NAME>:<PORT>/api/v1/auth/<slug>/callback`.
    pub redirect_uri_env: &'static str,
    /// Maps the provider's userinfo JSON to a normalized [`OAuthProfile`].
    /// Returns `None` when required fields are missing.
    pub parse_profile: fn(&Value) -> Option<OAuthProfile>,
}

/// Provider-agnostic user profile extracted from a userinfo response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthProfile {
    /// Unique, stable identifier assigned by the provider (`id`, `sub`, ...).
    pub provider_user_id: String,
    /// Username/handle stored as the user's login.
    pub login: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
}
{% if oauth_github or oauth_google or oauth_facebook %}
/// Reads an optional string field from a userinfo JSON object.
fn opt_str(body: &Value, field: &str) -> Option<String> {
    body.get(field).and_then(Value::as_str).map(str::to_owned)
}
{% endif %}{% if oauth_github %}
/// GitHub OAuth — create an OAuth app at
/// <https://github.com/settings/developers>.
static GITHUB: OAuthProviderSpec = OAuthProviderSpec {
    slug: "github",
    display_name: "GitHub",
    authorize_url: "https://github.com/login/oauth/authorize",
    token_url: "https://github.com/login/oauth/access_token",
    userinfo_url: "https://api.github.com/user",
    scopes: &["read:user"],
    client_id_env: "GH_CLIENT_ID",
    client_secret_env: "GH_CLIENT_SECRET",
    redirect_uri_env: "GH_REDIRECT_URI",
    parse_profile: |body| {
        Some(OAuthProfile {
            provider_user_id: body.get("id")?.as_u64()?.to_string(),
            login: body.get("login")?.as_str()?.to_owned(),
            name: opt_str(body, "name"),
            email: opt_str(body, "email"),
            avatar_url: opt_str(body, "avatar_url"),
        })
    },
};
{% endif %}{% if oauth_google %}
/// Google OAuth — create an OAuth client at
/// <https://console.cloud.google.com/apis/credentials>.
static GOOGLE: OAuthProviderSpec = OAuthProviderSpec {
    slug: "google",
    display_name: "Google",
    authorize_url: "https://accounts.google.com/o/oauth2/v2/auth",
    token_url: "https://oauth2.googleapis.com/token",
    userinfo_url: "https://openidconnect.googleapis.com/v1/userinfo",
    scopes: &["openid", "email", "profile"],
    client_id_env: "GOOGLE_CLIENT_ID",
    client_secret_env: "GOOGLE_CLIENT_SECRET",
    redirect_uri_env: "GOOGLE_REDIRECT_URI",
    parse_profile: |body| {
        let sub = body.get("sub")?.as_str()?;
        let name = opt_str(body, "name");
        let email = opt_str(body, "email");
        Some(OAuthProfile {
            provider_user_id: sub.to_owned(),
            login: email
                .clone()
                .or(name.clone())
                .unwrap_or_else(|| sub.to_owned()),
            name,
            email,
            avatar_url: opt_str(body, "picture"),
        })
    },
};
{% endif %}{% if oauth_facebook %}
/// Facebook OAuth — create an app at
/// <https://developers.facebook.com/apps> and add the "Facebook Login"
/// product. `FACEBOOK_REDIRECT_URI` must be listed under "Valid OAuth
/// Redirect URIs" in the app's Facebook Login settings.
static FACEBOOK: OAuthProviderSpec = OAuthProviderSpec {
    slug: "facebook",
    display_name: "Facebook",
    authorize_url: "https://www.facebook.com/v22.0/dialog/oauth",
    token_url: "https://graph.facebook.com/v22.0/oauth/access_token",
    userinfo_url: "https://graph.facebook.com/v22.0/me?fields=id,name,email,picture",
    scopes: &["public_profile", "email"],
    client_id_env: "FACEBOOK_CLIENT_ID",
    client_secret_env: "FACEBOOK_CLIENT_SECRET",
    redirect_uri_env: "FACEBOOK_REDIRECT_URI",
    parse_profile: |body| {
        let id = body.get("id")?.as_str()?;
        let name = opt_str(body, "name");
        let email = opt_str(body, "email");
        Some(OAuthProfile {
            provider_user_id: id.to_owned(),
            login: email
                .clone()
                .or(name.clone())
                .unwrap_or_else(|| id.to_owned()),
            name,
            email,
            avatar_url: body
                .pointer("/picture/data/url")
                .and_then(Value::as_str)
                .map(str::to_owned),
        })
    },
};
{% endif %}
/// All OAuth providers compiled into this application.
///
/// A spec being listed here does not mean the provider is usable at runtime:
/// it still needs its client credentials in the environment.
///
/// The `allow` covers clippy's `vec_init_then_push` — the pushes are
/// unconditional in every render because the template conditionals decide
/// which ones exist at generation time, so `vec![...]` would fight the
/// Liquid structure.
#[allow(clippy::vec_init_then_push)]
pub fn provider_specs() -> Vec<&'static OAuthProviderSpec> {
    #[allow(unused_mut)]
    let mut specs = Vec::new();
{% if oauth_github %}    specs.push(&GITHUB);
{% endif %}{% if oauth_google %}    specs.push(&GOOGLE);
{% endif %}{% if oauth_facebook %}    specs.push(&FACEBOOK);
{% endif %}    specs
}

#[cfg(test)]
mod tests {
    use super::*;
{% if oauth_github or oauth_google or oauth_facebook %}    use serde_json::json;
{% endif %}
{% if oauth_github %}    #[test]
    fn github_profile_parses() {
        let body = json!({
            "id": 583231,
            "login": "octocat",
            "name": "The Octocat",
            "email": "octocat@github.com",
            "avatar_url": "https://avatars.githubusercontent.com/u/583231"
        });
        let profile = (GITHUB.parse_profile)(&body).unwrap();
        assert_eq!(profile.provider_user_id, "583231");
        assert_eq!(profile.login, "octocat");
        assert_eq!(profile.name.as_deref(), Some("The Octocat"));
        assert_eq!(profile.email.as_deref(), Some("octocat@github.com"));
    }

    #[test]
    fn github_profile_missing_id_fails() {
        assert!((GITHUB.parse_profile)(&json!({"login": "octocat"})).is_none());
    }

{% endif %}{% if oauth_google %}    #[test]
    fn google_profile_parses() {
        let body = json!({
            "sub": "110169484474386276334",
            "name": "Ada Lovelace",
            "email": "ada@example.com",
            "picture": "https://lh3.googleusercontent.com/photo"
        });
        let profile = (GOOGLE.parse_profile)(&body).unwrap();
        assert_eq!(profile.provider_user_id, "110169484474386276334");
        assert_eq!(profile.login, "ada@example.com");
        assert_eq!(
            profile.avatar_url.as_deref(),
            Some("https://lh3.googleusercontent.com/photo")
        );
    }

    #[test]
    fn google_profile_missing_sub_fails() {
        assert!((GOOGLE.parse_profile)(&json!({"email": "a@b.c"})).is_none());
    }

{% endif %}{% if oauth_facebook %}    #[test]
    fn facebook_profile_parses() {
        let body = json!({
            "id": "102233445566",
            "name": "Grace Hopper",
            "email": "grace@example.com",
            "picture": {"data": {"url": "https://platform-lookaside.fbsbx.com/pic"}}
        });
        let profile = (FACEBOOK.parse_profile)(&body).unwrap();
        assert_eq!(profile.provider_user_id, "102233445566");
        assert_eq!(profile.login, "grace@example.com");
        assert_eq!(
            profile.avatar_url.as_deref(),
            Some("https://platform-lookaside.fbsbx.com/pic")
        );
    }

    #[test]
    fn facebook_profile_without_email_uses_id_as_login() {
        let body = json!({"id": "102233", "name": "Grace Hopper"});
        let profile = (FACEBOOK.parse_profile)(&body).unwrap();
        assert_eq!(profile.login, "Grace Hopper");
        assert!(profile.email.is_none());
    }

{% endif %}    #[test]
    fn provider_specs_have_unique_slugs() {
        let specs = provider_specs();
        let mut slugs: Vec<_> = specs.iter().map(|s| s.slug).collect();
        slugs.sort_unstable();
        slugs.dedup();
        assert_eq!(slugs.len(), specs.len());
    }
}
