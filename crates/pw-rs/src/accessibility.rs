// Copyright 2024 Paul Adamson
// Licensed under the Apache License, Version 2.0

//! Accessibility tree inspection API.
//!
//! The Accessibility API allows capturing a snapshot of the page's accessibility tree,
//! which is useful for testing accessibility features and understanding how assistive
//! technologies see the page.
//!
//! # Example
//!
//! ```ignore
//! // Get the full accessibility tree
//! let snapshot = page.accessibility().snapshot(None).await?;
//!
//! if let Some(tree) = snapshot {
//!     println!("Root role: {}", tree.role);
//!     for child in tree.children.iter().flatten() {
//!         println!("  Child: {} - {:?}", child.role, child.name);
//!     }
//! }
//!
//! // Get accessibility tree for a specific element
//! let root = page.query_selector("main").await?.unwrap();
//! let snapshot = page.accessibility().snapshot(Some(
//!     AccessibilitySnapshotOptions::builder()
//!         .root(root)
//!         .build()
//! )).await?;
//! ```
//!
//! See: <https://playwright.dev/docs/api/class-accessibility>

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::Page;
use pw_runtime::Result;
use pw_runtime::channel_owner::ChannelOwner;

/// Handle for accessibility tree inspection on a [`Page`].
///
/// Obtain via [`Page::accessibility()`].
///
/// [`Page::accessibility()`]: crate::Page::accessibility
#[derive(Clone)]
pub struct Accessibility {
    page: Page,
}

impl Accessibility {
    /// Creates a new Accessibility handle for the given page.
    pub(crate) fn new(page: Page) -> Self {
        Self { page }
    }

    /// Captures the accessibility tree of the page.
    ///
    /// Returns `None` if the page has no accessibility tree (e.g., empty page).
    /// Pass `options` to filter the snapshot or root it at a specific element.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ProtocolError`] if the page has been closed.
    ///
    /// [`Error::ProtocolError`]: pw_runtime::Error::ProtocolError
    ///
    /// See: <https://playwright.dev/docs/api/class-accessibility#accessibility-snapshot>
    pub async fn snapshot(
        &self,
        options: Option<AccessibilitySnapshotOptions>,
    ) -> Result<Option<AccessibilityNode>> {
        let params = options
            .map(|o| serde_json::to_value(&o).unwrap_or_default())
            .unwrap_or_else(|| serde_json::json!({}));

        #[derive(Deserialize)]
        struct SnapshotResponse {
            #[serde(rename = "rootAXNode")]
            root_ax_node: Option<AccessibilityNode>,
        }

        let response: SnapshotResponse = self
            .page
            .channel()
            .send("accessibilitySnapshot", params)
            .await?;

        Ok(response.root_ax_node)
    }
}

/// Options for capturing an accessibility snapshot.
///
/// See: <https://playwright.dev/docs/api/class-accessibility#accessibility-snapshot>
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessibilitySnapshotOptions {
    /// Whether to include nodes that are not interesting for most users.
    ///
    /// "Interesting" nodes are nodes with an accessible name or role that isn't
    /// typically hidden (e.g., not "none" or "presentation").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interesting_only: Option<bool>,

    /// The root element to capture the snapshot from.
    ///
    /// When specified, only the subtree rooted at this element is returned.
    #[serde(skip_serializing_if = "Option::is_none", rename = "root")]
    root_guid: Option<String>,
}

impl AccessibilitySnapshotOptions {
    /// Creates a new builder for snapshot options.
    pub fn builder() -> AccessibilitySnapshotOptionsBuilder {
        AccessibilitySnapshotOptionsBuilder::default()
    }
}

/// Builder for [`AccessibilitySnapshotOptions`].
#[derive(Debug, Clone, Default)]
pub struct AccessibilitySnapshotOptionsBuilder {
    interesting_only: Option<bool>,
    root_guid: Option<String>,
}

impl AccessibilitySnapshotOptionsBuilder {
    /// Sets whether to include only interesting nodes.
    pub fn interesting_only(mut self, value: bool) -> Self {
        self.interesting_only = Some(value);
        self
    }

    /// Sets the root element for the snapshot.
    pub fn root(mut self, element: Arc<crate::ElementHandle>) -> Self {
        self.root_guid = Some(element.guid().to_string());
        self
    }

    /// Builds the options.
    pub fn build(self) -> AccessibilitySnapshotOptions {
        AccessibilitySnapshotOptions {
            interesting_only: self.interesting_only,
            root_guid: self.root_guid,
        }
    }
}

/// A node in the accessibility tree.
///
/// Represents an element as seen by assistive technologies like screen readers.
///
/// See: <https://playwright.dev/docs/api/class-accessibility#accessibility-snapshot>
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessibilityNode {
    /// The ARIA role of the node (e.g., "button", "heading", "link").
    pub role: String,

    /// The accessible name of the node.
    pub name: Option<String>,

    /// The accessible value of the node.
    pub value: Option<AccessibilityValue>,

    /// The accessible description of the node.
    pub description: Option<String>,

    /// Keyboard shortcut associated with the node.
    pub key_shortcuts: Option<String>,

    /// Role description override.
    pub role_description: Option<String>,

    /// Value text for range widgets.
    pub value_text: Option<String>,

    /// Whether the node is disabled.
    #[serde(default)]
    pub disabled: bool,

    /// Whether the node is expanded (for expandable elements).
    pub expanded: Option<bool>,

    /// Whether the node is focused.
    #[serde(default)]
    pub focused: bool,

    /// Whether the node is modal.
    #[serde(default)]
    pub modal: bool,

    /// Whether the node supports multiple selection.
    #[serde(default)]
    pub multiselectable: bool,

    /// Whether the node is readonly.
    #[serde(default)]
    pub readonly: bool,

    /// Whether the node is required.
    #[serde(default)]
    pub required: bool,

    /// Whether the node is selected.
    pub selected: Option<bool>,

    /// The checked state for checkboxes and radio buttons.
    pub checked: Option<CheckedState>,

    /// The pressed state for toggle buttons.
    pub pressed: Option<PressedState>,

    /// The heading level (1-6).
    pub level: Option<u8>,

    /// Minimum value for range widgets.
    pub value_min: Option<f64>,

    /// Maximum value for range widgets.
    pub value_max: Option<f64>,

    /// The autocomplete behavior.
    pub autocomplete: Option<String>,

    /// The haspopup behavior.
    pub haspopup: Option<String>,

    /// Whether the node is invalid.
    pub invalid: Option<String>,

    /// The orientation for sliders and scrollbars.
    pub orientation: Option<String>,

    /// Child nodes.
    #[serde(default)]
    pub children: Option<Vec<AccessibilityNode>>,
}

/// The value of an accessibility node.
///
/// Can be a string or a number depending on the node type.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum AccessibilityValue {
    /// String value (e.g., text content)
    String(String),
    /// Numeric value (e.g., slider position)
    Number(f64),
}

/// The checked state of a checkbox or radio button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckedState {
    /// The element is checked
    True,
    /// The element is unchecked
    False,
    /// The element is in an indeterminate state
    Mixed,
}

/// The pressed state of a toggle button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PressedState {
    /// The button is pressed
    True,
    /// The button is not pressed
    False,
    /// The button is in a mixed state
    Mixed,
}
