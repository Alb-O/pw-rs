// Cookie and StorageState types for session/authentication management
//
// This module provides types for managing cookies and browser storage state,
// enabling authentication persistence across browser sessions.
//
// See: https://playwright.dev/docs/api/class-browsercontext#browser-context-cookies
// See: https://playwright.dev/docs/api/class-browsercontext#browser-context-storage-state

use serde::{Deserialize, Serialize};

/// SameSite cookie attribute.
///
/// Controls when cookies are sent with cross-site requests.
///
/// See: https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Set-Cookie/SameSite
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SameSite {
    /// Cookie is sent with same-site and cross-site requests
    #[serde(rename = "None")]
    None,
    /// Cookie is sent with same-site requests and cross-site top-level navigations
    #[default]
    #[serde(rename = "Lax")]
    Lax,
    /// Cookie is only sent with same-site requests
    #[serde(rename = "Strict")]
    Strict,
}

/// A browser cookie.
///
/// Represents a cookie with all its attributes. Used for adding cookies to
/// a browser context and retrieving existing cookies.
///
/// # Example
///
/// ```ignore
/// use pw_core::protocol::Cookie;
///
/// let cookie = Cookie::new("session", "abc123", ".example.com");
/// ```
///
/// See: https://playwright.dev/docs/api/class-browsercontext#browser-context-add-cookies
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cookie {
    /// Cookie name
    pub name: String,

    /// Cookie value
    pub value: String,

    /// Domain for the cookie. Either domain or url must be specified.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,

    /// Path for the cookie (default: "/")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,

    /// Unix timestamp in seconds. -1 means session cookie.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires: Option<f64>,

    /// Whether the cookie is HTTP-only
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_only: Option<bool>,

    /// Whether the cookie requires HTTPS
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secure: Option<bool>,

    /// SameSite attribute
    #[serde(skip_serializing_if = "Option::is_none")]
    pub same_site: Option<SameSite>,

    /// URL to infer domain and path from. Either url or domain must be specified.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

impl Cookie {
    /// Creates a new cookie with required fields.
    ///
    /// # Arguments
    ///
    /// * `name` - Cookie name
    /// * `value` - Cookie value
    /// * `domain` - Domain for the cookie (e.g., ".example.com")
    pub fn new(
        name: impl Into<String>,
        value: impl Into<String>,
        domain: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            domain: Some(domain.into()),
            path: None,
            expires: None,
            http_only: None,
            secure: None,
            same_site: None,
            url: None,
        }
    }

    /// Creates a new cookie from a URL (domain and path inferred).
    ///
    /// # Arguments
    ///
    /// * `name` - Cookie name
    /// * `value` - Cookie value
    /// * `url` - URL to infer domain and path from
    pub fn from_url(
        name: impl Into<String>,
        value: impl Into<String>,
        url: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            domain: None,
            path: None,
            expires: None,
            http_only: None,
            secure: None,
            same_site: None,
            url: Some(url.into()),
        }
    }

    /// Sets the path for the cookie
    pub fn path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    /// Sets the expiration timestamp (Unix seconds). Use -1 for session cookie.
    pub fn expires(mut self, expires: f64) -> Self {
        self.expires = Some(expires);
        self
    }

    /// Sets whether the cookie is HTTP-only
    pub fn http_only(mut self, http_only: bool) -> Self {
        self.http_only = Some(http_only);
        self
    }

    /// Sets whether the cookie requires HTTPS
    pub fn secure(mut self, secure: bool) -> Self {
        self.secure = Some(secure);
        self
    }

    /// Sets the SameSite attribute
    pub fn same_site(mut self, same_site: SameSite) -> Self {
        self.same_site = Some(same_site);
        self
    }
}

/// Options for clearing cookies.
///
/// All fields are optional. When specified, only cookies matching all
/// specified criteria will be cleared.
///
/// See: https://playwright.dev/docs/api/class-browsercontext#browser-context-clear-cookies
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearCookiesOptions {
    /// Only clear cookies with this name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Only clear cookies with this domain
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,

    /// Only clear cookies with this path
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

impl ClearCookiesOptions {
    /// Creates new empty options (clears all cookies)
    pub fn new() -> Self {
        Self::default()
    }

    /// Only clear cookies with this name
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Only clear cookies with this domain
    pub fn domain(mut self, domain: impl Into<String>) -> Self {
        self.domain = Some(domain.into());
        self
    }

    /// Only clear cookies with this path
    pub fn path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }
}

/// A localStorage entry within an origin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalStorageEntry {
    /// Storage key
    pub name: String,
    /// Storage value
    pub value: String,
}

/// Storage state for a single origin.
///
/// Contains localStorage entries for a specific origin.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OriginState {
    /// The origin URL (e.g., "https://example.com")
    pub origin: String,

    /// localStorage entries for this origin
    pub local_storage: Vec<LocalStorageEntry>,
}

