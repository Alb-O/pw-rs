//! Artifact collection for failure diagnostics.
//!
//! When a command fails, this module captures debug artifacts (screenshot, HTML)
//! to help diagnose the failure. Artifacts are saved to the specified directory
//! and reported in the error envelope.

use crate::output::{Artifact, ArtifactType};
use pw::Page;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, warn};

/// Collected artifacts from a failure scenario
#[derive(Debug, Default)]
pub struct CollectedArtifacts {
    pub artifacts: Vec<Artifact>,
}

impl CollectedArtifacts {
    pub fn is_empty(&self) -> bool {
        self.artifacts.is_empty()
    }
}

/// Collect debug artifacts (screenshot + HTML) from the current page state.
///
/// This is called when a command fails to capture the page state at the time of failure.
/// Artifacts are saved to the specified directory with timestamped filenames.
///
/// # Arguments
///
/// * `page` - The Playwright Page to capture
/// * `artifacts_dir` - Directory to save artifacts to
/// * `command_name` - Name of the command that failed (used in filenames)
///
/// # Returns
///
/// A `CollectedArtifacts` containing paths to saved artifacts.
/// Failures during collection are logged but don't propagate - we want the original
/// error to be the one reported.
pub async fn collect_failure_artifacts(
    page: &Page,
    artifacts_dir: &Path,
    command_name: &str,
) -> CollectedArtifacts {
    let mut collected = CollectedArtifacts::default();

    // Generate timestamp for unique filenames
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);

    // Ensure artifacts directory exists
    if let Err(e) = std::fs::create_dir_all(artifacts_dir) {
        warn!("Failed to create artifacts directory: {}", e);
        return collected;
    }

    // Capture screenshot
    let screenshot_path = artifacts_dir.join(format!("{}-{}-failure.png", command_name, timestamp));
    if let Some(artifact) = capture_screenshot(page, &screenshot_path).await {
        collected.artifacts.push(artifact);
    }

    // Capture HTML
    let html_path = artifacts_dir.join(format!("{}-{}-failure.html", command_name, timestamp));
    if let Some(artifact) = capture_html(page, &html_path).await {
        collected.artifacts.push(artifact);
    }

    debug!(
        "Collected {} failure artifacts in {}",
        collected.artifacts.len(),
        artifacts_dir.display()
    );

    collected
}

async fn capture_screenshot(page: &Page, path: &PathBuf) -> Option<Artifact> {
    match page.screenshot_to_file(path, None).await {
        Ok(bytes) => {
            let size_bytes = Some(bytes.len() as u64);
            debug!("Captured failure screenshot: {}", path.display());
            Some(Artifact {
                artifact_type: ArtifactType::Screenshot,
                path: path.clone(),
                size_bytes,
            })
        }
        Err(e) => {
            warn!("Failed to capture screenshot: {}", e);
            None
        }
    }
}

async fn capture_html(page: &Page, path: &PathBuf) -> Option<Artifact> {
    // Get full page HTML via locator("html")
    let locator = page.locator("html").await;
    match locator.inner_html().await {
        Ok(html) => match std::fs::write(path, &html) {
            Ok(()) => {
                let size_bytes = Some(html.len() as u64);
                debug!("Captured failure HTML: {}", path.display());
                Some(Artifact {
                    artifact_type: ArtifactType::Html,
                    path: path.clone(),
                    size_bytes,
                })
            }
            Err(e) => {
                warn!("Failed to write HTML file: {}", e);
                None
            }
        },
        Err(e) => {
            warn!("Failed to capture HTML content: {}", e);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collected_artifacts_empty_by_default() {
        let collected = CollectedArtifacts::default();
        assert!(collected.is_empty());
    }

    #[test]
    fn collected_artifacts_not_empty_with_items() {
        let mut collected = CollectedArtifacts::default();
        collected.artifacts.push(Artifact {
            artifact_type: ArtifactType::Screenshot,
            path: PathBuf::from("/tmp/test.png"),
            size_bytes: Some(1234),
        });
        assert!(!collected.is_empty());
    }
}
