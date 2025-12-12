use std::path::Path;
use std::time::Duration;

use crate::browser::BrowserSession;
use crate::error::Result;
use crate::types::NavigateResult;
use pw::WaitUntil;
use tracing::{info, warn};

pub async fn execute(url: &str, auth_file: Option<&Path>) -> Result<()> {
    info!(target = "pw", %url, "navigate");
    let session = BrowserSession::with_auth(WaitUntil::NetworkIdle, auth_file).await?;
    session.goto(url).await?;

    tokio::time::sleep(Duration::from_millis(2000)).await;

    let title = session.page().title().await.unwrap_or_default();
    let final_url = session.page().url();

    let errors_json = session
        .page()
        .evaluate_value("JSON.stringify(window.__playwrightErrors || [])")
        .await
        .unwrap_or_else(|_| "[]".to_string());

    let errors: Vec<String> = serde_json::from_str(&errors_json).unwrap_or_default();

    if !errors.is_empty() {
        warn!(target = "pw.browser", count = errors.len(), "page reported errors");
    }

    let result = NavigateResult {
        url: final_url,
        title,
        has_errors: !errors.is_empty(),
        errors,
        warnings: vec![],
    };

    println!("{}", serde_json::to_string_pretty(&result)?);

    session.close().await
}
