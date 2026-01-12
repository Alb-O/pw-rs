use clap::ValueEnum;
use serde::{Deserialize, Serialize};

/// Browser type for pw-cli commands
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BrowserKind {
    /// Chromium-based browser (Chrome, Edge)
    #[default]
    Chromium,
    /// Mozilla Firefox
    Firefox,
    /// WebKit (Safari)
    Webkit,
}

impl std::fmt::Display for BrowserKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BrowserKind::Chromium => write!(f, "chromium"),
            BrowserKind::Firefox => write!(f, "firefox"),
            BrowserKind::Webkit => write!(f, "webkit"),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct NavigateResult {
    pub url: String,
    pub title: String,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub has_errors: bool,
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stack: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct ElementCoords {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct IndexedElementCoords {
    pub index: usize,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigate_result_serializes() {
        let result = NavigateResult {
            url: "https://example.com".to_string(),
            title: "Example".to_string(),
            errors: vec!["Error 1".into()],
            warnings: vec![],
            has_errors: true,
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"url\":\"https://example.com\""));
        assert!(json.contains("\"hasErrors\":true"));
    }

    #[test]
    fn console_message_skips_none_stack() {
        let msg = ConsoleMessage {
            msg_type: "log".into(),
            text: "Test log".into(),
            stack: None,
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(!json.contains("stack"));
    }

    #[test]
    fn element_coords_round_trip() {
        let coords = ElementCoords {
            x: 100,
            y: 200,
            width: 50,
            height: 30,
            text: Some("Click me".into()),
            href: None,
        };

        let json = serde_json::to_string(&coords).unwrap();
        assert!(json.contains("\"x\":100"));

        let back: ElementCoords = serde_json::from_str(&json).unwrap();
        assert_eq!(back.x, 100);
        assert_eq!(back.text, Some("Click me".into()));
    }

    #[test]
    fn indexed_element_coords_round_trip() {
        let coords = IndexedElementCoords {
            index: 0,
            x: 10,
            y: 20,
            width: 30,
            height: 40,
            text: Some("Link".into()),
            href: Some("/page".into()),
        };

        let json = serde_json::to_string(&coords).unwrap();
        assert!(json.contains("\"href\":\"/page\""));

        let back: IndexedElementCoords = serde_json::from_str(&json).unwrap();
        assert_eq!(back.index, 0);
        assert_eq!(back.href, Some("/page".into()));
    }
}
