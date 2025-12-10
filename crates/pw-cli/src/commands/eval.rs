use crate::browser::BrowserSession;
use crate::error::Result;
use pw::WaitUntil;
use tracing::{debug, info};

pub async fn execute(url: &str, expression: &str) -> Result<()> {
    info!(target = "pw", %url, "eval js");
    debug!(target = "pw", %expression, "expression");

    let session = BrowserSession::new(WaitUntil::NetworkIdle).await?;
    session.goto(url).await?;

    let result = session
        .page()
        .evaluate_value(&format!("JSON.stringify({})", expression))
        .await?;

    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&result) {
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        println!("{result}");
    }

    session.close().await
}
