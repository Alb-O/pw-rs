use std::path::Path;
use std::time::Duration;

use crate::browser::BrowserSession;
use crate::error::{PwError, Result};
use pw::WaitUntil;
use tracing::info;

pub async fn execute(url: &str, condition: &str, auth_file: Option<&Path>) -> Result<()> {
    info!(target = "pw", %url, %condition, "wait");

    let session = BrowserSession::with_auth(WaitUntil::NetworkIdle, auth_file).await?;
    session.goto(url).await?;

    if let Ok(ms) = condition.parse::<u64>() {
        tokio::time::sleep(Duration::from_millis(ms)).await;
        println!("Waited {ms}ms");
    } else if matches!(condition, "load" | "domcontentloaded" | "networkidle") {
        println!("LoadState reached: {condition}");
    } else {
        let escaped = condition.replace('\\', "\\\\").replace('\'', "\\'");
        let mut attempts = 0;
        let max_attempts = 30u64;

        loop {
            let visible = session
                .page()
                .evaluate_value(&format!("document.querySelector('{escaped}') !== null"))
                .await
                .unwrap_or_else(|_| "false".to_string());

            if visible == "true" {
                println!("Element visible: {condition}");
                break;
            }

            attempts += 1;
            if attempts >= max_attempts {
                return Err(PwError::Timeout {
                    ms: max_attempts * 1000,
                    condition: condition.to_string(),
                });
            }

            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    session.close().await
}
