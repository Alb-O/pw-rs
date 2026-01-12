//! Cookie and storage state types for session management.
//!
//! These types represent browser cookies and localStorage data that can be
//! saved and restored to persist authentication across sessions.

use serde::{Deserialize, Serialize};

/// SameSite cookie attribute.
///
/// Controls when cookies are sent with cross-site requests.
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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cookie {
    /// Cookie name
    pub name: String,

    /// Cookie value
    pub value: String,

    /// Domain for the cookie
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,

    /// Path for the cookie
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,

    /// Unix timestamp in seconds (-1 means session cookie)
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

    /// URL to infer domain and path from
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

impl Cookie {
    /// Creates a new cookie with required fields.
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

    /// Sets the path for the cookie.
    pub fn path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    /// Sets the expiration timestamp.
    pub fn expires(mut self, expires: f64) -> Self {
        self.expires = Some(expires);
        self
    }

    /// Sets whether the cookie is HTTP-only.
    pub fn http_only(mut self, http_only: bool) -> Self {
        self.http_only = Some(http_only);
        self
    }

    /// Sets whether the cookie requires HTTPS.
    pub fn secure(mut self, secure: bool) -> Self {
        self.secure = Some(secure);
        self
    }

    /// Sets the SameSite attribute.
    pub fn same_site(mut self, same_site: SameSite) -> Self {
        self.same_site = Some(same_site);
        self
    }
}

/// Options for clearing cookies.
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
    /// Creates new empty options (clears all cookies).
    pub fn new() -> Self {
        Self::default()
    }

    /// Only clear cookies with this name.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Only clear cookies with this domain.
    pub fn domain(mut self, domain: impl Into<String>) -> Self {
        self.domain = Some(domain.into());
        self
    }

    /// Only clear cookies with this path.
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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OriginState {
    /// The origin URL
    pub origin: String,
    /// localStorage entries for this origin
    pub local_storage: Vec<LocalStorageEntry>,
}

/// Complete browser storage state.
///
/// Contains all cookies and localStorage data that can be saved and restored
/// to persist authentication across browser sessions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StorageState {
    /// All cookies in the browser context
    pub cookies: Vec<Cookie>,
    /// localStorage data per origin
    pub origins: Vec<OriginState>,
}

impl StorageState {
    /// Creates an empty storage state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a storage state with cookies only.
    pub fn with_cookies(cookies: Vec<Cookie>) -> Self {
        Self {
            cookies,
            origins: Vec::new(),
        }
    }

    /// Loads storage state from a JSON file.
    pub fn from_file(path: impl AsRef<std::path::Path>) -> std::io::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        serde_json::from_str(&content)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    /// Saves storage state to a JSON file.
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
    /// Path to save the storage state to
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

impl StorageStateOptions {
    /// Creates new empty options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Save storage state to this file path.
    pub fn path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cookie_serialization() {
        let cookie = Cookie::new("session", "abc", ".example.com")
            .http_only(true)
            .same_site(SameSite::Lax);

        let json = serde_json::to_string(&cookie).unwrap();
        assert!(json.contains("\"name\":\"session\""));
        assert!(json.contains("\"httpOnly\":true"));
    }

    #[test]
    fn test_storage_state_roundtrip() {
        let state = StorageState {
            cookies: vec![Cookie::new("auth", "token", ".example.com")],
            origins: vec![],
        };

        let json = serde_json::to_string(&state).unwrap();
        let restored: StorageState = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.cookies.len(), 1);
    }
}