/// Complete browser storage state.
///
/// Contains all cookies and localStorage data that can be saved and restored
/// to persist authentication across browser sessions.
///
/// # Example
///
/// ```ignore
/// use pw_core::protocol::{BrowserContext, StorageState};
///
/// // Save auth state after login
/// let state = context.storage_state().await?;
/// std::fs::write("auth.json", serde_json::to_string_pretty(&state)?)?;
///
/// // Load auth state in new session
/// let state: StorageState = serde_json::from_str(&std::fs::read_to_string("auth.json")?)?;
/// let options = BrowserContextOptions::builder()
///     .storage_state(state)
///     .build();
/// let context = browser.new_context_with_options(options).await?;
/// ```
///
/// See: https://playwright.dev/docs/api/class-browsercontext#browser-context-storage-state
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StorageState {
    /// All cookies in the browser context
    pub cookies: Vec<Cookie>,

    /// localStorage data per origin
    pub origins: Vec<OriginState>,
}

impl StorageState {
    /// Creates an empty storage state
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a storage state with cookies only
    pub fn with_cookies(cookies: Vec<Cookie>) -> Self {
        Self {
            cookies,
            origins: Vec::new(),
        }
    }

    /// Loads storage state from a JSON file
    pub fn from_file(path: impl AsRef<std::path::Path>) -> std::io::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        serde_json::from_str(&content)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    /// Saves storage state to a JSON file
    pub fn to_file(&self, path: impl AsRef<std::path::Path>) -> std::io::Result<()> {
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, content)
    }
}

/// Options for the storage_state() method.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageStateOptions {
    /// Path to save the storage state to (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

impl StorageStateOptions {
    /// Creates new empty options
    pub fn new() -> Self {
        Self::default()
    }

    /// Save storage state to this file path
    pub fn path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cookie_new() {
        let cookie = Cookie::new("session", "abc123", ".example.com");
        assert_eq!(cookie.name, "session");
        assert_eq!(cookie.value, "abc123");
        assert_eq!(cookie.domain, Some(".example.com".to_string()));
    }

    #[test]
    fn test_cookie_builder() {
        let cookie = Cookie::new("auth", "token123", "example.com")
            .path("/api")
            .expires(1234567890.0)
            .http_only(true)
            .secure(true)
            .same_site(SameSite::Strict);

        assert_eq!(cookie.path, Some("/api".to_string()));
        assert_eq!(cookie.expires, Some(1234567890.0));
        assert_eq!(cookie.http_only, Some(true));
        assert_eq!(cookie.secure, Some(true));
        assert_eq!(cookie.same_site, Some(SameSite::Strict));
    }

    #[test]
    fn test_cookie_from_url() {
        let cookie = Cookie::from_url("token", "xyz", "https://example.com/login");
        assert!(cookie.domain.is_none());
        assert_eq!(cookie.url, Some("https://example.com/login".to_string()));
    }

    #[test]
    fn test_cookie_serialization() {
        let cookie = Cookie::new("test", "value", ".example.com")
            .http_only(true)
            .same_site(SameSite::Lax);

        let json = serde_json::to_string(&cookie).unwrap();
        assert!(json.contains("\"name\":\"test\""));
        assert!(json.contains("\"httpOnly\":true"));
        assert!(json.contains("\"sameSite\":\"Lax\""));
    }

    #[test]
    fn test_same_site_serialization() {
        assert_eq!(serde_json::to_string(&SameSite::None).unwrap(), "\"None\"");
        assert_eq!(serde_json::to_string(&SameSite::Lax).unwrap(), "\"Lax\"");
        assert_eq!(
            serde_json::to_string(&SameSite::Strict).unwrap(),
            "\"Strict\""
        );
    }

    #[test]
    fn test_clear_cookies_options() {
        let opts = ClearCookiesOptions::new()
            .name("session")
            .domain("example.com");

        assert_eq!(opts.name, Some("session".to_string()));
        assert_eq!(opts.domain, Some("example.com".to_string()));
    }

    #[test]
    fn test_storage_state() {
        let state = StorageState {
            cookies: vec![Cookie::new("auth", "token", ".example.com")],
            origins: vec![OriginState {
                origin: "https://example.com".to_string(),
                local_storage: vec![LocalStorageEntry {
                    name: "user".to_string(),
                    value: "john".to_string(),
                }],
            }],
        };

        let json = serde_json::to_string_pretty(&state).unwrap();
        assert!(json.contains("\"cookies\""));
        assert!(json.contains("\"origins\""));
        assert!(json.contains("\"localStorage\""));
    }

    #[test]
    fn test_storage_state_roundtrip() {
        let state = StorageState {
            cookies: vec![
                Cookie::new("session", "abc", ".example.com")
                    .http_only(true)
                    .secure(true),
            ],
            origins: vec![],
        };

        let json = serde_json::to_string(&state).unwrap();
        let restored: StorageState = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.cookies.len(), 1);
        assert_eq!(restored.cookies[0].name, "session");
        assert_eq!(restored.cookies[0].http_only, Some(true));
    }
}
