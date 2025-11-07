// Actions example - Interacting with elements
//
// This example demonstrates:
// - Clicking elements
// - Double-clicking elements
// - Filling form inputs
// - Pressing keys
// - Checkbox interactions
// - Hover actions
// - Reading input values
// - Select dropdown interactions
// - File upload interactions
//
// Note: This is a smoke test showing the API.
// Full interaction testing requires custom test pages.

use playwright::Playwright;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🎭 Playwright Actions Example\n");

    // Launch Playwright
    let playwright = Playwright::launch().await?;
    let browser = playwright.chromium().launch().await?;
    let page = browser.new_page().await?;

    // Navigate to a page
    println!("🔗 Navigating to example.com...");
    page.goto("https://example.com", None).await?;
    println!("✅ Page loaded\\n");

    // Click action
    println!("🖱️  Testing click action:");
    let heading = page.locator("h1").await;
    heading.click(None).await?;
    println!("   • Click succeeded on heading");

    // Double-click action
    println!("\\n🖱️🖱️  Testing double-click action:");
    heading.dblclick(None).await?;
    println!("   • Double-click succeeded on heading");

    // Hover action
    println!("\\n👆 Testing hover action:");
    heading.hover(None).await?;
    println!("   • Hover succeeded on heading");

    // Note: The following actions are available but require appropriate elements:
    println!("\\n📋 Available form actions (require appropriate elements):");
    println!("   • fill(text) - Fill input fields");
    println!("   • clear() - Clear input fields");
    println!("   • press(key) - Press keyboard keys");
    println!("   • check() - Check checkboxes/radio buttons");
    println!("   • uncheck() - Uncheck checkboxes");
    println!("   • input_value() - Read input values");
    println!("   • select_option(value) - Select dropdown options");
    println!("   • select_option_multiple(values) - Select multiple options");
    println!("   • set_input_files(path) - Upload files");
    println!("   • set_input_files_multiple(paths) - Upload multiple files");
    println!("\\n   See integration tests for full examples with forms!");

    // Cleanup
    println!("\\n🧹 Cleaning up...");
    page.close().await?;
    browser.close().await?;

    println!("\\n🎉 Example complete!");

    Ok(())
}
