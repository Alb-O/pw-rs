//! Protocol types for browser extension cookie exchange.
//!
//! This module defines the WebSocket message format for communication between
//! the `pw auth listen` server and the browser extension. The protocol is simple:
//!
//! 1. Extension connects and sends [`ExtensionMessage::Hello`] with a token
//! 2. Server responds with [`ServerMessage::Welcome`] or [`ServerMessage::Rejected`]
//! 3. Extension sends [`ExtensionMessage::PushCookies`] with domain-grouped cookies
//! 4. Server responds with [`ServerMessage::Received`] or [`ServerMessage::Error`]
//!
//! # Main Types
//!
//! - [`ExtensionMessage`] - Messages from browser extension to CLI
//! - [`ServerMessage`] - Messages from CLI to browser extension
//! - [`DomainCookies`] - Cookies grouped by domain
//! - [`ExtensionCookie`] - Chrome cookie format with conversion to Playwright

use serde::{Deserialize, Serialize};

use crate::cookie::{Cookie, SameSite, StorageState};

/// Message sent from the browser extension to the CLI server.
///
/// The extension initiates communication with [`Hello`](Self::Hello) containing
/// the authentication token, then sends cookies via [`PushCookies`](Self::PushCookies).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExtensionMessage {
    /// Initial handshake containing the one-time authentication token.
    Hello {
        /// Token displayed by `pw auth listen`, proves the user authorized this connection.
        token: String,
    },
    /// Push cookies for one or more domains to be saved as auth files.
    PushCookies {
        /// Cookies grouped by domain, each group becomes a separate auth file.
        domains: Vec<DomainCookies>,
    },
}

/// Message sent from the CLI server to the browser extension.
///
/// Responses to extension messages indicating success, failure, or errors.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// Authentication succeeded, connection is ready for cookie transfer.
    Welcome {
        /// Server version for client compatibility checks.
        version: String,
    },
    /// Authentication failed due to invalid or expired token.
    Rejected {
        /// Human-readable reason for rejection.
        reason: String,
    },
    /// Cookies were successfully received and saved to disk.
    Received {
        /// Number of domain auth files written.
        domains_saved: usize,
        /// Absolute paths to the saved auth files.
        paths: Vec<String>,
    },
    /// An error occurred during processing.
    Error {
        /// Human-readable error description.
        message: String,
    },
}

/// Cookies for a single domain, ready to be saved as an auth file.
///
/// Each [`DomainCookies`] instance is converted to a separate Playwright
/// [`StorageState`] file via [`to_storage_state`](Self::to_storage_state).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainCookies {
    /// Domain name without protocol (e.g., `"github.com"` or `".github.com"`).
    pub domain: String,
    /// All cookies associated with this domain.
    pub cookies: Vec<ExtensionCookie>,
}

impl DomainCookies {
    /// Converts to Playwright [`StorageState`] format for use with `--auth`.
    ///
    /// The resulting state contains only cookies (no localStorage), suitable
    /// for loading into a browser context via Playwright's storage state API.
    pub fn to_storage_state(&self) -> StorageState {
        StorageState {
            cookies: self
                .cookies
                .iter()
                .map(ExtensionCookie::to_playwright_cookie)
                .collect(),
            origins: vec![],
        }
    }
}

/// Cookie as provided by the Chrome `chrome.cookies` API.
///
/// This structure matches Chrome's cookie format exactly, which differs from
/// Playwright's [`Cookie`] type in field naming and optional handling.
/// Use [`to_playwright_cookie`](Self::to_playwright_cookie) for conversion.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionCookie {
    /// Cookie name.
    pub name: String,
    /// Cookie value.
    pub value: String,
    /// Domain the cookie belongs to (may have leading dot for domain cookies).
    pub domain: String,
    /// URL path the cookie is valid for.
    pub path: String,
    /// Expiration as Unix timestamp in seconds, [`None`] for session cookies.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiration_date: Option<f64>,
    /// Whether the cookie is inaccessible to JavaScript.
    pub http_only: bool,
    /// Whether the cookie requires HTTPS.
    pub secure: bool,
    /// SameSite attribute: `"lax"`, `"strict"`, `"no_restriction"`, or `"unspecified"`.
    pub same_site: String,
    /// Whether this is a host-only cookie (exact domain match, no leading dot).
    pub host_only: bool,
    /// Chrome's internal cookie store identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store_id: Option<String>,
}

impl ExtensionCookie {
    /// Converts Chrome cookie format to Playwright [`Cookie`] format.
    ///
    /// Handles the differences between Chrome and Playwright cookie representations:
    /// - Maps `expirationDate` to `expires` (using `-1.0` for session cookies)
    /// - Converts `sameSite` string to [`SameSite`] enum
    /// - Wraps required fields in [`Option`] as Playwright expects
    pub fn to_playwright_cookie(&self) -> Cookie {
        Cookie {
            name: self.name.clone(),
            value: self.value.clone(),
            domain: Some(self.domain.clone()),
            path: Some(self.path.clone()),
            expires: self.expiration_date.or(Some(-1.0)),
            http_only: Some(self.http_only),
            secure: Some(self.secure),
            same_site: Some(self.parse_same_site()),
            url: None,
        }
    }

    fn parse_same_site(&self) -> SameSite {
        match self.same_site.as_str() {
            "strict" => SameSite::Strict,
            "lax" => SameSite::Lax,
            "no_restriction" => SameSite::None,
            _ => SameSite::Lax,
        }
    }
}

/// Default WebSocket port for the auth listener server.
pub const AUTH_LISTEN_PORT: u16 = 9271;

/// Default WebSocket host for the auth listener server.
pub const AUTH_LISTEN_HOST: &str = "127.0.0.1";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_message_hello_serializes_with_type_tag() {
        let msg = ExtensionMessage::Hello {
            token: "abc123".into(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"hello""#));
        assert!(json.contains(r#""token":"abc123""#));
    }

    #[test]
    fn server_message_welcome_serializes_with_type_tag() {
        let msg = ServerMessage::Welcome {
            version: "0.12.0".into(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"welcome""#));
    }

    #[test]
    fn extension_cookie_converts_to_playwright_format() {
        let chrome = ExtensionCookie {
            name: "session".into(),
            value: "abc".into(),
            domain: ".github.com".into(),
            path: "/".into(),
            expiration_date: Some(1700000000.0),
            http_only: true,
            secure: true,
            same_site: "lax".into(),
            host_only: false,
            store_id: None,
        };

        let pw = chrome.to_playwright_cookie();
        assert_eq!(pw.name, "session");
        assert_eq!(pw.domain.as_deref(), Some(".github.com"));
        assert_eq!(pw.same_site, Some(SameSite::Lax));
        assert_eq!(pw.expires, Some(1700000000.0));
    }

    #[test]
    fn session_cookie_gets_negative_expiry() {
        let chrome = ExtensionCookie {
            name: "temp".into(),
            value: "val".into(),
            domain: "example.com".into(),
            path: "/".into(),
            expiration_date: None,
            http_only: false,
            secure: false,
            same_site: "unspecified".into(),
            host_only: true,
            store_id: None,
        };

        let pw = chrome.to_playwright_cookie();
        assert_eq!(pw.expires, Some(-1.0));
    }
}
